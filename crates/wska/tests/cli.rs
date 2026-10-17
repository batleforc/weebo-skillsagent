use std::io::Read;
use std::process::{Command, Stdio};

#[test]
fn a_closed_stdout_ends_the_process_without_a_panic() {
    // completions needs no items repo and prints far more than a pipe buffer.
    let mut child = Command::new(env!("CARGO_BIN_EXE_wska"))
        .args(["completions", "bash"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    child.wait().unwrap();
    assert!(!stderr.contains("panicked"), "{stderr}");
}
