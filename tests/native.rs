// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
use kirk_companion::{
    launch::{arguments, discover_at},
    model::*,
    storage::DiskStore,
};
use std::path::PathBuf;

fn profile() -> Profile {
    Profile {
        name: "Kirk test".into(),
        target: std::env::current_exe().unwrap(),
        instance: "26.2 (Kirk test)".into(),
        ..Profile::default()
    }
}

#[test]
fn launcher_arguments_are_separate_and_never_a_shell_command() {
    let mut p = profile();
    p.root = Some(std::env::temp_dir().join("portable launcher"));
    let args = arguments(&p);
    assert_eq!(
        args,
        vec![
            "--dir".into(),
            p.root.unwrap().into_os_string(),
            "--launch".into(),
            "26.2 (Kirk test)".into()
        ]
    );
}
#[test]
fn invalid_profiles_cannot_be_executed() {
    for instance in ["../bad", "..", ".", "a/b", "a\\b", "--help", "x\0"] {
        let mut p = profile();
        p.instance = instance.into();
        assert!(p.validate().is_err(), "{instance}");
    }
    for target in [
        "relative.exe",
        "https://evil.example/a.exe",
        "kirk://launch",
    ] {
        let mut p = profile();
        p.target = PathBuf::from(target);
        assert!(p.validate().is_err());
    }
}
#[test]
fn profile_metadata_has_no_paths_arguments_or_credentials() {
    let p = profile();
    let value = serde_json::to_value(p.public()).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 4);
    assert!(value.get("target").is_none());
    assert!(value.get("root").is_none());
    assert_eq!(value["launcher"], "prism");
    assert_eq!(value["capability"], "launch");
}
#[test]
fn open_only_capability_is_explicit() {
    let mut p = profile();
    p.kind = Launcher::Open;
    assert_eq!(p.public().capability, "open");
}
#[test]
fn secure_origin_validation_accepts_only_https_and_literal_loopback() {
    assert_eq!(
        backend_origin("https://example.com/").unwrap(),
        "https://example.com"
    );
    assert!(backend_origin("http://127.0.0.1:1234").is_ok());
    assert!(backend_origin("http://[::1]:1234").is_ok());
    for value in [
        "http://example.com",
        "https://user:pass@example.com",
        "https://example.com/path",
        "https://example.com?token=a",
        "file:///tmp/a",
        "http://localhost.evil.example",
    ] {
        assert!(backend_origin(value).is_err(), "{value}");
    }
}
#[test]
fn pairing_codes_allow_typing_spaces_and_dashes() {
    assert_eq!(pairing_code("acde-fhjk").unwrap(), "ACDEFHJK");
    assert_eq!(pairing_code("ACDE FHJK").unwrap(), "ACDEFHJK");
    assert!(pairing_code("bad").is_err());
    assert!(pairing_code("ACDE!HJK").is_err());
}
#[test]
fn remote_requests_reject_unknown_fields_commands_and_expiry() {
    let id = uuid::Uuid::new_v4();
    let now = now_ms();
    let valid = serde_json::json!({"id":"request_12345678901234","op":"launch_game","profile":id,"expires_at":now+10000});
    let request: LaunchRequest = serde_json::from_value(valid.clone()).unwrap();
    assert!(request.validate(now).is_ok());
    for expires in [now - 1, now, now + 30001] {
        let mut v = valid.clone();
        v["expires_at"] = expires.into();
        assert!(
            serde_json::from_value::<LaunchRequest>(v)
                .unwrap()
                .validate(now)
                .is_err()
        );
    }
    let mut v = valid.clone();
    v["command"] = "evil.exe".into();
    assert!(serde_json::from_value::<LaunchRequest>(v).is_err());
    let mut v = valid;
    v["op"] = "shell".into();
    assert!(
        serde_json::from_value::<LaunchRequest>(v)
            .unwrap()
            .validate(now)
            .is_err()
    );
}
#[test]
fn settings_round_trip_never_stores_pairing_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let store = DiskStore {
        dir: dir.path().into(),
    };
    let mut settings = Settings::default();
    settings.profiles.push(profile());
    settings.pairing_enabled = true;
    store.save(&settings).unwrap();
    let disk = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
    assert!(!disk.contains("token"));
    assert!(!disk.contains("password"));
    assert_eq!(store.load().unwrap().profiles.len(), 1);
    settings.name = "Updated PC".into();
    store.save(&settings).unwrap();
    assert_eq!(store.load().unwrap().name, "Updated PC");
}
#[test]
fn damaged_settings_are_backed_up_before_replacement() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), "broken").unwrap();
    let store = DiskStore {
        dir: dir.path().into(),
    };
    assert!(store.load().is_err());
    store.save(&Settings::default()).unwrap();
    assert!(std::fs::read_dir(dir.path()).unwrap().flatten().any(|p| {
        p.file_name()
            .to_string_lossy()
            .starts_with("settings-damaged-")
    }));
}
#[test]
fn discovery_reads_only_instance_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("instances/26.2(kirk)");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("instance.cfg"), "[General]\nname=My Kirk\n").unwrap();
    let found = discover_at(&[(Launcher::Prism, dir.path().into())]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "My Kirk");
    assert_eq!(found[0].instance, "26.2(kirk)");
}
