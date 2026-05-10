use super::*;

#[test]
fn binary_path_uses_exe_suffix_for_current_platform() {
    let path = binary_path(Path::new("/some/root"));
    let expected_name = format!("{BINARY_NAME}{}", std::env::consts::EXE_SUFFIX);
    assert_eq!(
        path.file_name().and_then(|s| s.to_str()),
        Some(expected_name.as_str())
    );
    let path_str = path.to_string_lossy();
    assert!(
        path_str.contains("target") && path_str.contains("release"),
        "path should include target/release: {path_str}"
    );
}

#[test]
fn workspace_root_resolves_to_parent_of_xtask_manifest() {
    let root = workspace_root();
    assert!(
        root.join(README_FILENAME).exists(),
        "workspace root should contain README.md"
    );
    assert!(
        root.join("xtask").is_dir(),
        "workspace root should contain xtask/"
    );
}

#[test]
fn make_temp_dir_creates_unique_directory_under_temp() {
    let a = make_temp_dir().unwrap();
    let b = make_temp_dir().unwrap();
    assert!(a.exists() && a.is_dir());
    assert!(b.exists() && b.is_dir());
    assert_ne!(a, b);
    let parent = std::env::temp_dir();
    assert!(a.starts_with(&parent));
    assert!(b.starts_with(&parent));
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn error_display_messages_mention_cause() {
    let e = Error::QuickStartMissing;
    assert!(e.to_string().contains("Quick Start"));
    let e = Error::NoInitCommand;
    assert!(e.to_string().contains(BINARY_NAME));
    let e = Error::BinaryMissing(PathBuf::from("/x/y"));
    assert!(e.to_string().contains("/x/y"));
    let e = Error::BuildFailed {
        stderr: "oops".to_string(),
    };
    assert!(e.to_string().contains("oops"));
    let e = Error::CommandFailed {
        command: "cmd".to_string(),
        stderr: "boom".to_string(),
        status: Some(2),
    };
    assert!(e.to_string().contains("boom"));
    let e = Error::Io(io::Error::other("io"));
    assert!(e.to_string().contains("io"));
    let e = Error::ReadmeRead(io::Error::other("missing"));
    assert!(e.to_string().contains("missing"));
}

#[test]
fn execute_init_writes_redirect_file_with_stdout_payload() {
    use std::io::Write as _;

    let dir = make_temp_dir().unwrap();
    let stub_dir = dir.join("stub");
    fs::create_dir_all(&stub_dir).unwrap();

    #[cfg(windows)]
    let stub_path = {
        let p = stub_dir.join("stub.bat");
        let mut f = File::create(&p).unwrap();
        writeln!(f, "@echo off").unwrap();
        writeln!(f, "echo repo = \"dummy/dummy\"").unwrap();
        p
    };
    #[cfg(not(windows))]
    let stub_path = {
        use std::os::unix::fs::PermissionsExt as _;
        let p = stub_dir.join("stub.sh");
        let mut f = File::create(&p).unwrap();
        writeln!(f, "#!/bin/sh").unwrap();
        writeln!(f, "echo 'repo = \"dummy/dummy\"'").unwrap();
        let mut perms = fs::metadata(&p).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&p, perms).unwrap();
        p
    };

    let cmd = ParsedCommand {
        args: vec!["gitless-sync".to_string()],
        redirect_to: Some("out.toml".to_string()),
    };

    execute_init(&stub_path, &cmd, &dir).unwrap();

    let written = fs::read_to_string(dir.join("out.toml")).unwrap();
    assert!(
        written.contains("repo = \"dummy/dummy\""),
        "redirect target should contain stub stdout: {written:?}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn execute_init_returns_command_failed_when_binary_exits_nonzero() {
    let dir = make_temp_dir().unwrap();
    let stub_dir = dir.join("stub");
    fs::create_dir_all(&stub_dir).unwrap();

    #[cfg(windows)]
    let stub_path = {
        let p = stub_dir.join("stub.bat");
        let mut f = File::create(&p).unwrap();
        writeln!(f, "@echo off").unwrap();
        writeln!(f, "echo failed 1>&2").unwrap();
        writeln!(f, "exit /b 7").unwrap();
        p
    };
    #[cfg(not(windows))]
    let stub_path = {
        use std::os::unix::fs::PermissionsExt as _;
        let p = stub_dir.join("stub.sh");
        let mut f = File::create(&p).unwrap();
        writeln!(f, "#!/bin/sh").unwrap();
        writeln!(f, "echo failed 1>&2").unwrap();
        writeln!(f, "exit 7").unwrap();
        let mut perms = fs::metadata(&p).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&p, perms).unwrap();
        p
    };

    let cmd = ParsedCommand {
        args: vec!["gitless-sync".to_string()],
        redirect_to: None,
    };

    let result = execute_init(&stub_path, &cmd, &dir);
    let err = result.unwrap_err();
    match err {
        Error::CommandFailed { stderr, status, .. } => {
            assert!(stderr.contains("failed"), "stderr captured: {stderr}");
            assert_eq!(status, Some(7));
        }
        other => panic!("expected CommandFailed, got: {other:?}"),
    }

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn parsed_command_used_directly_in_module_executor_path() {
    let cmd = ParsedCommand {
        args: vec!["gitless-sync".to_string(), "init".to_string()],
        redirect_to: Some("foo.toml".to_string()),
    };
    assert_eq!(cmd.args.len(), 2);
    assert_eq!(cmd.redirect_to.as_deref(), Some("foo.toml"));
}
