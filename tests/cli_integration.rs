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

fn run_with_path(home: &PathBuf, path: &str, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .env("HOME", home)
        .env("PATH", path)
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
    assert!(stdout(&zsh).contains("_muxwf_work_names"));
    assert!(stdout(&zsh).contains("list --names-only"));

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

#[test]
fn version_command_prints_package_version() {
    let home = temp_home("version");
    let output = run(&home, &["version"]);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stdout(&output).contains(env!("CARGO_PKG_VERSION")));

    cleanup_home(home);
}

#[test]
fn workspace_short_alias_lists_workspaces() {
    let home = temp_home("workspace-alias");
    let workspaces_dir = home.join(".muxwf/workspaces");
    fs::create_dir_all(&workspaces_dir).unwrap();
    fs::write(
        workspaces_dir.join("suite.yaml"),
        "name: suite\nworks:\n  - api\n",
    )
    .unwrap();

    let output = run(&home, &["ws", "list"]);
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert_eq!(stdout(&output).trim(), "suite\tapi");

    cleanup_home(home);
}

#[test]
fn workspace_create_update_and_list_json_work() {
    let home = temp_home("workspace-create-update");

    let create = run(
        &home,
        &[
            "workspace",
            "create",
            "suite",
            "--work",
            "api",
            "--work",
            "web",
        ],
    );
    assert!(
        create.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&create),
        stderr(&create)
    );

    let names = run(&home, &["ws", "list", "--names-only"]);
    assert!(
        names.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&names),
        stderr(&names)
    );
    assert_eq!(stdout(&names).trim(), "suite");

    let json = run(&home, &["workspace", "list", "--json"]);
    assert!(
        json.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&json),
        stderr(&json)
    );
    let out = stdout(&json);
    assert!(out.contains("\"name\": \"suite\""));
    assert!(out.contains("\"api\""));
    assert!(out.contains("\"web\""));

    let update = run(
        &home,
        &[
            "workspace",
            "update",
            "suite",
            "--work",
            "api",
            "--work",
            "worker",
        ],
    );
    assert!(
        update.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&update),
        stderr(&update)
    );

    let listed = run(&home, &["ws", "list"]);
    assert!(
        listed.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&listed),
        stderr(&listed)
    );
    assert_eq!(stdout(&listed).trim(), "suite\tapi,worker");

    cleanup_home(home);
}

#[test]
fn workspace_add_remove_and_delete_work() {
    let home = temp_home("workspace-members");

    let create = run(
        &home,
        &[
            "workspace",
            "create",
            "suite",
            "--work",
            "api",
            "--work",
            "web",
        ],
    );
    assert!(
        create.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&create),
        stderr(&create)
    );

    let add = run(
        &home,
        &[
            "workspace",
            "add",
            "suite",
            "--work",
            "jobs",
            "--work",
            "web",
        ],
    );
    assert!(
        add.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&add),
        stderr(&add)
    );

    let remove = run(&home, &["workspace", "remove", "suite", "--work", "web"]);
    assert!(
        remove.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&remove),
        stderr(&remove)
    );

    let listed = run(&home, &["workspace", "list"]);
    assert!(
        listed.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&listed),
        stderr(&listed)
    );
    assert_eq!(stdout(&listed).trim(), "suite\tapi,jobs");

    let remove_all = run(
        &home,
        &[
            "workspace",
            "remove",
            "suite",
            "--work",
            "api",
            "--work",
            "jobs",
        ],
    );
    assert!(!remove_all.status.success());
    assert!(stderr(&remove_all).contains("would become empty"));

    let delete = run(&home, &["ws", "delete", "suite"]);
    assert!(
        delete.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&delete),
        stderr(&delete)
    );

    let listed_after_delete = run(&home, &["workspace", "list", "--names-only"]);
    assert!(
        listed_after_delete.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&listed_after_delete),
        stderr(&listed_after_delete)
    );
    assert!(stdout(&listed_after_delete).trim().is_empty());

    cleanup_home(home);
}

#[test]
fn rm_kills_tmux_session_before_deleting_work() {
    let home = temp_home("rm-kills-session");
    let works_dir = home.join(".muxwf/works");
    let snapshots_dir = home.join(".muxwf/snapshots");
    let fake_bin_dir = home.join("bin");
    fs::create_dir_all(&works_dir).unwrap();
    fs::create_dir_all(&snapshots_dir).unwrap();
    fs::create_dir_all(&fake_bin_dir).unwrap();

    fs::write(
        works_dir.join("api.yaml"),
        "name: api\nsession: api-session\nroot: /tmp\non_restore: \"\"\n",
    )
    .unwrap();
    fs::write(snapshots_dir.join("api.json"), "{}").unwrap();

    let tmux_log = home.join("tmux.log");
    let tmux_script = fake_bin_dir.join("tmux");
    fs::write(
        &tmux_script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\nif [ \"$1\" = \"has-session\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"kill-session\" ]; then\n  exit 0\nfi\nexit 1\n",
            tmux_log.display()
        ),
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&tmux_script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmux_script, perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let path = format!("{}:{}", fake_bin_dir.display(), current_path);
    let output = run_with_path(&home, &path, &["rm", "api"]);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(!works_dir.join("api.yaml").exists());
    assert!(!snapshots_dir.join("api.json").exists());

    let tmux_calls = fs::read_to_string(&tmux_log).unwrap();
    assert!(tmux_calls.contains("has-session -t api-session"));
    assert!(tmux_calls.contains("kill-session -t api-session"));
    assert!(stdout(&output).contains("killed session 'api-session'"));

    cleanup_home(home);
}
