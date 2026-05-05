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

#[test]
fn workspace_create_from_dir_creates_direct_child_works() {
    let home = temp_home("workspace-from-dir-create");
    let scan_root = home.join("projects");
    fs::create_dir_all(scan_root.join("api")).unwrap();
    fs::create_dir_all(scan_root.join("web app")).unwrap();
    fs::create_dir_all(scan_root.join("api/nested")).unwrap();

    let create = run(
        &home,
        &[
            "workspace",
            "create",
            "suite",
            "--from-dir",
            scan_root.to_str().unwrap(),
        ],
    );
    assert!(
        create.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&create),
        stderr(&create)
    );

    let workspace_yaml = fs::read_to_string(home.join(".muxwf/workspaces/suite.yaml")).unwrap();
    assert!(workspace_yaml.contains("- api\n"));
    assert!(workspace_yaml.contains("- web-app\n"));

    let api_work = fs::read_to_string(home.join(".muxwf/works/api.yaml")).unwrap();
    assert!(api_work.contains(&format!("root: {}", scan_root.join("api").display())));

    let web_work = fs::read_to_string(home.join(".muxwf/works/web-app.yaml")).unwrap();
    assert!(web_work.contains(&format!("root: {}", scan_root.join("web app").display())));

    assert!(!home.join(".muxwf/works/nested.yaml").exists());

    cleanup_home(home);
}

#[test]
fn workspace_add_from_dir_merges_missing_direct_child_works() {
    let home = temp_home("workspace-from-dir-add");
    let scan_root = home.join("services");
    let works_dir = home.join(".muxwf/works");
    fs::create_dir_all(scan_root.join("api")).unwrap();
    fs::create_dir_all(scan_root.join("jobs")).unwrap();
    fs::create_dir_all(&works_dir).unwrap();
    fs::write(
        works_dir.join("api.yaml"),
        format!(
            "name: api\nsession: api\nroot: {}\non_restore: \"\"\n",
            scan_root.join("api").display()
        ),
    )
    .unwrap();

    let create = run(&home, &["workspace", "create", "suite", "--work", "api"]);
    assert!(create.status.success(), "stderr:\n{}", stderr(&create));

    let add = run(
        &home,
        &[
            "workspace",
            "add",
            "suite",
            "--from-dir",
            scan_root.to_str().unwrap(),
        ],
    );
    assert!(add.status.success(), "stderr:\n{}", stderr(&add));

    let listed = run(&home, &["workspace", "list"]);
    assert!(listed.status.success(), "stderr:\n{}", stderr(&listed));
    assert_eq!(stdout(&listed).trim(), "suite\t-\tsmart\tapi,jobs");
    assert!(home.join(".muxwf/works/jobs.yaml").exists());

    cleanup_home(home);
}

#[test]
fn workspace_from_dir_reuses_existing_work_shared_with_another_workspace() {
    let home = temp_home("workspace-from-dir-shared-work");
    let scan_root = home.join("projects");
    let works_dir = home.join(".muxwf/works");
    let workspaces_dir = home.join(".muxwf/workspaces");
    fs::create_dir_all(scan_root.join("api")).unwrap();
    fs::create_dir_all(scan_root.join("web")).unwrap();
    fs::create_dir_all(&works_dir).unwrap();
    fs::create_dir_all(&workspaces_dir).unwrap();

    let existing_api = format!(
        "name: api\nsession: custom-api\nroot: {}\ndescription: Shared API\non_restore: \"\"\n",
        scan_root.join("api").display()
    );
    fs::write(works_dir.join("api.yaml"), &existing_api).unwrap();
    fs::write(
        workspaces_dir.join("daily.yaml"),
        "name: daily\nworks:\n  - api\n",
    )
    .unwrap();

    let create = run(
        &home,
        &[
            "workspace",
            "create",
            "release",
            "--from-dir",
            scan_root.to_str().unwrap(),
        ],
    );
    assert!(
        create.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&create),
        stderr(&create)
    );

    let api_work = fs::read_to_string(works_dir.join("api.yaml")).unwrap();
    assert_eq!(api_work, existing_api);
    assert!(stdout(&create).contains("created work 'web'"));
    assert!(!stdout(&create).contains("created work 'api'"));

    let listed = run(&home, &["workspace", "list"]);
    assert!(
        listed.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&listed),
        stderr(&listed)
    );
    let rows = stdout(&listed);
    assert!(rows.contains("daily\t-\tsmart\tapi"));
    assert!(rows.contains("release\t-\tsmart\tapi,web"));

    cleanup_home(home);
}
