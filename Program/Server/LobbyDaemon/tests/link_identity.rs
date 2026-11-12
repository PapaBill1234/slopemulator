//! Hello identity checks on the GameDaemon link (`exploit_gamed_link_trusted`).
//! Spec: `Docs/Server/LobbyDaemon/07-gamed-link.md` G4 (Peer discovery, Hello, Admission).
use lobbydaemon::{
    character::CreateCharacter,
    gamed::GameDaemon,
    gamed_link::{Consume, LinkGameDaemon, LinkPeer},
};
use packet::gamed_link::{Message, PACKET_VERSION};
use shared::net::FrameReader;
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::{tcp::OwnedReadHalf, tcp::OwnedWriteHalf, TcpListener, TcpStream},
    time::timeout,
};

const LIMIT: Duration = Duration::from_secs(5);

fn peer(world: i16, cluster: i16, address: Ipv4Addr) -> LinkPeer {
    LinkPeer {
        world,
        cluster,
        address: address.into(),
    }
}

async fn start(peers: Vec<LinkPeer>) -> (Arc<LinkGameDaemon>, SocketAddr) {
    let link = Arc::new(LinkGameDaemon::new(peers));
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let consume: Consume = Arc::new(|_, _, _, _| Box::pin(async { Err(15) }));
    tokio::spawn(link.clone().serve(listener, consume));
    (link, address)
}

struct Gamed {
    reader: FrameReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

impl Gamed {
    async fn connect(address: SocketAddr) -> Self {
        let (reader, writer) = TcpStream::connect(address).await.unwrap().into_split();
        Self {
            reader: FrameReader::new(reader, usize::from(u16::MAX)),
            writer,
        }
    }
    async fn send(&mut self, message: Message) {
        let frame = message.encode().unwrap();
        self.writer.write_all(&frame).await.unwrap();
    }
    /// Next message, or None when LobbyDaemon closed the link.
    async fn next(&mut self) -> Option<Message> {
        match timeout(LIMIT, self.reader.next()).await.unwrap() {
            Ok(Some(frame)) => Message::decode(&frame).unwrap(),
            _ => None,
        }
    }
    async fn hello(&mut self, cluster: i16, world: i16) -> Option<Message> {
        self.send(Message::Hello {
            cluster,
            world,
            version: PACKET_VERSION,
            port: 0,
        })
        .await;
        self.next().await
    }
    async fn open(&mut self, cluster: i16, world: i16) {
        assert_eq!(self.hello(cluster, world).await, Some(Message::HelloOk));
        self.send(Message::WorldLive { cluster, world }).await;
    }
}

async fn wait_live(link: &LinkGameDaemon, world: i16) {
    timeout(LIMIT, async {
        while !link.live(world) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn hello_unknown_world_is_refused_and_closed() {
    let (link, address) = start(vec![peer(1, 1, Ipv4Addr::LOCALHOST)]).await;
    let mut gamed = Gamed::connect(address).await;
    assert_eq!(gamed.hello(1, 2).await, Some(Message::HelloFail(1)));
    assert_eq!(gamed.next().await, None);
    assert!(!link.live(2));
}

#[tokio::test]
async fn hello_wrong_cluster_is_refused() {
    let (_link, address) = start(vec![peer(1, 1, Ipv4Addr::LOCALHOST)]).await;
    let mut gamed = Gamed::connect(address).await;
    assert_eq!(gamed.hello(9, 1).await, Some(Message::HelloFail(1)));
    assert_eq!(gamed.next().await, None);
}

#[tokio::test]
async fn hello_wrong_peer_address_is_refused() {
    // World 1 is configured for another host. The loopback peer does not match.
    let (_link, address) = start(vec![peer(1, 1, Ipv4Addr::new(10, 0, 0, 7))]).await;
    let mut gamed = Gamed::connect(address).await;
    assert_eq!(gamed.hello(1, 1).await, Some(Message::HelloFail(1)));
    assert_eq!(gamed.next().await, None);
}

#[tokio::test]
async fn empty_registry_refuses_every_hello() {
    let (_link, address) = start(Vec::new()).await;
    let mut gamed = Gamed::connect(address).await;
    assert_eq!(gamed.hello(1, 1).await, Some(Message::HelloFail(1)));
}

#[tokio::test]
async fn second_gamed_cannot_take_a_live_world() {
    let (link, address) = start(vec![peer(1, 1, Ipv4Addr::LOCALHOST)]).await;
    let mut first = Gamed::connect(address).await;
    first.open(1, 1).await;
    wait_live(&link, 1).await;
    let mut rogue = Gamed::connect(address).await;
    assert_eq!(rogue.hello(1, 1).await, Some(Message::HelloFail(12)));
    assert_eq!(rogue.next().await, None);
    // The first channel still owns world 1.
    assert!(link.live(1));
    let create = {
        let link = link.clone();
        tokio::spawn(async move { link.create_character(request(1), 5, vec![1, 2, 3]).await })
    };
    let Some(Message::Create { transaction, .. }) = first.next().await else {
        panic!("create must go to the first GameDaemon");
    };
    first
        .send(Message::CreateFail {
            transaction,
            reason: 33,
        })
        .await;
    assert_eq!(create.await.unwrap().unwrap_err(), 33);
}

#[tokio::test]
async fn world_is_free_again_after_close() {
    let (link, address) = start(vec![peer(1, 1, Ipv4Addr::LOCALHOST)]).await;
    let mut first = Gamed::connect(address).await;
    first.open(1, 1).await;
    wait_live(&link, 1).await;
    drop(first);
    timeout(LIMIT, async {
        loop {
            let mut next = Gamed::connect(address).await;
            match next.hello(1, 1).await {
                Some(Message::HelloOk) => break,
                _ => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn create_reply_from_another_world_is_ignored() {
    let (link, address) = start(vec![
        peer(1, 1, Ipv4Addr::LOCALHOST),
        peer(2, 1, Ipv4Addr::LOCALHOST),
    ])
    .await;
    let mut one = Gamed::connect(address).await;
    one.open(1, 1).await;
    let mut two = Gamed::connect(address).await;
    two.open(1, 2).await;
    wait_live(&link, 1).await;
    wait_live(&link, 2).await;
    let create = {
        let link = link.clone();
        tokio::spawn(async move { link.create_character(request(1), 5, vec![1, 2, 3]).await })
    };
    let Some(Message::Create { transaction, .. }) = one.next().await else {
        panic!("create must go to world 1");
    };
    // World 2 tries to answer the transaction of world 1.
    two.send(Message::CreateFail {
        transaction,
        reason: 44,
    })
    .await;
    one.send(Message::CreateFail {
        transaction,
        reason: 33,
    })
    .await;
    assert_eq!(create.await.unwrap().unwrap_err(), 33);
}

#[tokio::test]
async fn second_hello_on_one_channel_closes_it() {
    let (_link, address) = start(vec![
        peer(1, 1, Ipv4Addr::LOCALHOST),
        peer(2, 1, Ipv4Addr::LOCALHOST),
    ])
    .await;
    let mut gamed = Gamed::connect(address).await;
    assert_eq!(gamed.hello(1, 1).await, Some(Message::HelloOk));
    assert_eq!(gamed.hello(1, 2).await, None);
}

fn request(world: i16) -> CreateCharacter {
    CreateCharacter {
        account: [0; 16],
        slot: [0; 16],
        world,
        name: "Test".encode_utf16().collect(),
        right: 0,
    }
}
