use std::process::{Command, Stdio};
use std::io::Write;

#[test]
fn test_main_binary() {
    let mut child = Command::new("target/debug/Prokopakop")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start binary");

    // Write to stdin
    let stdin = child.stdin.as_mut().expect("Failed to open stdin");
    stdin.write_all(b"quit\n").expect("Failed to write to stdin");
    stdin.flush().expect("Failed to flush stdin");

    // Read output
    let output = child.wait_with_output().expect("Failed to read output");

    assert_eq!(String::from_utf8_lossy(&output.stdout), "expected output\n");
    assert!(output.status.success());
}
