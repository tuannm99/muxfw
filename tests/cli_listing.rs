mod common;

use common::{cleanup_home, run, stderr, stdout, temp_home};
use std::fs;

#[test]
fn list_orders_by_recency_favorite_usage_then_name() {
    let home = temp_home("list-priority");
    let works_dir = home.join(".muxwf/works");
    fs::create_dir_all(&works_dir).unwrap();

    fs::write(
        works_dir.join("cold.yaml"),
        "\
name: cold
session: cold
root: /tmp
created_at: 2026-01-01T00:00:00Z
updated_at: 2026-01-01T00:00:00Z
",
    )
    .unwrap();
    fs::write(
        works_dir.join("recent.yaml"),
        "\
name: recent
session: recent
root: /tmp
open_count: 2
last_opened_at: 2026-03-01T00:00:00Z
created_at: 2026-01-01T00:00:00Z
updated_at: 2026-01-01T00:00:00Z
",
    )
    .unwrap();
    fs::write(
        works_dir.join("frequent.yaml"),
        "\
name: frequent
session: frequent
root: /tmp
open_count: 9
last_opened_at: 2026-02-01T00:00:00Z
created_at: 2026-01-01T00:00:00Z
updated_at: 2026-01-01T00:00:00Z
",
    )
    .unwrap();
    fs::write(
        works_dir.join("favorite.yaml"),
        "\
name: favorite
session: favorite
root: /tmp
favorite: true
open_count: 1
last_opened_at: 2026-01-01T00:00:00Z
created_at: 2026-01-01T00:00:00Z
updated_at: 2026-01-01T00:00:00Z
",
    )
    .unwrap();

    let output = run(&home, &["list", "--names-only"]);
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert_eq!(
        stdout(&output).lines().collect::<Vec<_>>(),
        vec!["recent", "frequent", "favorite", "cold"]
    );

    cleanup_home(home);
}

#[test]
fn work_status_filters_archive_and_stale_listing_work() {
    let home = temp_home("work-status-stale");

    let create = run(
        &home,
        &[
            "work", "create", "api", "--root", "/tmp", "--status", "paused",
        ],
    );
    assert!(
        create.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&create),
        stderr(&create)
    );

    let paused = run(&home, &["list", "--status", "paused", "--names-only"]);
    assert!(paused.status.success(), "stderr:\n{}", stderr(&paused));
    assert_eq!(stdout(&paused).trim(), "api");

    let archive = run(&home, &["archive", "api"]);
    assert!(archive.status.success(), "stderr:\n{}", stderr(&archive));

    let archived = run(&home, &["list", "--status", "archived", "--json"]);
    assert!(archived.status.success(), "stderr:\n{}", stderr(&archived));
    let archived_out = stdout(&archived);
    assert!(archived_out.contains("\"status\": \"archived\""));

    let work_file = home.join(".muxwf/works/api.yaml");
    let stale_yaml = "\
name: stale
session: stale
root: /tmp
status: archived
updated_at: 2026-01-01T00:00:00Z
last_opened_at: 2026-01-01T00:00:00Z
";
    fs::write(home.join(".muxwf/works/stale.yaml"), stale_yaml).unwrap();

    let stale = run(&home, &["stale", "--days", "30", "--names-only"]);
    assert!(stale.status.success(), "stderr:\n{}", stderr(&stale));
    let stale_out = stdout(&stale);
    assert!(stale_out.lines().any(|line| line == "stale"));
    assert!(!stale_out.lines().any(|line| line == "api"));

    assert!(work_file.exists());
    cleanup_home(home);
}
