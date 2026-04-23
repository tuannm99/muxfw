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
    let mut output = Vec::new();
    generate(shell, &mut command, name, &mut output);
    if matches!(shell, Shell::Zsh) {
        output = patch_zsh_completion(output, command.get_name());
    }
    io::Write::write_all(&mut io::stdout(), &output).expect("failed to write completion");
}

fn patch_zsh_completion(output: Vec<u8>, command_name: &str) -> Vec<u8> {
    let rendered = String::from_utf8(output).expect("completion should be UTF-8");
    let rendered = rendered
        .replace(
            "::name -- Work name. Defaults to the work mapped to the current tmux session:_default",
            "::name -- Work name. Defaults to the work mapped to the current tmux session:_muxwf_work_names",
        )
        .replace(":name:_default", ":name:_muxwf_name_values")
        .replace(
            ":target -- Work name, or `current` to add the current tmux session:_default",
            ":target -- Work name, or `current` to add the current tmux session:_muxwf_add_targets",
        );
    format!("{rendered}\n{}", zsh_dynamic_helpers(command_name)).into_bytes()
}

fn zsh_dynamic_helpers(command_name: &str) -> String {
    let command = shell_single_quote(command_name);
    r#"
_muxwf_name_values() {
    if (( ${words[(Ie)workspace]} || ${words[(Ie)ws]} )); then
        _muxwf_workspace_names
    else
        _muxwf_work_names
    fi
}

_muxwf_work_names() {
    local cmd=__MUXWF_COMMAND__
    local -a works
    works=("${(@f)$($cmd list --names-only 2>/dev/null)}")
    _describe -t works 'muxwf works' works
}

_muxwf_workspace_names() {
    local cmd=__MUXWF_COMMAND__
    local -a rows workspaces
    rows=("${(@f)$($cmd ws list 2>/dev/null)}")
    workspaces=("${(@)rows%%	*}")
    _describe -t workspaces 'muxwf workspaces' workspaces
}

_muxwf_add_targets() {
    local cmd=__MUXWF_COMMAND__
    local -a targets works
    works=("${(@f)$($cmd list --names-only 2>/dev/null)}")
    targets=(current "${works[@]}")
    _describe -t targets 'muxwf add targets' targets
}
"#
    .replace("__MUXWF_COMMAND__", &command)
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
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

    #[test]
    fn zsh_completion_uses_dynamic_work_names() {
        let mut command = Cli::command();
        let mut output = Vec::new();
        command = command.name("mw");
        let name = command.get_name().to_string();

        generate(Shell::Zsh, &mut command, name, &mut output);
        let rendered = String::from_utf8(patch_zsh_completion(output, "mw")).unwrap();

        assert!(rendered.contains("_mw_commands"));
        assert!(rendered.contains(
            "'open:Switch or attach to the session, restoring or creating it if needed'"
        ));
        assert!(rendered.contains("'init:Generate work configs"));
        assert!(rendered.contains("local cmd='mw'"));
        assert!(rendered.contains("_muxwf_work_names"));
        assert!(rendered.contains("_muxwf_workspace_names"));
        assert!(rendered.contains("list --names-only"));
        assert!(!rendered.contains("words[1]"));
        assert!(rendered.contains("_muxwf_add_targets"));
    }
}
