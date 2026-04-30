mod app;
mod autocomplete;
mod cli;
mod commands;
mod context;
mod discover;
mod editor;
mod output;
mod paths;
mod plugin;
mod restore;
mod rules;
mod snapshot;
mod tmux;
mod work;
mod workspace;

fn main() {
    match app::run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("error: {error:#}");
            std::process::exit(1);
        }
    }
}
