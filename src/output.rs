use crate::work::Work;
use chrono::{DateTime, Utc};

pub fn print_work_row(work: &Work, names_only: bool) {
    if names_only {
        println!("{}", work.name);
        return;
    }

    let tags = if work.tags.is_empty() {
        "-".to_string()
    } else {
        work.tags.join(",")
    };
    let group = work.group.as_deref().unwrap_or("-");
    let favorite = if work.favorite { "yes" } else { "-" };
    let description = work.description.as_deref().unwrap_or("-");
    let last_opened_at = format_timestamp(work.last_opened_at.as_ref());
    let status = format!("{:?}", work.status).to_lowercase();
    println!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        work.name,
        work.session,
        work.root,
        status,
        tags,
        description,
        group,
        favorite,
        last_opened_at
    );
}

pub fn print_workspace_row(workspace: &crate::workspace::Workspace, names_only: bool) {
    if names_only {
        println!("{}", workspace.name);
        return;
    }
    let profile = workspace.profile.as_deref().unwrap_or("-");
    println!(
        "{}\t{}\t{}\t{}",
        workspace.name,
        profile,
        format!("{:?}", workspace.policy)
            .to_lowercase()
            .replace('_', "-"),
        workspace.works.join(",")
    );
}

pub fn format_jump_row(work: &Work, live: bool) -> String {
    let favorite = if work.favorite { "favorite" } else { "-" };
    let live = if live { "live" } else { "-" };
    let group = work.group.as_deref().unwrap_or("-");
    let last_opened_at = format_timestamp(work.last_opened_at.as_ref());
    let description = work.description.as_deref().unwrap_or("-");
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        work.name, favorite, live, group, last_opened_at, description
    )
}

pub fn format_timestamp(value: Option<&DateTime<Utc>>) -> String {
    value
        .map(DateTime::to_rfc3339)
        .unwrap_or_else(|| "-".to_string())
}
