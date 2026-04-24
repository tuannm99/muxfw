use crate::snapshot::{Snapshot, WindowSnapshot};
use crate::work::{self, Work, WorkStatus, WorkWindow};
use anyhow::Result;

#[derive(Debug, Clone, Default)]
pub struct WorkMetadata {
    pub on_restore: Option<String>,
    pub description: Option<String>,
    pub status: WorkStatus,
    pub group: Option<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
}

pub fn work_from_snapshot(
    snapshot: &Snapshot,
    name_override: Option<String>,
    root_override: Option<String>,
    metadata: WorkMetadata,
) -> Result<Work> {
    snapshot.validate()?;
    let name = match name_override {
        Some(name) => name,
        None => inferred_work_name(&snapshot.session_name)?,
    };
    let root = root_override.unwrap_or_else(|| snapshot_root(snapshot));
    let mut work = Work::new(name, snapshot.session_name.clone(), root);
    work.windows = snapshot
        .windows
        .iter()
        .map(work_window_from_snapshot)
        .collect();
    work.on_restore = metadata.on_restore;
    work.description = metadata.description;
    work.status = metadata.status;
    work.group = metadata.group;
    work.tags = metadata.tags;
    work.favorite = metadata.favorite;
    work.validate()?;
    Ok(work)
}

pub fn inferred_work_name(session: &str) -> Result<String> {
    let name = work::sanitize_name(session);
    if name.is_empty() {
        anyhow::bail!(
            "could not infer a work name from tmux session '{}'",
            session
        );
    }
    Ok(name)
}

fn snapshot_root(snapshot: &Snapshot) -> String {
    snapshot
        .windows
        .iter()
        .find(|window| window.index == snapshot.active_window_index)
        .and_then(|window| {
            window
                .panes
                .iter()
                .find(|pane| pane.index == window.active_pane_index)
                .or_else(|| window.panes.first())
        })
        .or_else(|| {
            snapshot
                .windows
                .first()
                .and_then(|window| window.panes.first())
        })
        .map(|pane| pane.cwd.clone())
        .unwrap_or_else(|| ".".to_string())
}

fn work_window_from_snapshot(window: &WindowSnapshot) -> WorkWindow {
    WorkWindow {
        name: window.name.clone(),
        cwd: window.panes.first().map(|pane| pane.cwd.clone()),
        panes: window.pane_count,
    }
}

pub fn apply_add_args_metadata(args: &crate::cli::AddArgs) -> WorkMetadata {
    WorkMetadata {
        on_restore: args.on_restore.clone(),
        description: args.description.clone(),
        status: args.status,
        group: args.group.clone(),
        tags: args.tags.clone(),
        favorite: args.favorite,
    }
}

pub fn ensure_session_option_absent(session: &Option<String>) -> Result<()> {
    if session.is_some() {
        anyhow::bail!("--session is not valid with `muxwf add current`");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{PaneSnapshot, WindowSnapshot};

    fn snapshot() -> Snapshot {
        Snapshot {
            version: 1,
            work_name: None,
            session_name: "api_dev".to_string(),
            active_window_index: 1,
            windows: vec![
                WindowSnapshot {
                    index: 0,
                    name: "main".to_string(),
                    layout: None,
                    active_pane_index: 0,
                    pane_count: 1,
                    panes: vec![PaneSnapshot {
                        index: 0,
                        cwd: "/tmp/api".to_string(),
                    }],
                },
                WindowSnapshot {
                    index: 1,
                    name: "logs".to_string(),
                    layout: None,
                    active_pane_index: 2,
                    pane_count: 2,
                    panes: vec![
                        PaneSnapshot {
                            index: 1,
                            cwd: "/tmp/api/logs".to_string(),
                        },
                        PaneSnapshot {
                            index: 2,
                            cwd: "/tmp/api/current".to_string(),
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn work_from_snapshot_uses_active_pane_as_root() {
        let work = work_from_snapshot(&snapshot(), None, None, WorkMetadata::default()).unwrap();

        assert_eq!(work.name, "api_dev");
        assert_eq!(work.session, "api_dev");
        assert_eq!(work.root, "/tmp/api/current");
        assert_eq!(work.windows.len(), 2);
        assert_eq!(work.windows[1].panes, 2);
        assert_eq!(work.windows[1].cwd.as_deref(), Some("/tmp/api/logs"));
    }

    #[test]
    fn work_from_snapshot_applies_overrides_and_metadata() {
        let metadata = WorkMetadata {
            description: Some("API".to_string()),
            status: WorkStatus::Paused,
            group: Some("backend".to_string()),
            tags: vec!["rust".to_string()],
            favorite: true,
            on_restore: Some("cargo check".to_string()),
        };

        let work = work_from_snapshot(
            &snapshot(),
            Some("api".to_string()),
            Some("~/dev/api".to_string()),
            metadata,
        )
        .unwrap();

        assert_eq!(work.name, "api");
        assert_eq!(work.root, "~/dev/api");
        assert_eq!(work.description.as_deref(), Some("API"));
        assert_eq!(work.status, WorkStatus::Paused);
        assert_eq!(work.group.as_deref(), Some("backend"));
        assert_eq!(work.tags, vec!["rust"]);
        assert!(work.favorite);
        assert_eq!(work.on_restore.as_deref(), Some("cargo check"));
    }
}
