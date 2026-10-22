use nxmonitor::config::{ConfigStore, SelectionMode, Settings};
use nxmonitor::target::{BlacklistRule, ProcessIdentity, Selection};

fn process(pid: u32, started_at: u64, name: &str, path: Option<&str>) -> ProcessIdentity {
    ProcessIdentity::new(pid, started_at, name, path.map(str::to_owned))
}

#[test]
fn automatic_follows_foreground_but_rejects_self_system_unknown_and_blacklisted() {
    let first = process(10, 100, "First.exe", Some("C:\\Games\\First.exe"));
    let second = process(11, 101, "Second.exe", Some("C:/Games/Second.exe"));
    let unknown = process(12, 102, "Unknown.exe", None);
    let relative = process(13, 103, "Relative.exe", Some("Games\\Relative.exe"));
    let running = [
        first.clone(),
        second.clone(),
        unknown.clone(),
        relative.clone(),
    ];
    let selection = Selection::new(SelectionMode::Automatic);
    assert_eq!(
        selection.resolve(Some(&first), &running, &[], 99),
        Some(first.clone())
    );
    assert_eq!(
        selection.resolve(Some(&second), &running, &[], 99),
        Some(second.clone())
    );
    assert_eq!(selection.resolve(Some(&unknown), &running, &[], 99), None);
    assert_eq!(selection.resolve(Some(&relative), &running, &[], 99), None);
    assert_eq!(
        selection.resolve(
            Some(&process(99, 10, "NxMonitor.exe", Some("C:\\NxMonitor.exe"))),
            &running,
            &[],
            99
        ),
        None
    );
    assert_eq!(
        selection.resolve(
            Some(&process(0, 1, "Idle", Some("C:\\Idle.exe"))),
            &running,
            &[],
            99
        ),
        None
    );
    assert_eq!(
        selection.resolve(
            Some(&process(4, 1, "System", Some("C:\\System.exe"))),
            &running,
            &[],
            99
        ),
        None
    );
    assert_eq!(
        selection.resolve(
            Some(&first),
            &running,
            &[BlacklistRule::FileName("FIRST.EXE".into())],
            99
        ),
        None
    );
    assert_eq!(
        selection.resolve(
            Some(&second),
            &running,
            &[BlacklistRule::FullPath("c:\\games\\SECOND.exe".into())],
            99
        ),
        None
    );
    assert_eq!(
        selection.resolve(
            Some(&process(10, 200, "First.exe", Some("C:\\Games\\First.exe"))),
            &running,
            &[],
            99
        ),
        None
    );
}

#[test]
fn manual_lock_is_stable_bypasses_blacklist_and_never_reuses_pid() {
    let old = process(22, 1000, "Game.exe", Some("C:\\Games\\Game.exe"));
    let other = process(23, 2000, "Other.exe", Some("C:\\Games\\Other.exe"));
    let mut selection = Selection::new(SelectionMode::Automatic);
    selection.lock(old.clone());
    assert_eq!(
        selection.resolve(
            Some(&other),
            &[old.clone(), other.clone()],
            &[BlacklistRule::FileName("game.exe".into())],
            999
        ),
        Some(old.clone())
    );
    assert_eq!(
        selection.resolve(Some(&other), std::slice::from_ref(&other), &[], 999),
        None
    );
    let reused = process(22, 3000, "Game.exe", Some("C:\\Games\\Game.exe"));
    assert_eq!(
        selection.resolve(Some(&reused), std::slice::from_ref(&reused), &[], 999),
        None
    );
    selection.set_mode(SelectionMode::Automatic);
    assert_eq!(
        selection.resolve(Some(&other), std::slice::from_ref(&other), &[], 999),
        Some(other)
    );
    selection.lock(process(999, 3, "Self", Some("C:\\Self.exe")));
    assert_eq!(
        selection.resolve(
            None,
            &[process(999, 3, "Self", Some("C:\\Self.exe"))],
            &[],
            999
        ),
        None
    );
}

#[test]
fn blacklist_windows_matching_and_validation() {
    let p = process(7, 1, "Display Name", Some("C:/Apps/Game.EXE"));
    assert!(BlacklistRule::FileName("game.exe".into()).matches(&p));
    assert!(BlacklistRule::FileName("display name".into()).matches(&p));
    assert!(BlacklistRule::FullPath("c:\\apps\\GAME.exe".into()).matches(&p));
    assert!(!BlacklistRule::FileName("game".into()).matches(&p));
    assert!(!BlacklistRule::FullPath("C:\\Other\\Game.exe".into()).matches(&p));
    assert!(
        BlacklistRule::FullPath("\\\\server\\share\\game.exe".into())
            .validate()
            .is_ok()
    );
    for invalid in [
        BlacklistRule::FileName("".into()),
        BlacklistRule::FileName("dir/game.exe".into()),
        BlacklistRule::FileName("dir\\game.exe".into()),
        BlacklistRule::FullPath("game.exe".into()),
        BlacklistRule::FullPath("C:game.exe".into()),
        BlacklistRule::FullPath("\\server\\game.exe".into()),
    ] {
        assert!(
            invalid.validate().is_err(),
            "unexpected valid rule: {invalid:?}"
        );
    }
}

#[test]
fn settings_round_trip_and_atomic_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(dir.path().join("nested").join("settings.json"));
    assert_eq!(
        store.path(),
        dir.path().join("nested").join("settings.json")
    );
    assert_eq!(store.load().unwrap(), Settings::default());
    let mut settings = Settings::default();
    settings.mode = SelectionMode::Manual;
    settings.manual_path = Some("C:/Games/Game.exe".into());
    settings.blacklist = vec![BlacklistRule::FileName("overlay.exe".into())];
    settings.english = true;
    settings.graphy = true;
    settings.presentmon_path = "C:\\Tools\\PresentMon.exe".into();
    store.save(&settings).unwrap();
    assert_eq!(store.load().unwrap(), settings);
    // Replacing an existing destination must work on Windows, not just on Unix.
    settings.english = false;
    store.save(&settings).unwrap();
    assert_eq!(store.load().unwrap(), settings);
    let persisted = std::fs::read_to_string(store.path()).unwrap();
    assert!(!persisted.contains("\"pid\""));
    assert!(!persisted.contains("started_at"));
}

#[test]
fn invalid_json_reports_error_without_overwriting_and_invalid_save_preserves_previous() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(dir.path().join("settings.json"));
    std::fs::write(store.path(), "{ invalid json").unwrap();
    assert!(store.load().is_err());
    assert_eq!(
        std::fs::read_to_string(store.path()).unwrap(),
        "{ invalid json"
    );
    store.save(&Settings::default()).unwrap();
    let previous = std::fs::read(store.path()).unwrap();
    let mut invalid = Settings::default();
    invalid.manual_path = Some("relative\\game.exe".into());
    assert!(store.save(&invalid).is_err());
    assert_eq!(std::fs::read(store.path()).unwrap(), previous);
    invalid = Settings::default();
    invalid.version = 2;
    assert!(store.save(&invalid).is_err());
    assert_eq!(std::fs::read(store.path()).unwrap(), previous);
    invalid = Settings::default();
    invalid.blacklist = vec![BlacklistRule::FileName("".into())];
    assert!(store.save(&invalid).is_err());
    assert_eq!(std::fs::read(store.path()).unwrap(), previous);
}

#[test]
fn settings_validation_limits_and_no_silent_manual_relock() {
    let mut settings = Settings::default();
    assert_eq!(settings.version, 1);
    assert_eq!(settings.mode, SelectionMode::Automatic);
    assert_eq!(settings.presentmon_path, "tools\\PresentMon-2.6.0-x64.exe");
    settings.manual_path = Some("\\\\server\\share\\Game.exe".into());
    settings.mode = SelectionMode::Manual;
    assert!(settings.validate().is_ok());
    let selection = Selection::new(settings.mode);
    let game = process(12, 1, "Game.exe", Some("\\\\server\\share\\Game.exe"));
    assert_eq!(
        selection.resolve(Some(&game), std::slice::from_ref(&game), &[], 900),
        None
    );
    settings.presentmon_path.clear();
    assert!(settings.validate().is_err());
    settings.presentmon_path = "x".repeat(32769);
    assert!(settings.validate().is_err());
    settings.presentmon_path = "helper.exe".into();
    settings.blacklist = vec![BlacklistRule::FileName("x".into()); 257];
    assert!(settings.validate().is_err());
}

#[test]
fn utf8_bom_settings_restore_without_changing_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(dir.path().join("settings.json"));
    let mut expected = Settings::default();
    expected.english = true;
    expected.mode = SelectionMode::Manual;
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend(serde_json::to_vec(&expected).unwrap());
    std::fs::write(store.path(), &bytes).unwrap();
    assert_eq!(store.load().unwrap(), expected);
    assert_eq!(std::fs::read(store.path()).unwrap(), bytes);
}
