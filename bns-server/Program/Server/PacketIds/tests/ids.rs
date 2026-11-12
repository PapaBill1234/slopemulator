use packet_ids::{
    generate, generate_lobbyd, generated_dir, is_valid_ident, sanitize, workspace_root,
    AccountInventoryLobbydCmd, AcquireSkillBuildUpPointReason, GameCmd, GameLobbydCmd, GenError,
    InfoGateCmd, LobbyCmd, LobbyGateLobbydCmd, StsCmd,
};

#[test]
fn generated_is_current() {
    let root = workspace_root();
    let (commands, enums) = generate(&root).expect("generator runs");
    let dir = generated_dir(&root);
    let old_commands = std::fs::read_to_string(dir.join("commands.rs")).expect("commands.rs");
    let old_enums = std::fs::read_to_string(dir.join("enums.rs")).expect("enums.rs");
    assert!(
        commands == old_commands,
        "commands.rs is old: run `cargo run -q -p packet-ids --bin gen-ids`"
    );
    assert!(
        enums == old_enums,
        "enums.rs is old: run `cargo run -q -p packet-ids --bin gen-ids`"
    );
    let lobbyd = generate_lobbyd(&root).expect("lobbyd generator runs");
    let old_lobbyd = std::fs::read_to_string(dir.join("lobbyd.rs")).expect("lobbyd.rs");
    assert!(
        lobbyd == old_lobbyd,
        "lobbyd.rs is old: run `cargo run -q -p packet-ids --bin gen-ids`"
    );
}

#[test]
fn lobbyd_names_from_table() {
    assert_eq!(
        LobbyGateLobbydCmd::from_id(9),
        Some(LobbyGateLobbydCmd::StartSessionWithNPTokenReq)
    );
    assert_eq!(
        LobbyGateLobbydCmd::from_id(24),
        Some(LobbyGateLobbydCmd::GetEnterWorldTokenReq)
    );
    assert_eq!(
        LobbyGateLobbydCmd::from_id(28),
        Some(LobbyGateLobbydCmd::GetWaitInfoReq)
    );
    assert_eq!(LobbyGateLobbydCmd::GetWaitInfoReq.id(), 28);
    assert_eq!(LobbyGateLobbydCmd::GetWaitInfoReq.name(), "GetWaitInfoReq");
    assert_eq!(LobbyGateLobbydCmd::COUNT, 35);
}

#[test]
fn lobbyd_unverified_absent() {
    // Only the rows with a verified name are in the enums.
    assert_eq!(GameLobbydCmd::COUNT, 15);
    assert_eq!(AccountInventoryLobbydCmd::COUNT, 3);
    // 4 is `ServiceOpenNotify` in the table, but its name is not verified.
    assert_eq!(GameLobbydCmd::from_id(4), None);
    assert_eq!(GameLobbydCmd::from_id(1), Some(GameLobbydCmd::HelloReq));
    assert_eq!(AccountInventoryLobbydCmd::from_id(2), None);
}

#[test]
fn game_hello_ids() {
    assert_eq!(GameCmd::G_HelloReqCmd.id(), 11);
    assert_eq!(GameCmd::from_id(11), Some(GameCmd::G_HelloReqCmd));
    assert_eq!(GameCmd::G_HelloReqCmd.name(), "G_HelloReqCmd");
    assert_eq!(GameCmd::from_id(-1), None);
}

#[test]
fn lobby_hello_id() {
    assert_eq!(LobbyCmd::from_id(1), Some(LobbyCmd::L_HelloReq));
}

#[test]
fn sts_name_kept() {
    let cmd = StsCmd::from_id(1001).expect("1001 is in the table");
    assert_eq!(cmd.name(), "Sts/Connect");
    assert_eq!(cmd, StsCmd::Sts_Connect);
}

#[test]
fn infogate_hello_id() {
    assert_eq!(InfoGateCmd::from_id(1), Some(InfoGateCmd::I_HelloReqCmd));
}

#[test]
fn names_are_cleaned() {
    assert_eq!(sanitize("Sts/Connect"), "Sts_Connect");
    assert_eq!(sanitize("Ui::TransitState"), "Ui__TransitState");
    assert!(is_valid_ident("G_HelloReqCmd"));
    assert!(!is_valid_ident("1abc"));
    assert!(!is_valid_ident(""));
    assert!(!is_valid_ident("type"));
}

#[test]
fn generator_stops_on_duplicate_id() {
    let root = std::env::temp_dir().join(format!("packet-ids-dup-{}", std::process::id()));
    let re = root.join("Docs/Client/RE");
    std::fs::create_dir_all(&re).expect("temp dir");
    let table = "id\tname\tdirection\n1\tA_Cmd\tC2S\n1\tB_Cmd\tS2C\n";
    for file in [
        "game-commands.tsv",
        "lobby-commands.tsv",
        "03-sts-commands.tsv",
        "12-infogate-commands.tsv",
    ] {
        std::fs::write(re.join(file), table).expect("write table");
    }
    let result = generate(&root);
    let _ = std::fs::remove_dir_all(&root);
    assert!(matches!(result, Err(GenError::DuplicateId { id: 1, .. })));
}

#[test]
fn enum_const() {
    assert_eq!(
        AcquireSkillBuildUpPointReason::G_ACQUIRESKILLBUILDUPPOINTREASON_DEBUG,
        0
    );
}
