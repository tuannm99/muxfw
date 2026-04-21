use crate::cli::Cli;
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::io;

pub fn print_completion(shell: Shell, command_name: Option<String>) {
    let mut command = Cli::command();
    if let Some(name) = command_name {
        let name: &'static str = Box::leak(name.into_boxed_str());
        command = command.name(name);
    }
    let name = command.get_name().to_string();
    generate(shell, &mut command, name, &mut io::stdout());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_completion_mentions_muxwf_command() {
        let mut command = Cli::command();
        let mut output = Vec::new();
        command = command.name("mw");
        let name = command.get_name().to_string();

        generate(Shell::Zsh, &mut command, name, &mut output);
        let rendered = String::from_utf8(output).unwrap();

        assert!(rendered.contains("#compdef mw"));
        assert!(rendered.contains("completion"));
    }
}
