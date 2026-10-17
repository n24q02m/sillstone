//! End-to-end tests for the `gen-reference` binary: the generator and
//! verifier modes the CI gate drives, exercised as a real process.

use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

/// A private per-test working directory the binary can run in, so
/// `gen` and `verify` never touch the repository root.
struct WorkDir(std::path::PathBuf);

impl WorkDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "sillstone-digest-cli-{}-{}",
            tag,
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create work dir");
        WorkDir(dir)
    }

    fn run(&self, args: &[&str]) -> (Option<i32>, String, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_sillstone-digest-gen-ref"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .expect("run gen-reference");
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `gen` writes the vectors and `verify` immediately accepts them.
#[test]
fn gen_then_verify_succeeds() {
    let dir = WorkDir::new("roundtrip");
    let (code, stdout, _) = dir.run(&["gen"]);
    assert_eq!(code, Some(0), "gen failed: {stdout}");
    assert!(stdout.contains("wrote reference.json"));
    assert!(dir.0.join("reference.json").is_file());

    let (code, stdout, _) = dir.run(&["verify"]);
    assert_eq!(code, Some(0), "verify failed: {stdout}");
    assert!(stdout.contains("reference vectors are current"));
}

/// A drifted file is rejected with the stale-file diagnosis.
#[test]
fn verify_rejects_a_drifted_file() {
    let dir = WorkDir::new("drift");
    dir.run(&["gen"]);
    let path = dir.0.join("reference.json");
    let bytes = std::fs::read(&path).expect("read generated file");
    let mut drifted = bytes.clone();
    let at = drifted.len() / 2;
    drifted[at] = if drifted[at] == b'0' { b'1' } else { b'0' };
    std::fs::write(&path, drifted).expect("write drifted file");

    let (code, _, stderr) = dir.run(&["verify"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("stale"), "stderr was: {stderr}");
    assert!(stderr.contains("regenerate with"), "stderr was: {stderr}");
}

/// A missing file is reported as unreadable, not as a mismatch.
#[test]
fn verify_reports_a_missing_file() {
    let dir = WorkDir::new("missing");
    let (code, _, stderr) = dir.run(&["verify"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("could not read"), "stderr was: {stderr}");
}

/// Missing or unknown modes print usage and exit with a distinct code.
#[test]
fn bad_usage_exits_with_code_two() {
    let dir = WorkDir::new("usage");
    let (code, _, stderr) = dir.run(&[]);
    assert_eq!(code, Some(2));
    assert!(stderr.contains("usage:"), "stderr was: {stderr}");

    let (code, _, stderr) = dir.run(&["regenerate"]);
    assert_eq!(code, Some(2));
    assert!(stderr.contains("got"), "stderr was: {stderr}");
}
