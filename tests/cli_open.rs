mod common;

use common::{bin, cleanup_home, run, run_with_path, stderr, stdout, temp_home};
use std::fs;
use std::process::{Command, Stdio};

#[test]
fn jump_json_returns_ranked_work_rows() {
    let home = temp_home("jump-json");

    let create_api = run(
        &home,
        &[
            "work",
            "create",
            "api",
            "--root",
            "/tmp",
            "--favorite",
            "--description",
            "API service",
        ],
    );
    assert!(
        create_api.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&create_api),
        stderr(&create_api)
    );

    let create_web = run(
        &home,
        &[
            "work",
            "create",
            "web",
            "--root",
            "/tmp",
            "--description",
            "Web UI",
        ],
    );
    assert!(
        create_web.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&create_web),
        stderr(&create_web)
    );

    let output = run(&home, &["jump", "--json"]);
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let out = stdout(&output);
    assert!(out.contains("\"name\": \"api\""));
    assert!(out.contains("\"kind\": \"work\""));
    assert!(out.contains("\"tracked\": true"));
    assert!(out.contains("\"jump_rank\": 0"));
    assert!(out.contains("\"live\": false"));

    cleanup_home(home);
}

#[test]
fn jump_json_includes_untracked_live_tmux_sessions() {
    let home = temp_home("jump-json-live-session");
    let fake_bin_dir = home.join("bin");
    fs::create_dir_all(&fake_bin_dir).unwrap();

    let create = run(&home, &["work", "create", "api", "--root", "/tmp"]);
    assert!(
        create.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&create),
        stderr(&create)
    );

    let tmux_script = fake_bin_dir.join("tmux");
    fs::write(
        &tmux_script,
        "#!/bin/sh\nif [ \"$1\" = \"list-sessions\" ]; then\n  printf 'api\\nadhoc\\n'\n  exit 0\nfi\nexit 1\n",
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
    let output = run_with_path(&home, &path, &["jump", "--json"]);
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let out = stdout(&output);
    assert!(out.contains("\"kind\": \"live_session\""));
    assert!(out.contains("\"tracked\": false"));
    assert!(out.contains("\"name\": \"adhoc\""));
    assert!(out.contains("\"session\": \"adhoc\""));

    cleanup_home(home);
}

#[test]
fn open_without_name_shows_ranked_prompt_and_accepts_selection() {
    let home = temp_home("open-no-name");
    let fake_bin_dir = home.join("bin");
    fs::create_dir_all(&fake_bin_dir).unwrap();

    let create = run(
        &home,
        &["work", "create", "api", "--root", "/tmp", "--favorite"],
    );
    assert!(
        create.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&create),
        stderr(&create)
    );

    let tmux_script = fake_bin_dir.join("tmux");
    fs::write(
        &tmux_script,
        "#!/bin/sh\nif [ \"$1\" = \"list-sessions\" ]; then\n  exit 0\nfi\nexit 1\n",
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&tmux_script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmux_script, perms).unwrap();
    }

    let mut child = Command::new(bin())
        .args(["open"])
        .env("HOME", &home)
        .env("PATH", fake_bin_dir.display().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    use std::io::Write;
    child.stdin.as_mut().unwrap().write_all(b"\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(out.contains("api"));

    cleanup_home(home);
}

#[test]
fn open_named_restores_snapshot_when_tmux_session_is_missing() {
    let home = temp_home("open-restores-missing-session");
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
    fs::write(
        snapshots_dir.join("api.json"),
        r#"{
  "version": 1,
  "work_name": "api",
  "session_name": "api-session",
  "active_window_index": 0,
  "windows": [
    {
      "index": 0,
      "name": "main",
      "active_pane_index": 0,
      "pane_count": 1,
      "panes": [
        {
          "index": 0,
          "cwd": "/tmp"
        }
      ]
    }
  ]
}
"#,
    )
    .unwrap();

    let tmux_log = home.join("tmux.log");
    let tmux_state = home.join("tmux-session-created");
    let tmux_script = fake_bin_dir.join("tmux");
    fs::write(
        &tmux_script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{log}\"\nif [ \"$1\" = \"has-session\" ]; then\n  if [ -f \"{state}\" ]; then\n    exit 0\n  fi\n  exit 1\nfi\nif [ \"$1\" = \"new-session\" ]; then\n  : > \"{state}\"\n  exit 0\nfi\nif [ \"$1\" = \"display-message\" ]; then\n  printf '0\\n'\n  exit 0\nfi\nif [ \"$1\" = \"rename-window\" ] || [ \"$1\" = \"send-keys\" ] || [ \"$1\" = \"select-window\" ] || [ \"$1\" = \"select-pane\" ] || [ \"$1\" = \"attach-session\" ] || [ \"$1\" = \"switch-client\" ]; then\n  exit 0\nfi\nexit 1\n",
            log = tmux_log.display(),
            state = tmux_state.display()
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
    let output = run_with_path(&home, &path, &["open", "api"]);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let tmux_calls = fs::read_to_string(&tmux_log).unwrap();
    assert!(tmux_calls.contains("has-session -t api-session"));
    assert!(tmux_calls.contains("new-session -d -s api-session -n main -c /tmp"));
    assert!(
        tmux_calls.contains("attach-session -t api-session")
            || tmux_calls.contains("switch-client -t api-session")
    );

    cleanup_home(home);
}

#[test]
fn open_named_untracked_live_session_attaches_directly() {
    let home = temp_home("open-live-session");
    let fake_bin_dir = home.join("bin");
    fs::create_dir_all(&fake_bin_dir).unwrap();

    let tmux_log = home.join("tmux.log");
    let tmux_script = fake_bin_dir.join("tmux");
    fs::write(
        &tmux_script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{log}\"\nif [ \"$1\" = \"has-session\" ] && [ \"$3\" = \"adhoc\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"attach-session\" ] && [ \"$3\" = \"adhoc\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"switch-client\" ] && [ \"$3\" = \"adhoc\" ]; then\n  exit 0\nfi\nexit 1\n",
            log = tmux_log.display()
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
    let output = run_with_path(&home, &path, &["open", "adhoc"]);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let tmux_calls = fs::read_to_string(&tmux_log).unwrap();
    assert!(tmux_calls.contains("has-session -t adhoc"));
    assert!(
        tmux_calls.contains("attach-session -t adhoc")
            || tmux_calls.contains("switch-client -t adhoc")
    );

    cleanup_home(home);
}
