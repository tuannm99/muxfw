mod common;

use common::{cleanup_home, run, run_with_path, stderr, stdout, temp_home};
use std::fs;

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
fn import_session_creates_work_and_snapshot_from_named_tmux_session() {
    let home = temp_home("import-session");
    let fake_bin_dir = home.join("bin");
    let works_dir = home.join(".muxwf/works");
    let snapshots_dir = home.join(".muxwf/snapshots");
    fs::create_dir_all(&fake_bin_dir).unwrap();
    fs::create_dir_all(&works_dir).unwrap();
    fs::create_dir_all(&snapshots_dir).unwrap();

    let tmux_script = fake_bin_dir.join("tmux");
    fs::write(
        &tmux_script,
        "#!/bin/sh\nif [ \"$1\" = \"has-session\" ] && [ \"$3\" = \"adhoc\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"display-message\" ]; then\n  printf '0\\n'\n  exit 0\nfi\nif [ \"$1\" = \"list-windows\" ] && [ \"$3\" = \"adhoc\" ]; then\n  printf '0\tmain\tlayout\t1\\n'\n  exit 0\nfi\nif [ \"$1\" = \"list-panes\" ]; then\n  printf '0\t1\t/tmp/adhoc\\n'\n  exit 0\nfi\nexit 1\n",
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
    let output = run_with_path(
        &home,
        &path,
        &["work", "import-session", "adhoc", "--name", "adhoc-work"],
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let work_yaml = fs::read_to_string(works_dir.join("adhoc-work.yaml")).unwrap();
    assert!(work_yaml.contains("name: adhoc-work"));
    assert!(work_yaml.contains("session: adhoc"));

    let snapshot_json = fs::read_to_string(snapshots_dir.join("adhoc-work.json")).unwrap();
    assert!(snapshot_json.contains("\"session_name\": \"adhoc\""));

    cleanup_home(home);
}

#[test]
fn save_updates_usage_metrics() {
    let home = temp_home("save-metrics");
    let works_dir = home.join(".muxwf/works");
    let fake_bin_dir = home.join("bin");
    fs::create_dir_all(&works_dir).unwrap();
    fs::create_dir_all(&fake_bin_dir).unwrap();

    fs::write(
        works_dir.join("api.yaml"),
        "name: api\nsession: api-session\nroot: /tmp\non_restore: \"\"\n",
    )
    .unwrap();

    let tmux_script = fake_bin_dir.join("tmux");
    fs::write(
        &tmux_script,
        "#!/bin/sh\nif [ \"$1\" = \"has-session\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"display-message\" ]; then\n  printf '0\\n'\n  exit 0\nfi\nif [ \"$1\" = \"list-windows\" ]; then\n  printf '0\tmain\tlayout\t1\\n'\n  exit 0\nfi\nif [ \"$1\" = \"list-panes\" ]; then\n  printf '0\t1\t/tmp\\n'\n  exit 0\nfi\nexit 1\n",
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
    let save = run_with_path(&home, &path, &["save", "api"]);
    assert!(save.status.success(), "stderr:\n{}", stderr(&save));

    let work_yaml = fs::read_to_string(works_dir.join("api.yaml")).unwrap();
    assert!(work_yaml.contains("save_count: 1"));
    assert!(work_yaml.contains("last_saved_at:"));

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
