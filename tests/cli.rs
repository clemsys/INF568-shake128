use assert_cmd::Command;
use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake128,
};
use std::{fs, str};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn dies_no_args() -> TestResult {
    let mut cmd = Command::cargo_bin("shake128")?;
    cmd.assert().failure();
    Ok(())
}

#[test]
fn dies_two_args() -> TestResult {
    let mut cmd = Command::cargo_bin("shake128")?;
    cmd.args(["32", "16"]).assert().failure();
    Ok(())
}

#[test]
fn dies_arg_not_number() -> TestResult {
    let mut cmd = Command::cargo_bin("shake128")?;
    cmd.args(["32a"]).assert().failure();
    Ok(())
}

#[test]
fn dies_arg_negative() -> TestResult {
    let mut cmd = Command::cargo_bin("shake128")?;
    cmd.args(["-32"]).assert().failure();
    Ok(())
}

#[test]
fn runs_arg_uint() -> TestResult {
    let mut cmd = Command::cargo_bin("shake128")?;
    cmd.args(["32"]).assert().success();
    Ok(())
}

fn bytes_to_string(bytes: &[u8]) -> String {
    let mut s = String::new();
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn run(n: usize, input_file: &str) -> TestResult {
    let mut cmd = Command::cargo_bin("shake128")?;
    let input_bytes = fs::read(input_file)?;

    let mut hasher = Shake128::default();
    hasher.update(&input_bytes);
    let mut reader = hasher.finalize_xof();
    let mut expected_bytes = vec![0u8; n];
    reader.read(&mut expected_bytes);
    let expected = bytes_to_string(&expected_bytes);

    cmd.args([n.to_string()])
        .write_stdin(input_bytes)
        .assert()
        .success()
        .stdout(expected);
    Ok(())
}

#[test]
fn correct_short_text() -> TestResult {
    run(32, "tests/samples/short-text.txt")
}

#[test]
fn correct_short_binary() -> TestResult {
    run(32, "tests/samples/short-binary.bin")
}
