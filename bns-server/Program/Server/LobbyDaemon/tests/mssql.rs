#![cfg(feature = "mssql")]
//! Operator-only SQL tests. No connection is made when the environment is unset.
//! All SQL text is fixed. All fixture values use bound RPC parameters.
#[path = "end_to_end.rs"]
mod existing_gate;

use bloodlib::packet::{compose, Value};
use lobbydaemon::{
    gamed::{ChargeMode, Eligibility, StubGameDaemon, WorldFixture},
    jobs::{Jobs, Settings, State},
    mssql::MssqlStore,
    np::StubNp,
    protocol as w,
    store::{Account, AccountExp, AccountId, ServiceDetail, ServiceKind, Store},
    Server,
};
use lobbygate::{daemon::TcpLobbyDaemon, server::Server as Gate, wire::read_frame};
use std::{net::Ipv4Addr, sync::Arc, time::Duration};
use tiberius::{Client, Config, Row, ToSql, Uuid};
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::{Builder, Runtime},
    sync::watch,
};
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

type SqlClient = Client<Compat<TcpStream>>;
struct Fixture {
    runtime: Runtime,
    client: SqlClient,
    accounts: [AccountId; 2],
    owned: Vec<AccountId>,
}
impl Fixture {
    fn new() -> Option<(Self, MssqlStore)> {
        let Some(url) = std::env::var_os("LOBBYD_MSSQL_URL") else {
            eprintln!("SKIP SQL Server integration: LOBBYD_MSSQL_URL is unset");
            return None;
        };
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let config = Config::from_ado_string(&url.to_string_lossy())
            .expect("invalid ADO settings; details withheld");
        let client = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(10), async {
                let tcp = TcpStream::connect(config.get_addr())
                    .await
                    .expect("fixture connection failed");
                Client::connect(config, tcp.compat_write())
                    .await
                    .expect("fixture authentication failed; details withheld")
            })
            .await
            .expect("fixture connection timeout")
        });
        let accounts = [rand::random(), rand::random()];
        let mut fixture = Self {
            runtime,
            client,
            accounts,
            owned: Vec::new(),
        };
        let store = fixture
            .runtime
            .block_on(MssqlStore::from_env())
            .expect("SQL Store startup failed; details withheld");
        for id in accounts {
            assert!(fixture
                .runtime
                .block_on(store.load_game_account(id))
                .expect("account preflight failed")
                .is_none());
        }
        // Track only preflight-absent IDs. The guard exists before each write.
        for id in accounts {
            fixture.owned.push(id);
            fixture
                .runtime
                .block_on(store.register_game_account(
                    Account {
                        id,
                        name: generated_name(id).encode_utf16().collect(),
                        right: 1,
                        representative_pcid: 0,
                        representative_world: 0,
                        bot_score: 0,
                    },
                    0,
                ))
                .expect("generated registration failed");
        }
        Some((fixture, store))
    }
    fn query(&mut self, sql: &'static str, params: &[&dyn ToSql]) -> Vec<Row> {
        self.runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(10), async {
                self.client
                    .query(sql, params)
                    .await
                    .expect("fixture SQL failed; details withheld")
                    .into_results()
                    .await
                    .expect("fixture fetch failed; details withheld")
                    .into_iter()
                    .flatten()
                    .collect()
            })
            .await
            .expect("fixture query timeout")
        })
    }
    fn character(&mut self, state: u8, blob_len: usize) -> u64 {
        let account = Uuid::from_bytes_le(self.accounts[0]);
        let slot = Uuid::from_bytes_le(self.accounts[0]);
        let name = generated_name(rand::random());
        let rows = self.query("INSERT INTO dbo.[Character] (GameAccountID, SlotID, WorldID, Job, [Level], CharacterName, PureCharacterName, CharacterRight, CharacterState, RegistrationTime, LastStateUpdateTime, JumpingCharacterState) OUTPUT INSERTED.PCID VALUES (@P1, @P2, 1, 1, 1, @P3, @P3, 1, @P4, GETDATE(), GETDATE(), 0)", &[&account, &slot, &name, &state]);
        let pcid = rows[0].get::<i64, _>(0).unwrap();
        self.query(
            "INSERT INTO dbo.CharacterData (PCID, ShowcaseData) VALUES (@P1, @P2)",
            &[&pcid, &vec![7u8; blob_len]],
        );
        pcid as u64
    }
    fn np(&self, pcid: u64) -> (StubNp, AccountId, AccountId, AccountId, AccountId) {
        let (mut np, token, portal, mut identity) = StubNp::fixture(42);
        identity.account = self.accounts[0];
        np.identities.insert((token, portal), Ok(identity));
        let (_, token2, portal2, mut identity2) = StubNp::fixture(43);
        identity2.account = self.accounts[1];
        np.identities.insert((token2, portal2), Ok(identity2));
        np.world_results.insert(
            (self.accounts[0], pcid, self.accounts[0]),
            Ok(ChargeMode::Paid),
        );
        (np, token, portal, token2, portal2)
    }
}
fn generated_name(id: AccountId) -> String {
    // 32 ASCII letters. No source account name or token is used.
    id.into_iter()
        .flat_map(|v| [char::from(b'A' + (v >> 4)), char::from(b'A' + (v & 15))])
        .collect()
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Deletes are limited to the two new, preflight-absent account GUIDs.
        // No TRUNCATE, identity reset, or global log deletion is used.
        for id in self.owned.iter().copied() {
            let id = Uuid::from_bytes_le(id);
            let result = self.runtime.block_on(async {
                tokio::time::timeout(Duration::from_secs(10), async {
                    self.client.query("DELETE d FROM dbo.CharacterCosmesis d JOIN dbo.[Character] c ON c.PCID=d.PCID WHERE c.GameAccountID=@P1; DELETE d FROM dbo.ChangeCharacterSex d JOIN dbo.[Character] c ON c.PCID=d.PCID WHERE c.GameAccountID=@P1; DELETE d FROM dbo.ChangeCharacterRace d JOIN dbo.[Character] c ON c.PCID=d.PCID WHERE c.GameAccountID=@P1; DELETE d FROM dbo.CharacterData d JOIN dbo.[Character] c ON c.PCID=d.PCID WHERE c.GameAccountID=@P1; DELETE d FROM dbo.CharacterLifeStory d JOIN dbo.[Character] c ON c.PCID=d.PCID WHERE c.GameAccountID=@P1; DELETE FROM dbo.[Character] WHERE GameAccountID=@P1; DELETE FROM dbo.GameAccountExp WHERE GameAccountID=@P1; DELETE FROM dbo.GameAccountTimeBasedPlayInfo WHERE GameAccountID=@P1; DELETE FROM dbo.JumpingCharacterCreationQuota WHERE GameAccountID=@P1; DELETE FROM dbo.GameAccount WHERE GameAccountID=@P1; SELECT COUNT(*) AS Remaining FROM dbo.GameAccount WHERE GameAccountID=@P1", &[&id]).await?.into_results().await
                }).await
            });
            let cleaned = match result {
                Ok(Ok(sets)) => {
                    sets.last()
                        .and_then(|rows| rows.first())
                        .and_then(|r| r.try_get::<i32, _>(0).ok().flatten())
                        == Some(0)
                }
                _ => false,
            };
            if !cleaned {
                eprintln!("SQL FIXTURE CLEANUP FAILED; inspect generated test rows");
                if !std::thread::panicking() {
                    panic!("SQL fixture cleanup failed");
                }
            }
        }
    }
}

#[test]
fn sql_character_lifecycle_outputs_and_cleanup() {
    use lobbydaemon::character::{CreateCharacter, CreatedCharacter};
    let Some((mut f, store)) = Fixture::new() else {
        return;
    };
    let account = f.accounts[0];
    let request = |slot| CreateCharacter {
        account,
        slot,
        world: 1,
        name: generated_name(rand::random()).encode_utf16().collect(),
        right: 1,
    };
    let pending = request(rand::random());
    f.runtime
        .block_on(store.check_character_name(&pending.name))
        .unwrap();
    let cancelled = f
        .runtime
        .block_on(store.try_character_creation(pending))
        .unwrap();
    f.runtime
        .block_on(store.cancel_character_creation(cancelled))
        .unwrap();
    let pcid = f
        .runtime
        .block_on(store.try_character_creation(request(rand::random())))
        .unwrap();
    f.runtime
        .block_on(store.finish_character_creation(
            pcid,
            CreatedCharacter {
                job: 1,
                level: 1,
                showcase: vec![7; 512],
            },
        ))
        .unwrap();
    let state = |f: &mut Fixture, pcid: u64| {
        let id = pcid as i64;
        f.query(
            "SELECT CharacterState FROM dbo.[Character] WHERE PCID=@P1",
            &[&id],
        )[0]
        .get::<u8, _>(0)
        .unwrap()
    };
    assert_eq!(state(&mut f, pcid), 2);
    f.runtime
        .block_on(store.try_character_deletion(pcid))
        .unwrap();
    assert_eq!(state(&mut f, pcid), 3);
    f.runtime
        .block_on(store.cancel_character_deletion(pcid))
        .unwrap();
    assert_eq!(state(&mut f, pcid), 2);
    for _ in 0..2 {
        f.runtime
            .block_on(store.try_character_deletion(pcid))
            .unwrap();
        let timestamp = f
            .runtime
            .block_on(store.finish_character_deletion(pcid, 3600))
            .unwrap()
            .unwrap();
        assert!((1..=12).contains(&timestamp.month));
        assert_eq!(state(&mut f, pcid), 4);
        f.runtime
            .block_on(store.try_character_restoration(pcid))
            .unwrap();
        assert_eq!(state(&mut f, pcid), 5);
        f.runtime
            .block_on(store.finish_character_restoration(pcid))
            .unwrap();
        assert_eq!(state(&mut f, pcid), 2);
    }
    f.runtime
        .block_on(store.try_character_deletion(pcid))
        .unwrap();
    assert!(f
        .runtime
        .block_on(store.finish_character_deletion(pcid, 0))
        .unwrap()
        .is_none());
    assert_eq!(state(&mut f, pcid), 6);
    // The guard deletes generated character data, life stories and accounts.
    // Do not provoke procedure errors that can write unscoped error-log rows.
}

#[test]
fn sql_account_characters_logout_optional_and_blob_boundaries() {
    let Some((mut f, store)) = Fixture::new() else {
        return;
    };
    let id = f.accounts[0];
    assert_eq!(
        f.runtime
            .block_on(store.load_game_account(id))
            .unwrap()
            .unwrap()
            .id,
        id
    );
    f.runtime.block_on(store.update_last_login(id, 0)).unwrap();
    f.runtime
        .block_on(store.update_last_logout(id, 29))
        .unwrap();
    assert_eq!(
        f.runtime
            .block_on(store.load_game_account(id))
            .unwrap()
            .unwrap()
            .bot_score,
        29
    );
    assert!(f
        .runtime
        .block_on(store.load_incomplete_lobby_service(id))
        .unwrap()
        .is_empty());
    assert!(f
        .runtime
        .block_on(store.load_character_list(id))
        .unwrap()
        .is_empty());
    assert!(f
        .runtime
        .block_on(store.load_account_exp(id))
        .unwrap()
        .is_none());
    let exp = AccountExp {
        exp: 307,
        quota_per_day: 17,
        last_update: 381,
    };
    f.runtime
        .block_on(store.insert_account_exp(id, exp.clone()))
        .unwrap();
    assert_eq!(
        f.runtime.block_on(store.load_account_exp(id)).unwrap(),
        Some(exp)
    );
    assert!(f
        .runtime
        .block_on(store.load_jumping_creation_quota(id))
        .unwrap()
        .is_none());
    let guid = Uuid::from_bytes_le(id);
    f.query("INSERT INTO dbo.JumpingCharacterCreationQuota (GameAccountID, CreationQuota, CreationCount) VALUES (@P1, 3, 1)", &[&guid]);
    assert_eq!(
        f.runtime
            .block_on(store.load_jumping_creation_quota(id))
            .unwrap()
            .unwrap()
            .quota,
        3
    );
    assert_eq!(
        f.runtime
            .block_on(store.load_time_based_play_info(id))
            .unwrap()
            .len(),
        2
    );
    let pcid = f.character(2, 0) as i64;
    // All four conflicting blob columns use the same generated boundary lengths.
    let service = pcid;
    f.query("INSERT INTO dbo.CharacterCosmesis (ServiceID, PCID, NewAppearanceData, CosmesisState, RegistrationTime, LastStateUpdateTime) VALUES (@P1, @P1, @P2, 1, GETDATE(), GETDATE()); INSERT INTO dbo.ChangeCharacterSex (ServiceID, PCID, Sex, NewAppearanceData, ChangeState, RegistrationTime, LastStateUpdateTime) VALUES (@P1, @P1, 1, @P2, 1, GETDATE(), GETDATE()); INSERT INTO dbo.ChangeCharacterRace (ServiceID, PCID, Race, Sex, NewAppearanceData, ChangeState, RegistrationTime, LastStateUpdateTime) VALUES (@P1, @P1, 1, 1, @P2, 1, GETDATE(), GETDATE())", &[&service, &vec![0u8; 0]]);
    for length in [0, 511, 512, 513, 4096] {
        let bytes = vec![7u8; length];
        f.query("UPDATE dbo.CharacterData SET ShowcaseData=@P2 WHERE PCID=@P1; UPDATE dbo.CharacterCosmesis SET NewAppearanceData=@P2 WHERE PCID=@P1; UPDATE dbo.ChangeCharacterSex SET NewAppearanceData=@P2 WHERE PCID=@P1; UPDATE dbo.ChangeCharacterRace SET NewAppearanceData=@P2 WHERE PCID=@P1", &[&pcid, &bytes]);
        let list = f.runtime.block_on(store.load_character_list(id));
        if length <= 512 {
            assert_eq!(list.unwrap()[0].showcase, bytes);
        } else {
            assert_eq!(list.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
        }
        for kind in [ServiceKind::Cosmesis, ServiceKind::Sex, ServiceKind::Race] {
            let detail = f
                .runtime
                .block_on(store.load_service_detail(service as u64, kind));
            if length > 512 {
                assert_eq!(detail.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
                continue;
            }
            let appearance = match detail.unwrap() {
                ServiceDetail::Cosmesis { appearance, .. }
                | ServiceDetail::Sex { appearance, .. }
                | ServiceDetail::Race { appearance, .. } => appearance,
                _ => panic!("wrong detail kind"),
            };
            assert_eq!(appearance, bytes);
        }
    }
    // Do not force a SQL failure here. The recreated catch path writes an
    // unscoped error log. Pure unit tests cover fail-closed return handling.
}

async fn dispatch(
    j: &mut Jobs<MssqlStore, StubNp>,
    format: &str,
    fields: &[Value],
) -> Vec<Vec<u8>> {
    j.handle(1, &w::packet(format, fields).unwrap())
        .await
        .unwrap()
        .into_iter()
        .flat_map(|o| o.packets)
        .collect()
}
async fn login(j: &mut Jobs<MssqlStore, StubNp>, t: AccountId, p: AccountId) -> u64 {
    dispatch(
        j,
        "hhhh",
        &[Value::I16(1), Value::I16(0), Value::I16(58), Value::I16(0)],
    )
    .await;
    let init = dispatch(
        j,
        "hqooi",
        &[
            Value::I16(6),
            Value::U64(100),
            Value::Id(t),
            Value::Id(p),
            Value::U32(123),
        ],
    )
    .await;
    let Value::U64(key) = w::fields("hqqoo", &init[0]).unwrap()[2] else {
        panic!("key")
    };
    let start = dispatch(
        j,
        "hqoo",
        &[Value::I16(9), Value::U64(key), Value::Id(t), Value::Id(p)],
    )
    .await;
    assert_eq!(w::command(&start[0]).unwrap(), 10);
    key
}
fn game(pcid: u64, slot: AccountId, eligibility: Eligibility) -> Arc<StubGameDaemon> {
    let mut game = StubGameDaemon::default();
    game.worlds.insert(
        (1, pcid),
        WorldFixture {
            slot,
            eligibility: Ok(eligibility),
            restore: Ok(()),
            admission: Ok(None),
        },
    );
    Arc::new(game)
}
#[test]
fn sql_retained_login_world_restoration_and_kick_score() {
    let Some((mut f, store)) = Fixture::new() else {
        return;
    };
    f.runtime
        .block_on(store.update_last_logout(f.accounts[0], 29))
        .unwrap();
    let pcid = f.character(5, 512);
    let (np, t, p, _, _) = f.np(pcid);
    let slot = f.accounts[0];
    let game = game(pcid, slot, Eligibility::Restore);
    let mut j = Jobs::new(
        store,
        np,
        Settings {
            world_token_lifetime: Some(Duration::from_secs(10)),
            ..Settings::default()
        },
    )
    .unwrap()
    .with_game_daemon(game.clone());
    let key = f.runtime.block_on(login(&mut j, t, p));
    let retained = j.retain(key, Duration::from_secs(10)).unwrap();
    let account_guid = Uuid::from_bytes_le(f.accounts[0]);
    f.query(
        "UPDATE dbo.GameAccount SET BotScore=-17 WHERE GameAccountID=@P1",
        &[&account_guid],
    );
    // Change persisted data. D15/D35 must still use the login cache.
    let pcid_sql = pcid as i64;
    f.query(
        "UPDATE dbo.CharacterData SET ShowcaseData=@P2 WHERE PCID=@P1",
        &[&pcid_sql, &vec![9u8; 513]],
    );
    f.runtime.block_on(async {
        let init = dispatch(
            &mut j,
            "hqqqi",
            &[
                Value::I16(12),
                Value::U64(101),
                Value::U64(key),
                Value::U64(retained),
                Value::U32(123),
            ],
        )
        .await;
        assert_eq!(w::command(&init[0]).unwrap(), 13);
        let start = dispatch(&mut j, "hq", &[Value::I16(15), Value::U64(key)]).await;
        assert_eq!(w::command(&start[0]).unwrap(), 16);
        let cached = dispatch(&mut j, "hq", &[Value::I16(35), Value::U64(key)]).await;
        assert_eq!(w::command(&cached[0]).unwrap(), 36);
        let entry = dispatch(
            &mut j,
            "hqo",
            &[Value::I16(24), Value::U64(key), Value::Id(slot)],
        )
        .await;
        let fields = w::fields("hqqh", &entry[0]).unwrap();
        assert_eq!(fields[0], Value::I16(25));
        let Value::U64(token) = fields[2] else {
            panic!("world token")
        };
        assert_eq!(j.consume_world(key, token, 123, 1), Ok(pcid));
        assert_eq!(j.state(key), Some(State::World));
    });
    assert_eq!(game.restored(), 1);
    let rows = f.query(
        "SELECT CharacterState, DeletionDelayTime FROM dbo.[Character] WHERE PCID=@P1",
        &[&pcid_sql],
    );
    assert_eq!(rows[0].get::<u8, _>(0), Some(2));
    assert!(rows[0]
        .get::<tiberius::time::chrono::NaiveDateTime, _>(1)
        .is_none());
    f.runtime.block_on(j.kick(key, 7)).unwrap();
    assert_eq!(j.session_count(), 0);
    assert_eq!(
        f.runtime
            .block_on(j.store.load_game_account(f.accounts[0]))
            .unwrap()
            .unwrap()
            .bot_score,
        29
    );
}

#[test]
fn sql_existing_encrypted_gate_flow() {
    let Some((mut f, store)) = Fixture::new() else {
        return;
    };
    let pcid = f.character(2, 512);
    let (np, t, p, t2, p2) = f.np(pcid);
    let slot = f.accounts[0];
    let game = game(pcid, slot, Eligibility::Ready);
    let account = f.accounts[0];
    f.runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(20), async {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let cfg = existing_gate::config(listener.local_addr().unwrap().port());
            let lobby = Arc::new(
                Server::new(
                    store,
                    np,
                    Settings {
                        world_token_lifetime: Some(Duration::from_secs(10)),
                        ..Settings::default()
                    },
                )
                .unwrap()
                .with_game_daemon(game),
            );
            let (stop, shutdown) = watch::channel(false);
            let lobby_task = tokio::spawn(lobby.clone().serve(listener, shutdown.clone()));
            let daemon = TcpLobbyDaemon::start(&cfg, shutdown.clone()).await.unwrap();
            let (public, private, _) = existing_gate::keys(1024);
            let gate = Arc::new(Gate::new(&cfg, private, daemon).unwrap());
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let address = listener.local_addr().unwrap();
            let gate_task = tokio::spawn(gate.serve(listener, shutdown));
            let (mut client, mut crypt) = existing_gate::connect_client(address, &public).await;
            existing_gate::send(
                &mut client,
                &mut crypt,
                compose("hoo", &[Value::I16(3), Value::Id(t), Value::Id(p)]).unwrap(),
            )
            .await;
            let login =
                existing_gate::receive(&mut client, &mut crypt, 4, "qQoihCiiiii;CiiQiqq;QqiqiqK")
                    .await;
            assert_eq!(login[2], Value::Id(account));
            let Value::U64(key) = login[0] else {
                panic!("key")
            };
            existing_gate::send(
                &mut client,
                &mut crypt,
                compose("h", &[Value::I16(27)]).unwrap(),
            )
            .await;
            let cached = existing_gate::receive(&mut client, &mut crypt, 28, "qhCqohsbqQQi;").await;
            let Value::Repeat(rows) = &cached[2] else {
                panic!("characters")
            };
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0][0], Value::U64(pcid));
            assert_eq!(rows[0][4], Value::Bytes(vec![7; 512]));
            let retained = lobby.retain(key, Duration::from_secs(10)).await.unwrap();
            drop(client);
            let (mut client, mut crypt) = existing_gate::connect_client(address, &public).await;
            existing_gate::send(
                &mut client,
                &mut crypt,
                compose(
                    "hqq",
                    &[Value::I16(6), Value::U64(key), Value::U64(retained)],
                )
                .unwrap(),
            )
            .await;
            assert_eq!(
                existing_gate::receive(&mut client, &mut crypt, 7, "qCiiiii;CiiQiqq;iqiqK").await
                    [0],
                Value::U64(key)
            );
            existing_gate::send(
                &mut client,
                &mut crypt,
                compose("ho", &[Value::I16(13), Value::Id(slot)]).unwrap(),
            )
            .await;
            let result = existing_gate::receive(&mut client, &mut crypt, 14, "q").await;
            let Value::U64(token) = result[0] else {
                panic!("token")
            };
            assert_eq!(
                lobby
                    .consume_world(
                        key,
                        token,
                        u32::from_le_bytes(Ipv4Addr::LOCALHOST.octets()),
                        1
                    )
                    .await,
                Ok(pcid)
            );
            lobby.kick(key, 7).await.unwrap();
            assert_eq!(
                existing_gate::receive(&mut client, &mut crypt, 10, "Q").await,
                vec![Value::I8(7)]
            );
            assert!(read_frame(&mut client).await.unwrap().is_none());
            let (mut client, mut crypt) = existing_gate::connect_client(address, &public).await;
            existing_gate::send(
                &mut client,
                &mut crypt,
                compose("hoo", &[Value::I16(3), Value::Id(t2), Value::Id(p2)]).unwrap(),
            )
            .await;
            existing_gate::receive(&mut client, &mut crypt, 4, "qQoihCiiiii;CiiQiqq;QqiqiqK").await;
            existing_gate::send(
                &mut client,
                &mut crypt,
                compose("h", &[Value::I16(12)]).unwrap(),
            )
            .await;
            assert!(read_frame(&mut client).await.unwrap().is_none());
            assert_eq!(lobby.session_count().await, 0);
            stop.send(true).unwrap();
            gate_task.await.unwrap().unwrap();
            lobby_task.await.unwrap().unwrap();
        })
        .await
        .unwrap();
    });
}
