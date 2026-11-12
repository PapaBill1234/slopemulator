//! LobbyDaemon ↔ GameDaemon link, both Rust sides, on loopback.
//! With `GAMED_MSSQL_URL` set, the create goes to `BlGame` and the test deletes its rows.
use gamedaemon::{db::Db, link::Link};
use lobbydaemon::{
    character::CreateCharacter,
    gamed::GameDaemon,
    gamed_link::{Consume, LinkGameDaemon, LinkPeer},
};
use shared::fsd::APPEARANCE_BYTES;
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

/// Make-PC record: format `cccNc`, symbols 0 race, 1 sex, 2 job, 3 appearance.
fn make_pc(race: u8) -> Vec<u8> {
    let format = b"cccNc";
    let mut data = Vec::new();
    for (id, value) in [(0u16, race), (1, 2), (2, 1)] {
        data.extend(id.to_le_bytes());
        data.push(value);
    }
    data.extend(3u16.to_le_bytes());
    data.extend((APPEARANCE_BYTES as u16).to_le_bytes());
    data.extend([5u8; APPEARANCE_BYTES]);
    let mut out = ((4 + format.len() + data.len()) as u16)
        .to_le_bytes()
        .to_vec();
    out.extend((format.len() as u16).to_le_bytes());
    out.extend(format);
    out.extend(data);
    out
}

fn request() -> CreateCharacter {
    CreateCharacter {
        account: [9; 16],
        slot: [1; 16],
        world: 1,
        name: "Zzlinktest".encode_utf16().collect(),
        right: 0,
    }
}

#[tokio::test]
async fn create_and_enter() {
    const PCID: u64 = 900_002;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let lobby = Arc::new(LinkGameDaemon::new(vec![LinkPeer {
        world: 1,
        cluster: 1,
        address: std::net::Ipv4Addr::LOCALHOST.into(),
    }]));
    let consume: Consume = Arc::new(|key, token, _, world| {
        Box::pin(async move {
            if (key, token, world) == (11, 22, 1) {
                Ok(PCID)
            } else {
                Err(15)
            }
        })
    });
    tokio::spawn(lobby.clone().serve(listener, consume));
    // No live world: reason 2.
    assert_eq!(
        lobby.create_character(request(), PCID, make_pc(1)).await,
        Err(2)
    );
    let db = match Db::from_env().unwrap() {
        Some(mut db) => {
            db.prepare().await.unwrap();
            Some(Arc::new(db))
        }
        None => None,
    };
    let game = Link::new(1, 1, 10241, db.clone());
    tokio::spawn(game.clone().run(address));
    for _ in 0..100 {
        if lobby.live(1) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(lobby.live(1));
    // Race 12 is out of range: reason 25.
    assert_eq!(
        lobby.create_character(request(), PCID, make_pc(12)).await,
        Err(25)
    );
    let created = lobby.create_character(request(), PCID, make_pc(1)).await;
    match &db {
        None => assert_eq!(created, Err(2)),
        Some(db) => {
            let created = created.unwrap();
            assert_eq!((created.job, created.level), (1, 1));
            assert!(!created.showcase.is_empty());
            let pc = db.load(PCID).await.unwrap().unwrap();
            assert_eq!((pc.pc.race, pc.pc.sex, pc.pc.job), (1, 2, 1));
            assert_eq!(pc.pc.appearance, [5; APPEARANCE_BYTES]);
            assert_eq!(pc.name, request().name);
            assert_eq!((pc.zone, pc.level, pc.hp), (1101, 1, 303));
            assert_eq!(pc.position, [-3199, 9241, 599]);
            // A second create of the same PC id fails in the procedure.
            assert!(lobby
                .create_character(request(), PCID, make_pc(1))
                .await
                .is_err());
            db.remove_test_rows(PCID).await.unwrap();
            assert_eq!(db.load(PCID).await.unwrap(), None);
        }
    }
    assert_eq!(game.enter(11, 22, 0).await, Ok(PCID));
    assert_eq!(game.enter(11, 23, 0).await, Err(15));
}
