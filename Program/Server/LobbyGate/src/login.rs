//! Gate-key and client-key correlation. Spec 05 §1.3.
use crate::{daemon::LobbyDaemon, wire::invalid};
use bloodlib::packet::{compose, decode_fields, decompose, DecodeLimits, Value};
use packet::fragments::compose_fragments;
use packet_ids::{LobbyCmd, LobbyGateLobbydCmd};
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;

/// Rust policy: drop the newest broadcast when a session queue is full.
pub const DELIVERY_QUEUE_CAPACITY: usize = 32;
#[derive(Debug)]
pub struct Delivery {
    pub packets: Vec<Vec<u8>>,
    pub authed: Option<u64>,
    pub wire: bool,
    pub close: bool,
}
#[derive(Clone)]
struct Entry {
    id: u64,
    tx: mpsc::Sender<Delivery>,
    client_key: u64,
    broadcast: bool,
}
#[derive(Clone)]
struct Client {
    gate: u64,
    session: Option<Entry>,
}
#[derive(Default)]
struct State {
    gates: HashMap<u64, Entry>,
    clients: HashMap<u64, Client>,
}
#[derive(Default)]
pub struct LoginHub {
    state: Mutex<State>,
}
pub struct SessionGuard {
    hub: Arc<LoginHub>,
    id: u64,
}
impl Drop for SessionGuard {
    fn drop(&mut self) {
        let mut state = self.hub.state.lock().unwrap();
        let keys: Vec<_> = state
            .gates
            .iter()
            .filter(|(_, e)| e.id == self.id)
            .map(|(k, _)| *k)
            .collect();
        for key in keys {
            state.gates.remove(&key);
        }
        state.clients.retain(|_, client| {
            !client
                .session
                .as_ref()
                .is_some_and(|entry| entry.id == self.id)
        });
    }
}
impl LoginHub {
    pub fn guard(self: &Arc<Self>, id: u64) -> SessionGuard {
        SessionGuard {
            hub: self.clone(),
            id,
        }
    }
    pub fn insert(&self, key: u64, id: u64, tx: mpsc::Sender<Delivery>) {
        self.state.lock().unwrap().gates.insert(
            key,
            Entry {
                id,
                tx,
                client_key: 0,
                broadcast: false,
            },
        );
    }
    pub fn enable_broadcast(&self, key: u64) {
        let mut state = self.state.lock().unwrap();
        if let Some(client) = state.clients.get_mut(&key) {
            if let Some(entry) = &mut client.session {
                entry.broadcast = true;
            }
        }
    }
    pub fn broadcast_enabled(&self, key: u64) -> bool {
        self.by_client(key).is_some_and(|entry| entry.broadcast)
    }
    fn by_client(&self, key: u64) -> Option<Entry> {
        let s = self.state.lock().unwrap();
        s.clients.get(&key)?.session.clone()
    }
    pub fn gate_for_client(&self, key: u64) -> Option<u64> {
        self.state
            .lock()
            .unwrap()
            .clients
            .get(&key)
            .map(|client| client.gate)
    }
    pub fn has_session(&self, key: u64) -> bool {
        self.by_client(key)
            .is_some_and(|entry| !entry.tx.is_closed())
    }
    pub async fn close_gate(&self, gate: u64) {
        let entry = {
            let mut state = self.state.lock().unwrap();
            let Some(entry) = state.gates.remove(&gate) else {
                return;
            };
            if entry.client_key == 0 {
                Some(entry)
            } else if state
                .clients
                .get(&entry.client_key)
                .is_some_and(|c| c.session.is_some())
            {
                state
                    .clients
                    .remove(&entry.client_key)
                    .and_then(|c| c.session)
            } else {
                // Spec 05 §6.4: the detached client record stays.
                None
            }
        };
        if let Some(e) = entry {
            let _ =
                e.tx.send(Delivery {
                    packets: vec![],
                    authed: None,
                    wire: false,
                    close: true,
                })
                .await;
        }
    }
    pub fn broadcast(&self, packets: Vec<Vec<u8>>) {
        let entries: Vec<_> = self
            .state
            .lock()
            .unwrap()
            .clients
            .values()
            .filter_map(|client| client.session.as_ref())
            .filter(|e| e.broadcast)
            .cloned()
            .collect();
        for e in entries {
            let _ = e.tx.try_send(Delivery {
                packets: packets.clone(),
                authed: None,
                wire: true,
                close: false,
            });
        }
    }
    pub async fn send_client(&self, key: u64, packets: Vec<Vec<u8>>) {
        if let Some(e) = self.by_client(key) {
            let _ =
                e.tx.send(Delivery {
                    packets,
                    authed: None,
                    wire: false,
                    close: false,
                })
                .await;
        }
    }
    pub async fn handle(
        &self,
        cmd: i16,
        data: &[u8],
        daemon: &dyn LobbyDaemon,
        broadcast: &[u8],
    ) -> io::Result<()> {
        let p = |e: bloodlib::packet::PacketError| invalid(&e.to_string());
        match cmd {
            7 | 13 => {
                let v = decompose(
                    if cmd == 7 { "hqqoo" } else { "hqq" },
                    data,
                    DecodeLimits::default(),
                )
                .map_err(p)?;
                let (Value::U64(gate), Value::U64(client)) = (&v[1], &v[2]) else {
                    unreachable!()
                };
                let (entry, added) = {
                    let mut s = self.state.lock().unwrap();
                    let Some(entry) = s.gates.get(gate).cloned() else {
                        return Ok(());
                    };
                    if cmd == 13 && (*gate == 0 || *client == 0) {
                        return Ok(());
                    }
                    let added = *client != 0
                        && !s.clients.contains_key(client)
                        && !s
                            .gates
                            .values()
                            .any(|e| e.id == entry.id && e.client_key != 0);
                    if added {
                        s.clients.insert(
                            *client,
                            Client {
                                gate: *gate,
                                session: Some(entry.clone()),
                            },
                        );
                        s.gates.get_mut(gate).unwrap().client_key = *client;
                    }
                    (entry, added)
                };
                let request = if cmd == 7 {
                    compose(
                        "hqoo",
                        &[
                            Value::I16(LobbyGateLobbydCmd::StartSessionWithNPTokenReq.id()),
                            v[2].clone(),
                            v[3].clone(),
                            v[4].clone(),
                        ],
                    )
                } else {
                    compose(
                        "hq",
                        &[
                            Value::I16(LobbyGateLobbydCmd::StartSessionWithLobbyTokenReq.id()),
                            v[2].clone(),
                        ],
                    )
                }
                .map_err(p)?;
                if !added || daemon.send_packet(request).await.is_err() {
                    let fail = if cmd == 7 {
                        LobbyCmd::L_LoginWithNPTokenFail
                    } else {
                        LobbyCmd::L_LoginWithLobbyTokenFail
                    };
                    deliver_fail(entry, fail.id(), 2).await;
                }
            }
            8 | 11 | 14 | 17 => {
                let v = decompose("hqE", data, DecodeLimits::default()).map_err(p)?;
                let (Value::U64(key), Value::I16(reason)) = (&v[1], &v[2]) else {
                    unreachable!()
                };
                let entry = {
                    let mut s = self.state.lock().unwrap();
                    let gate = if matches!(cmd, 11 | 17) {
                        s.clients.remove(key).map(|client| client.gate)
                    } else {
                        Some(*key)
                    };
                    gate.and_then(|gate| s.gates.remove(&gate))
                };
                if let Some(entry) = entry {
                    let fail = if cmd <= 11 {
                        LobbyCmd::L_LoginWithNPTokenFail
                    } else {
                        LobbyCmd::L_LoginWithLobbyTokenFail
                    };
                    deliver_fail(entry, fail.id(), map_reason(cmd, *reason)).await;
                }
            }
            10 | 16 => {
                let format = if cmd == 10 {
                    "qQoihCiiiii;CiiQiqq;Qqiqiq"
                } else {
                    "qCiiiii;CiiQiqq;iqiq"
                };
                let mut v = decode_fields(format, data, DecodeLimits::default()).map_err(p)?;
                let Value::U64(client) = v[0] else {
                    unreachable!()
                };
                let Some(entry) = self.by_client(client) else {
                    return Ok(());
                };
                if cmd == 10 {
                    let Value::I8(create) = v[1] else {
                        unreachable!()
                    };
                    v[1] = Value::I8(i8::from(create == 1));
                    let Value::I8(teen) = v[7] else {
                        unreachable!()
                    };
                    v[7] = Value::I8(i8::from(teen != 0));
                }
                v.push(Value::Bytes(broadcast.to_vec()));
                let ok = if cmd == 10 {
                    LobbyCmd::L_LoginWithNPTokenOk
                } else {
                    LobbyCmd::L_LoginWithLobbyTokenOk
                };
                let packets = compose_fragments(ok.id(), &format!("hi{format}K"), &v).map_err(p)?;
                let _ = entry
                    .tx
                    .send(Delivery {
                        packets,
                        authed: Some(client),
                        wire: false,
                        close: false,
                    })
                    .await;
                // C109 is never sent: both reload globals are zero (spec 05 §5.4).
            }
            _ => (),
        }
        Ok(())
    }
}
async fn deliver_fail(entry: Entry, cmd: i16, reason: i16) {
    let packet = compose("hh", &[Value::I16(cmd), Value::I16(reason)]).expect("fixed failure");
    let _ = entry
        .tx
        .send(Delivery {
            packets: vec![packet],
            authed: None,
            wire: false,
            close: false,
        })
        .await;
}
pub fn map_reason(cmd: i16, reason: i16) -> i16 {
    match (cmd, reason) {
        (11, 15..=18) => reason - 5,
        (14, 19..=21) => reason - 5,
        _ => reason,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn stalled_tcp_receiver_keeps_fifo_drops_newest_and_recovers() {
        use crate::wire::{read_frame, write_frame};
        use tokio::{
            net::{TcpListener, TcpSocket, TcpStream},
            time::{timeout, Duration},
        };
        async fn pair() -> (TcpStream, TcpStream) {
            let socket = TcpSocket::new_v4().unwrap();
            socket.set_send_buffer_size(1024).unwrap();
            socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
            let listener: TcpListener = socket.listen(1).unwrap();
            let socket = TcpSocket::new_v4().unwrap();
            socket.set_recv_buffer_size(4096).unwrap();
            let client = socket
                .connect(listener.local_addr().unwrap())
                .await
                .unwrap();
            let (server, _) = listener.accept().await.unwrap();
            server.set_nodelay(true).unwrap();
            (server, client)
        }
        let hub = LoginHub::default();
        let mut writers = Vec::new();
        let mut peers = Vec::new();
        let mut slow_queue = None;
        for key in 1..=2 {
            let (mut socket, peer) = pair().await;
            peers.push(peer);
            let (tx, mut rx) = mpsc::channel::<Delivery>(DELIVERY_QUEUE_CAPACITY);
            if key == 1 {
                slow_queue = Some(tx.clone());
            }
            hub.state.lock().unwrap().clients.insert(
                key,
                Client {
                    gate: key,
                    session: Some(Entry {
                        id: key,
                        tx,
                        client_key: key,
                        broadcast: true,
                    }),
                },
            );
            writers.push(tokio::spawn(async move {
                while let Some(delivery) = rx.recv().await {
                    for frame in delivery.packets {
                        write_frame(&mut socket, &frame).await.unwrap();
                    }
                }
            }));
        }
        let mut fast = peers.pop().unwrap();
        let mut slow = peers.pop().unwrap();
        let slow_queue = slow_queue.unwrap();
        let frame = |n: u16| {
            let mut text = vec![65; 1900];
            text[0] = n;
            crate::lists::Lists::announcement(
                &compose("hs", &[Value::I16(5), Value::String(text)]).unwrap(),
                &[7; 17],
            )
            .unwrap()
            .remove(0)
        };
        let mut accepted = Vec::new();
        // The fast TCP peer drains each frame. The slow TCP peer reads nothing.
        // Wait for a full application queue caused by a blocked TCP writer.
        for n in 0..2000 {
            if slow_queue.capacity() == 0 {
                // A TCP ACK can free one writer slot after the first full
                // observation. Continue until the full queue stays full.
                tokio::time::sleep(Duration::from_millis(100)).await;
                if slow_queue.capacity() == 0 {
                    break;
                }
            }
            let packet = frame(n);
            hub.broadcast(vec![packet.clone()]);
            assert_eq!(
                timeout(Duration::from_secs(2), read_frame(&mut fast))
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap(),
                packet
            );
            accepted.push(packet);
        }
        assert_eq!(
            slow_queue.capacity(),
            0,
            "TCP receiver did not stall the writer"
        );
        // A stable full queue proves this is TCP backpressure, not task scheduling.
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(slow_queue.capacity(), 0);
        let dropped = frame(3000);
        hub.broadcast(vec![dropped.clone()]);
        assert_eq!(
            timeout(Duration::from_secs(2), read_frame(&mut fast))
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            dropped
        );
        assert_eq!(slow_queue.capacity(), 0);
        let expected = accepted.clone();
        let drain = tokio::spawn(async move {
            for packet in expected {
                assert_eq!(
                    timeout(Duration::from_secs(5), read_frame(&mut slow))
                        .await
                        .unwrap()
                        .unwrap()
                        .unwrap(),
                    packet
                );
            }
            slow
        });
        let mut slow = timeout(Duration::from_secs(10), drain)
            .await
            .unwrap()
            .unwrap();
        let recovery = frame(4000);
        hub.broadcast(vec![recovery.clone()]);
        assert_eq!(
            timeout(Duration::from_secs(2), read_frame(&mut slow))
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            recovery
        );
        assert_eq!(
            timeout(Duration::from_secs(2), read_frame(&mut fast))
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            recovery
        );
        for task in writers {
            task.abort();
            let _ = task.await;
        }
    }
    #[tokio::test]
    async fn bounded_broadcast_drops_newest_without_blocking_other_clients() {
        let hub = LoginHub::default();
        let (slow_tx, mut slow) = mpsc::channel(DELIVERY_QUEUE_CAPACITY);
        let (fast_tx, mut fast) = mpsc::channel(DELIVERY_QUEUE_CAPACITY);
        for (key, tx) in [(1, slow_tx), (2, fast_tx)] {
            hub.state.lock().unwrap().clients.insert(
                key,
                Client {
                    gate: key,
                    session: Some(Entry {
                        id: key,
                        tx,
                        client_key: key,
                        broadcast: true,
                    }),
                },
            );
        }
        for n in 0..40u8 {
            hub.broadcast(vec![vec![n]]);
            assert_eq!(fast.recv().await.unwrap().packets, vec![vec![n]]);
        }
        assert_eq!(slow.len(), DELIVERY_QUEUE_CAPACITY);
        for n in 0..DELIVERY_QUEUE_CAPACITY as u8 {
            assert_eq!(slow.recv().await.unwrap().packets, vec![vec![n]]);
        }
        assert!(slow.try_recv().is_err());
        hub.broadcast(vec![vec![99]]);
        assert_eq!(slow.recv().await.unwrap().packets, vec![vec![99]]);
    }
    #[tokio::test]
    async fn close_job_detached_missing_anonymous_and_duplicate() {
        use crate::daemon::UnavailableDaemon;
        let hub = Arc::new(LoginHub::default());
        let (tx, mut rx) = mpsc::channel(8);
        hub.insert(10, 1, tx.clone());
        hub.handle(
            13,
            &compose("hqq", &[Value::I16(13), Value::U64(10), Value::U64(20)]).unwrap(),
            &UnavailableDaemon,
            &[0; 17],
        )
        .await
        .unwrap();
        rx.recv().await.unwrap();
        hub.state
            .lock()
            .unwrap()
            .clients
            .get_mut(&20)
            .unwrap()
            .session = None;
        assert!(!hub.has_session(20));
        hub.close_gate(10).await;
        assert!(!hub.state.lock().unwrap().gates.contains_key(&10));
        assert_eq!(hub.gate_for_client(20), Some(10));
        assert!(rx.try_recv().is_err());
        hub.close_gate(10).await;
        hub.close_gate(999).await;
        hub.insert(11, 2, tx);
        hub.close_gate(11).await;
        assert!(rx.recv().await.unwrap().close);
        hub.close_gate(11).await;
        assert!(rx.try_recv().is_err());
    }
    #[tokio::test]
    async fn missing_entries_collision_failed_start_and_session_cleanup() {
        use crate::daemon::UnavailableDaemon;
        let hub = Arc::new(LoginHub::default());
        let guard = hub.guard(1);
        let (tx, mut rx) = mpsc::channel(8);
        hub.insert(10, 1, tx.clone());
        hub.insert(11, 1, tx);
        let ok = compose("hqq", &[Value::I16(13), Value::U64(10), Value::U64(20)]).unwrap();
        hub.handle(13, &ok, &UnavailableDaemon, &[0; 17])
            .await
            .unwrap();
        let fail = rx.recv().await.unwrap();
        assert_eq!(
            decompose("hh", &fail.packets[0], Default::default()).unwrap(),
            vec![Value::I16(8), Value::I16(2)]
        );
        assert!(hub.by_client(20).is_some()); // failed Start send keeps both entries
        assert!(!hub.broadcast_enabled(20));
        hub.enable_broadcast(20);
        assert!(hub.broadcast_enabled(20));
        let collision = compose("hqq", &[Value::I16(13), Value::U64(11), Value::U64(21)]).unwrap();
        hub.handle(13, &collision, &UnavailableDaemon, &[0; 17])
            .await
            .unwrap();
        assert!(rx.recv().await.is_some());
        assert!(hub.by_client(21).is_none());
        assert!(hub
            .handle(
                13,
                &compose("h", &[Value::I16(13)]).unwrap(),
                &UnavailableDaemon,
                &[0; 17]
            )
            .await
            .is_err());
        let missing = compose("hqE", &[Value::I16(14), Value::U64(999), Value::I16(19)]).unwrap();
        hub.handle(14, &missing, &UnavailableDaemon, &[0; 17])
            .await
            .unwrap();
        assert!(rx.try_recv().is_err());
        let fail = compose("hqE", &[Value::I16(17), Value::U64(20), Value::I16(19)]).unwrap();
        hub.handle(17, &fail, &UnavailableDaemon, &[0; 17])
            .await
            .unwrap();
        assert!(rx.recv().await.is_some());
        assert!(hub.by_client(20).is_none());
        assert!(!hub.state.lock().unwrap().gates.contains_key(&10));
        drop(guard);
        assert!(hub.state.lock().unwrap().gates.is_empty());
        assert!(hub.state.lock().unwrap().clients.is_empty());
    }
    #[test]
    fn every_named_failure_and_unmapped_quirks() {
        for (reason, mapped) in [(15, 10), (16, 11), (17, 12), (18, 13)] {
            assert_eq!(map_reason(11, reason), mapped);
        }
        for (reason, mapped) in [(19, 14), (20, 15), (21, 16)] {
            assert_eq!(map_reason(14, reason), mapped);
        }
        for cmd in [8, 11, 14, 17] {
            for reason in [0, 2, 7, 15, 16, 17, 18, 19, 20, 21, 234] {
                if !(cmd == 11 && (15..=18).contains(&reason)
                    || cmd == 14 && (19..=21).contains(&reason))
                {
                    assert_eq!(map_reason(cmd, reason), reason);
                }
            }
        }
    }
}
