use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_muxwf"))
}

fn temp_home(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("muxwf-{test_name}-{}-{nanos}", std::process::id()))
}

fn run(home: &PathBuf, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .env("HOME", home)
        .output()
        .unwrap()
}

fn cleanup_home(home: PathBuf) {
    match fs::remove_dir_all(&home) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("failed to remove {}: {error}", home.display()),
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn work_create_and_list_json_use_isolated_home() {
    let home = temp_home("work-create-list");
    let create = run(
        &home,
        &[
            "work",
            "create",
            "api",
            "--root",
            "/tmp",
            "--tag",
            "backend",
            "--group",
            "platform",
            "--description",
            "API service",
        ],
    );
    assert!(
        create.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&create),
        stderr(&create)
    );

    let list = run(&home, &["list", "--json"]);
    assert!(
        list.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&list),
        stderr(&list)
    );
    let out = stdout(&list);
    assert!(out.contains("\"name\": \"api\""));
    assert!(out.contains("\"group\": \"platform\""));
    assert!(out.contains("\"backend\""));

    cleanup_home(home);
}

#[test]
fn short_add_still_creates_named_work() {
    let home = temp_home("add-named");
    let add = run(
        &home,
        &["add", "cli", "--root", "/tmp", "--description", "CLI"],
    );
    assert!(
        add.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&add),
        stderr(&add)
    );

    let names = run(&home, &["list", "--names-only"]);
    assert!(
        names.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&names),
        stderr(&names)
    );
    assert_eq!(stdout(&names).trim(), "cli");

    cleanup_home(home);
}

#[test]
fn add_current_rejects_manual_session_override_before_tmux_lookup() {
    let home = temp_home("add-current-session-override");
    let output = run(&home, &["add", "current", "--session", "manual"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("--session is not valid with `muxwf add current`"));

    cleanup_home(home);
}

#[test]
fn completion_command_generates_shell_script() {
    let home = temp_home("completion");
    let bash = run(&home, &["completion", "bash"]);
    assert!(
        bash.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&bash),
        stderr(&bash)
    );
    assert!(stdout(&bash).contains("complete"));

    let zsh = run(&home, &["completion", "zsh"]);
    assert!(
        zsh.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&zsh),
        stderr(&zsh)
    );
    assert!(stdout(&zsh).contains("#compdef mw"));

    let muxwf_zsh = run(&home, &["completion", "zsh", "--name", "muxwf"]);
    assert!(
        muxwf_zsh.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&muxwf_zsh),
        stderr(&muxwf_zsh)
    );
    assert!(stdout(&muxwf_zsh).contains("#compdef muxwf"));

    cleanup_home(home);
}
