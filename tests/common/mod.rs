#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_muxwf"))
}

pub fn temp_home(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("muxwf-{test_name}-{}-{nanos}", std::process::id()))
}

pub fn run(home: &PathBuf, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .env("HOME", home)
        .output()
        .unwrap()
}

pub fn run_with_path(home: &PathBuf, path: &str, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .env("HOME", home)
        .env("PATH", path)
        .output()
        .unwrap()
}

pub fn cleanup_home(home: PathBuf) {
    match fs::remove_dir_all(&home) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("failed to remove {}: {error}", home.display()),
    }
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}
