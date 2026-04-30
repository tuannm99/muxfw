mod common;

use common::{cleanup_home, run, stderr, stdout, temp_home};

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
fn top_level_help_surfaces_core_commands() {
    let home = temp_home("top-level-help");
    let output = run(&home, &["--help"]);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let out = stdout(&output);
    assert!(out.contains("open"));
    assert!(out.contains("list"));
    assert!(out.contains("doctor"));
    assert!(out.contains("add"));

    cleanup_home(home);
}

#[test]
fn unknown_single_token_command_reports_unknown_command_instead_of_plugin_usage() {
    let home = temp_home("unknown-command");
    let output = run(&home, &["oepn"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("unknown command or plugin 'oepn'"));
    assert!(!stderr(&output).contains("plugin invocation requires"));

    cleanup_home(home);
}
