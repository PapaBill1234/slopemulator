//! Local Store contracts. All bytes and identifiers are generated.
use lobbydaemon::store::{
    AccountExp, Character, MemoryStore, ServiceDetail, ServiceKind, Shared, Store, SyncStore,
};
use std::io;

fn character(size: usize) -> Character {
    Character {
        pcid: 700,
        slot: [81; 16],
        world: 1,
        name: vec![65],
        showcase: vec![0x5a; size],
        deletion_time: None,
        state: lobbydaemon::character::InternalState::Restoring,
        jumping_state: 0,
        last_play_end: 0,
    }
}
fn detail(kind: ServiceKind, size: usize) -> ServiceDetail {
    match kind {
        ServiceKind::Cosmesis => ServiceDetail::Cosmesis {
            pcid: 700,
            appearance: vec![0x6b; size],
        },
        ServiceKind::Sex => ServiceDetail::Sex {
            pcid: 700,
            sex: 1,
            appearance: vec![0x7c; size],
        },
        ServiceKind::Race => ServiceDetail::Race {
            pcid: 700,
            race: 2,
            sex: 1,
            appearance: vec![0x8d; size],
        },
        ServiceKind::Rename => ServiceDetail::Rename {
            pcid: 700,
            old_name: vec![65],
            new_name: vec![66],
        },
        ServiceKind::SlotAddition => ServiceDetail::SlotAddition { slot: [82; 16] },
    }
}
#[test]
fn four_blob_fields_accept_exact_capacity_and_reject_larger_values() {
    for size in [0, 511, 512, 513, 4096] {
        let mut store = MemoryStore::default();
        store.characters.insert([80; 16], vec![character(size)]);
        let result = store.load_character_list([80; 16]);
        if size <= 512 {
            assert_eq!(result.unwrap(), vec![character(size)]);
        } else {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
        }
        for kind in [ServiceKind::Cosmesis, ServiceKind::Sex, ServiceKind::Race] {
            let row = detail(kind, size);
            store.service_details.insert(900, row.clone());
            let result = store.load_service_detail(900, kind);
            if size <= 512 {
                assert_eq!(result.unwrap(), row);
            } else {
                assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
            }
            assert_eq!(store.service_details[&900], row); // Never truncate persisted data.
        }
    }
}
#[test]
fn details_require_matching_explicit_fixtures_and_preserve_pending_rows() {
    let mut store = MemoryStore::default();
    assert_eq!(
        store
            .load_service_detail(900, ServiceKind::Rename)
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
    for kind in [
        ServiceKind::Cosmesis,
        ServiceKind::Sex,
        ServiceKind::Race,
        ServiceKind::Rename,
        ServiceKind::SlotAddition,
    ] {
        let row = detail(kind, 512);
        store.service_details.insert(900, row.clone());
        assert_eq!(store.load_service_detail(900, kind).unwrap(), row);
        assert_eq!(store.service_details[&900], row);
    }
    assert_eq!(
        store
            .load_service_detail(900, ServiceKind::Race)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
}
#[test]
fn optional_loads_are_not_implicit_success_and_exp_insert_is_once() {
    let mut store = MemoryStore::default();
    let id = [83; 16];
    assert!(store.load_time_based_play_info(id).is_err());
    assert!(store.load_jumping_creation_quota(id).is_err());
    assert!(store.load_account_exp(id).is_err());
    store.account_exp.insert(id, None); // Explicit successful no-row result.
    assert_eq!(store.load_account_exp(id).unwrap(), None);
    let row = AccountExp {
        exp: 42,
        quota_per_day: 7,
        last_update: 90,
    };
    store.insert_account_exp(id, row.clone()).unwrap();
    assert_eq!(store.load_account_exp(id).unwrap(), Some(row.clone()));
    assert_eq!(
        store
            .insert_account_exp(id, row.clone())
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    store.time_based_play_info.insert(id, vec![]);
    store.jumping_creation_quota.insert(id, None);
    assert!(store.load_time_based_play_info(id).unwrap().is_empty());
    assert_eq!(store.load_jumping_creation_quota(id).unwrap(), None);
    // The async adapter gives the same results as the model.
    let shared = Shared::new(store);
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            assert_eq!(shared.load_account_exp(id).await.unwrap(), Some(row));
            assert!(shared
                .load_time_based_play_info(id)
                .await
                .unwrap()
                .is_empty());
            assert_eq!(shared.load_jumping_creation_quota(id).await.unwrap(), None);
            assert!(shared
                .load_service_detail(901, ServiceKind::Sex)
                .await
                .is_err());
        });
}
#[test]
fn restoration_commits_cached_fixture_state_once() {
    let mut store = MemoryStore::default();
    store.characters.insert([84; 16], vec![character(512)]);
    assert!(store.finish_character_restoration(700).is_err());
    store.restoration_allowed.insert(700);
    store.finish_character_restoration(700).unwrap();
    assert_eq!(
        store.load_character_list([84; 16]).unwrap()[0].state,
        lobbydaemon::character::InternalState::Normal
    );
    assert!(store.finish_character_restoration(700).is_err());
    assert_eq!(store.restored, vec![700]);
}
