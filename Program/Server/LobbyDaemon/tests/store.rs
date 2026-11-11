//! Generated persistence fixtures. No SQL connection is used.
use lobbydaemon::store::{Account, MemoryStore, SyncStore};

fn account() -> Account {
    Account {
        id: [73; 16],
        name: "generated-store-account".encode_utf16().collect(),
        right: 1,
        representative_pcid: 0,
        representative_world: 0,
        bot_score: 3,
    }
}

#[test]
fn logout_score_survives_a_fresh_account_load() {
    let mut store = MemoryStore::default();
    let account = account();
    store.register_game_account(account.clone(), 0).unwrap();
    store.update_last_logout(account.id, 29).unwrap();
    let loaded = store.load_game_account(account.id).unwrap().unwrap();
    assert_eq!(loaded.bot_score, 29);
    assert_eq!(store.logout_updates, vec![(account.id, 29)]);
    assert_eq!(loaded.name, account.name);
    assert_eq!(loaded.right, account.right);
}

#[test]
fn missing_account_updates_fail_without_recording_success() {
    let mut store = MemoryStore::default();
    let id = [74; 16];
    assert!(store.update_last_login(id, 0).is_err());
    assert!(store.update_last_logout(id, 31).is_err());
    assert!(store.login_updates.is_empty());
    assert!(store.logout_updates.is_empty());
    assert!(store.load_game_account(id).unwrap().is_none());
}
