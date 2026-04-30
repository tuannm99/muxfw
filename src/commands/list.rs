use crate::cli::{ListArgs, StaleArgs};
use crate::context;
use crate::output;
use crate::paths::AppPaths;
use crate::work::{self, Work, WorkStatus};
use anyhow::Result;

pub fn list(paths: &AppPaths, args: ListArgs) -> Result<i32> {
    let works = filtered_works(paths, &args)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&works)?);
        return Ok(0);
    }

    for work in &works {
        output::print_work_row(work, args.names_only);
    }
    Ok(0)
}

pub fn recent(paths: &AppPaths) -> Result<i32> {
    list(
        paths,
        ListArgs {
            names_only: false,
            json: false,
            tags: Vec::new(),
            group: None,
            favorite: false,
            status: None,
            recent: true,
            live: false,
            stale_days: None,
        },
    )
}

pub fn stale(paths: &AppPaths, args: StaleArgs) -> Result<i32> {
    list(
        paths,
        ListArgs {
            names_only: args.names_only,
            json: args.json,
            tags: Vec::new(),
            group: None,
            favorite: false,
            status: None,
            recent: false,
            live: false,
            stale_days: Some(args.days),
        },
    )
}

pub fn current(paths: &AppPaths) -> Result<i32> {
    let work = context::current_work(paths)?;
    output::print_work_row(&work, false);
    Ok(0)
}

pub fn set_favorite(paths: &AppPaths, name: &str, favorite: bool) -> Result<i32> {
    let mut work = work::load_work(paths, name)?;
    if work.favorite == favorite {
        println!(
            "'{}' is already {}",
            work.name,
            if favorite { "pinned" } else { "unpinned" }
        );
        return Ok(0);
    }

    work.favorite = favorite;
    work::write_work(paths, &work)?;
    println!(
        "{} '{}'",
        if favorite { "pinned" } else { "unpinned" },
        work.name
    );
    Ok(0)
}

pub fn set_work_status(paths: &AppPaths, name: &str, status: WorkStatus) -> Result<i32> {
    let mut work = work::load_work(paths, name)?;
    if work.status == status {
        println!(
            "'{}' is already {}",
            work.name,
            format!("{:?}", status).to_lowercase()
        );
        return Ok(0);
    }

    work.status = status;
    work::write_work(paths, &work)?;
    println!(
        "updated '{}' status to {}",
        work.name,
        format!("{:?}", status).to_lowercase()
    );
    Ok(0)
}

fn filtered_works(paths: &AppPaths, args: &ListArgs) -> Result<Vec<Work>> {
    let live_sessions = if args.live {
        Some(crate::tmux::list_sessions()?)
    } else {
        None
    };
    let mut works = work::list_works(paths)?;

    works.retain(|work| {
        args.tags
            .iter()
            .all(|tag| work.tags.iter().any(|work_tag| work_tag == tag))
    });
    if let Some(group) = &args.group {
        works.retain(|work| work.group.as_deref() == Some(group.as_str()));
    }
    if args.favorite {
        works.retain(|work| work.favorite);
    }
    if let Some(status) = args.status {
        works.retain(|work| work.status == status);
    }
    if args.recent {
        works.retain(|work| work.last_opened_at.is_some());
    }
    if let Some(stale_days) = args.stale_days {
        works.retain(|work| work.is_stale(stale_days));
    }
    if let Some(live_sessions) = &live_sessions {
        works.retain(|work| live_sessions.contains(&work.session));
    }

    if args.recent {
        works.sort_by(|a, b| {
            b.last_opened_at
                .cmp(&a.last_opened_at)
                .then_with(|| a.name.cmp(&b.name))
        });
    }

    Ok(works)
}
