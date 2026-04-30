mod common;

use common::{cleanup_home, run, stderr, stdout, temp_home};
use std::fs;

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
    assert_eq!(stdout(&output).trim(), "suite\t-\tsmart\tapi");

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
    assert_eq!(stdout(&listed).trim(), "suite\t-\tsmart\tapi,worker");

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
    assert_eq!(stdout(&listed).trim(), "suite\t-\tsmart\tapi,jobs");

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
fn workspace_profile_and_policy_round_trip() {
    let home = temp_home("workspace-profile-policy");

    let create = run(
        &home,
        &[
            "workspace",
            "create",
            "daily",
            "--work",
            "api",
            "--profile",
            "incident",
            "--policy",
            "fresh",
        ],
    );
    assert!(create.status.success(), "stderr:\n{}", stderr(&create));

    let listed = run(&home, &["workspace", "list"]);
    assert!(listed.status.success(), "stderr:\n{}", stderr(&listed));
    assert_eq!(stdout(&listed).trim(), "daily\tincident\tfresh\tapi");

    let update = run(
        &home,
        &[
            "workspace",
            "update",
            "daily",
            "--work",
            "api",
            "--policy",
            "reuse-only",
            "--clear-profile",
        ],
    );
    assert!(update.status.success(), "stderr:\n{}", stderr(&update));

    let json = run(&home, &["workspace", "list", "--json"]);
    assert!(json.status.success(), "stderr:\n{}", stderr(&json));
    let out = stdout(&json);
    assert!(out.contains("\"policy\": \"reuse-only\""));
    assert!(!out.contains("\"profile\": \"incident\""));

    cleanup_home(home);
}
