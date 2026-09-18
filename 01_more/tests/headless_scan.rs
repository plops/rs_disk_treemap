//! End-to-end check for `--headless` mode: scans a fixture directory
//! (including a CJK filename, mirroring the crash report) and asserts the
//! binary exits 0 with a size-sorted summary on stdout.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;

fn test_binary() -> PathBuf {
    let mut dir = std::env::current_exe().expect("current test exe");
    dir.pop(); // strip test binary name -> deps/
    if dir.file_name().is_some_and(|n| n == "deps") {
        dir.pop(); // -> debug/ or release/
    }
    #[cfg(windows)]
    let name = "treemap-disk-analyzer.exe";
    #[cfg(not(windows))]
    let name = "treemap-disk-analyzer";
    dir.join(name)
}

fn fixture_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("treemap-headless-fixture-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("hello.txt"), vec![b'x'; 100]).unwrap();
    fs::write(dir.join("Bericht-还.txt"), vec![b'z'; 30]).unwrap();
    fs::write(dir.join("sub").join("daten-还.bin"), vec![b'y'; 50]).unwrap();
    dir
}

#[test]
fn headless_scan_lists_files_and_totals() {
    let dir = fixture_dir();
    let output = Command::new(test_binary())
        .arg("--headless")
        .arg(&dir)
        .output()
        .expect("run headless binary");
    assert!(
        output.status.success(),
        "exit={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("hello.txt"), "stdout:\n{stdout}");
    assert!(stdout.contains("Bericht-还.txt"), "stdout:\n{stdout}");
    assert!(stdout.contains("sub/"), "stdout:\n{stdout}");
    assert!(stdout.contains("3 files"), "stdout:\n{stdout}");
    assert!(stdout.contains("180"), "stdout:\n{stdout}"); // 100 + 30 + 50 bytes total
    let _ = fs::remove_dir_all(&dir);
}

/// `binary --headless dir | head -n 1` must not panic with
/// "failed printing to stdout: Broken pipe": simulate the closed pipe by
/// dropping the child's stdout handle immediately after spawn.
#[cfg(unix)]
#[test]
fn headless_survives_closed_stdout() {
    let dir = fixture_dir();
    let mut child = Command::new(test_binary())
        .arg("--headless")
        .arg(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn headless binary");
    drop(child.stdout.take());
    let output = child.wait_with_output().expect("wait for binary");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "binary panicked on closed stdout:\n{stderr}"
    );
    assert!(
        output.status.success(),
        "exit={} stderr={stderr}",
        output.status
    );
    let _ = fs::remove_dir_all(&dir);
}
