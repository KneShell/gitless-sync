use super::*;
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

fn small_args(out: PathBuf) -> Args {
    Args {
        out,
        count: 4,
        seed: DEFAULT_SEED,
    }
}

#[test]
fn parse_args_requires_out() {
    let err = parse_args(&[]).unwrap_err();
    assert!(matches!(err, Error::MissingOut));
}

#[test]
fn parse_args_accepts_out_count_seed() {
    let parsed = parse_args(&[
        "--out".into(),
        "tmp/synth-vault-42".into(),
        "--count".into(),
        "10".into(),
        "--seed".into(),
        "7".into(),
    ])
    .unwrap();
    assert_eq!(parsed.out, PathBuf::from("tmp/synth-vault-42"));
    assert_eq!(parsed.count, 10);
    assert_eq!(parsed.seed, 7);
}

#[test]
fn parse_args_rejects_unknown_arg() {
    let err = parse_args(&["--bogus".into()]).unwrap_err();
    assert!(matches!(err, Error::InvalidArg(_)));
}

#[test]
fn parse_args_rejects_bad_count() {
    let err =
        parse_args(&["--out".into(), "x".into(), "--count".into(), "abc".into()]).unwrap_err();
    assert!(matches!(err, Error::InvalidArg(_)));
}

#[test]
fn xorshift_is_deterministic_given_seed() {
    let mut a = Xorshift64::new(42);
    let mut b = Xorshift64::new(42);
    for _ in 0..16 {
        assert_eq!(a.next_u64(), b.next_u64());
    }
}

#[test]
fn xorshift_zero_seed_does_not_lock_to_zero() {
    let mut prng = Xorshift64::new(0);
    let v = prng.next_u64();
    assert_ne!(v, 0);
}

#[test]
fn build_content_uses_lf_only_no_crlf() {
    let mut prng = Xorshift64::new(1);
    let s = build_content("note-00000.md", 4096, &mut prng);
    assert!(s.len() >= 4096);
    assert!(!s.contains('\r'), "content must use LF only, found CR");
}

#[test]
fn build_content_is_pure_ascii_thus_nfc() {
    let mut prng = Xorshift64::new(7);
    let s = build_content("note-00001.md", 2048, &mut prng);
    assert!(s.is_ascii(), "content must be ASCII (trivially NFC)");
}

#[test]
fn generate_writes_count_files_with_ascii_paths() {
    let dir = tempdir().unwrap();
    let args = small_args(dir.path().to_path_buf());
    generate(&args).unwrap();
    let entries: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap())
        .collect();
    assert_eq!(entries.len(), args.count);
    for entry in &entries {
        let name = entry.file_name();
        let s = name.to_str().unwrap();
        assert!(s.is_ascii(), "path must be ASCII: {s}");
        assert!(s.starts_with("note-"));
        let ext = std::path::Path::new(s)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap();
        assert!(ext.eq_ignore_ascii_case("md"));
    }
}

#[test]
fn generate_files_use_lf_newlines_only() {
    let dir = tempdir().unwrap();
    let args = small_args(dir.path().to_path_buf());
    generate(&args).unwrap();
    for entry in fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        let bytes = fs::read(&path).unwrap();
        assert!(
            !bytes.contains(&b'\r'),
            "file must contain no CR: {}",
            path.display()
        );
        assert!(bytes.contains(&b'\n'));
    }
}

#[test]
fn generate_sets_fixed_mtime_epoch() {
    let dir = tempdir().unwrap();
    let args = small_args(dir.path().to_path_buf());
    generate(&args).unwrap();
    let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(FIXED_MTIME_EPOCH_SECS);
    for entry in fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        let actual = fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(
            actual,
            expected,
            "mtime mismatch for {} (expected fixed epoch {})",
            path.display(),
            FIXED_MTIME_EPOCH_SECS
        );
    }
}

#[test]
fn generate_file_sizes_within_min_max_bounds() {
    let dir = tempdir().unwrap();
    let args = small_args(dir.path().to_path_buf());
    generate(&args).unwrap();
    for entry in fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        let len = usize::try_from(fs::metadata(&path).unwrap().len()).unwrap();
        assert!(
            len >= MIN_FILE_BYTES,
            "file {} below MIN_FILE_BYTES: {len}",
            path.display()
        );
        assert!(
            len <= MAX_FILE_BYTES + 256,
            "file {} above MAX_FILE_BYTES (+slack): {len}",
            path.display()
        );
    }
}

#[test]
fn generate_no_case_collision_in_lowercase_keys() {
    let dir = tempdir().unwrap();
    let args = small_args(dir.path().to_path_buf());
    generate(&args).unwrap();
    let mut seen: Vec<String> = Vec::new();
    for entry in fs::read_dir(dir.path()).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().to_lowercase();
        assert!(
            !seen.contains(&name),
            "case-insensitive collision detected: {name}"
        );
        seen.push(name);
    }
}

#[test]
fn generate_is_deterministic_for_same_seed() {
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();
    let args_a = small_args(dir_a.path().to_path_buf());
    let args_b = small_args(dir_b.path().to_path_buf());
    generate(&args_a).unwrap();
    generate(&args_b).unwrap();
    let mut a_files: Vec<_> = fs::read_dir(dir_a.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let mut b_files: Vec<_> = fs::read_dir(dir_b.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    a_files.sort();
    b_files.sort();
    assert_eq!(a_files, b_files);
    for name in &a_files {
        let a_bytes = fs::read(dir_a.path().join(name)).unwrap();
        let b_bytes = fs::read(dir_b.path().join(name)).unwrap();
        assert_eq!(a_bytes, b_bytes, "content diverged for {name:?}");
    }
}

#[test]
fn run_with_no_out_returns_error() {
    let result = run(&[]);
    assert!(matches!(result, Err(Error::MissingOut)));
}

#[test]
fn run_with_unknown_flag_returns_error() {
    let result = run(&["--bogus".into()]);
    assert!(matches!(result, Err(Error::InvalidArg(_))));
}

#[test]
fn run_with_valid_args_writes_files() {
    let dir = tempdir().unwrap();
    let out = dir.path().to_string_lossy().into_owned();
    let result = run(&[
        "--out".into(),
        out,
        "--count".into(),
        "3".into(),
        "--seed".into(),
        "42".into(),
    ]);
    assert_eq!(result.unwrap(), 0);
    let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 3);
}

#[test]
fn error_display_messages_mention_cause() {
    let e = Error::MissingOut;
    assert!(e.to_string().contains("--out"));
    let e = Error::InvalidArg("foo".into());
    assert!(e.to_string().contains("foo"));
}
