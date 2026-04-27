use crate::paths::{AppPaths, find_binary, is_yaml_file};
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static PLUGIN_CACHE: OnceLock<std::result::Result<Vec<Plugin>, String>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub binary: String,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

impl Plugin {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("plugin has an empty name");
        }
        if self.binary.trim().is_empty() {
            bail!("plugin '{}' has an empty binary", self.name);
        }
        if self.aliases.is_empty() {
            bail!("plugin '{}' has no aliases", self.name);
        }
        Ok(())
    }
}

pub fn execute_external(paths: &AppPaths, argv: &[String]) -> Result<i32> {
    if argv.is_empty() {
        bail!("unknown command; run `muxwf --help`");
    }

    let plugin_name = &argv[0];
    let plugins = load_plugins(paths)?;

    if argv.len() < 2 {
        if let Some(plugin) = plugins.iter().find(|plugin| plugin.name == *plugin_name) {
            let aliases = plugin
                .aliases
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "plugin '{}' requires an alias; available aliases: {}",
                plugin_name,
                aliases
            );
        }
        bail!(
            "unknown command or plugin '{}'; run `muxwf --help` or add ~/.muxwf/plugins/{}.yaml to use it",
            plugin_name,
            plugin_name
        );
    }

    let alias = &argv[1];
    let args = &argv[2..];
    let plugin = plugins
        .iter()
        .find(|plugin| plugin.name == *plugin_name)
        .with_context(|| {
            format!(
                "unknown command or plugin '{}'; add ~/.muxwf/plugins/{}.yaml to use it",
                plugin_name, plugin_name
            )
        })?;

    run_alias(plugin, alias, args)
}

pub fn run_alias(plugin: &Plugin, alias: &str, args: &[String]) -> Result<i32> {
    plugin.validate()?;
    if find_binary(&plugin.binary).is_none() {
        bail!(
            "plugin '{}' requires binary '{}' but it was not found in PATH",
            plugin.name,
            plugin.binary
        );
    }

    let template = plugin.aliases.get(alias).with_context(|| {
        let aliases = plugin
            .aliases
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "plugin '{}' has no alias '{}'; available aliases: {}",
            plugin.name, alias, aliases
        )
    })?;

    let status = if is_shell_binary(&plugin.binary) {
        let command = render_shell_template(template, args)?;
        Command::new(&plugin.binary)
            .arg("-lc")
            .arg(command)
            .status()
            .with_context(|| format!("failed to run plugin '{} {}'", plugin.name, alias))?
    } else {
        let rendered_args = render_direct_args(template, args)?;
        Command::new(&plugin.binary)
            .args(rendered_args)
            .status()
            .with_context(|| format!("failed to run plugin '{} {}'", plugin.name, alias))?
    };

    Ok(status.code().unwrap_or(1))
}

pub fn load_plugins(paths: &AppPaths) -> Result<Vec<Plugin>> {
    match PLUGIN_CACHE
        .get_or_init(|| load_plugins_uncached(paths).map_err(|error| format!("{error:#}")))
    {
        Ok(plugins) => Ok(plugins.clone()),
        Err(error) => bail!("{error}"),
    }
}

fn load_plugins_uncached(paths: &AppPaths) -> Result<Vec<Plugin>> {
    let mut plugins = Vec::new();
    for file in plugin_files(paths)? {
        plugins.push(load_plugin_file(&file)?);
    }
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(plugins)
}

pub fn load_plugin_file(path: &Path) -> Result<Plugin> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read plugin file {}", path.display()))?;
    let plugin: Plugin = serde_yaml::from_str(&raw)
        .with_context(|| format!("invalid YAML in plugin file {}", path.display()))?;
    plugin
        .validate()
        .with_context(|| format!("invalid plugin file {}", path.display()))?;
    Ok(plugin)
}

pub fn plugin_files(paths: &AppPaths) -> Result<Vec<PathBuf>> {
    let dir = paths.plugins_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let path = entry
            .with_context(|| format!("failed to read entry in {}", dir.display()))?
            .path();
        if is_yaml_file(&path) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn is_shell_binary(binary: &str) -> bool {
    let name = Path::new(binary)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(binary);
    matches!(name, "bash" | "sh" | "zsh" | "fish")
}

fn render_shell_template(template: &str, args: &[String]) -> Result<String> {
    let mut used = BTreeSet::new();
    let mut rendered = replace_arg_placeholders(template, args, &mut used, true)?;
    if rendered.contains("{{args}}") {
        rendered = rendered.replace("{{args}}", &join_shell_escaped(args));
        used.extend(0..args.len());
    }
    append_unused_args(&mut rendered, args, &used, true);
    Ok(rendered)
}

fn render_direct_args(template: &str, args: &[String]) -> Result<Vec<String>> {
    let tokens = split_command(template)?;
    let mut used = BTreeSet::new();
    let mut out = Vec::new();

    for token in tokens {
        if token == "{{args}}" {
            out.extend(args.iter().cloned());
            used.extend(0..args.len());
            continue;
        }

        let mut rendered = replace_arg_placeholders(&token, args, &mut used, false)?;
        if rendered.contains("{{args}}") {
            rendered = rendered.replace("{{args}}", &args.join(" "));
            used.extend(0..args.len());
        }
        if !rendered.is_empty() {
            out.push(rendered);
        }
    }

    for (idx, arg) in args.iter().enumerate() {
        if !used.contains(&idx) {
            out.push(arg.clone());
        }
    }

    Ok(out)
}

fn replace_arg_placeholders(
    template: &str,
    args: &[String],
    used: &mut BTreeSet<usize>,
    shell_escape_values: bool,
) -> Result<String> {
    let re = Regex::new(r"\{\{arg([0-9]+)\}\}").expect("valid placeholder regex");
    let mut rendered = template.to_string();
    for captures in re.captures_iter(template) {
        let placeholder = captures.get(0).expect("placeholder capture").as_str();
        let number = captures
            .get(1)
            .expect("number capture")
            .as_str()
            .parse::<usize>()
            .context("invalid arg placeholder")?;
        if number == 0 {
            bail!("{} is invalid; placeholders start at {{arg1}}", placeholder);
        }
        let idx = number - 1;
        let value = args.get(idx).with_context(|| {
            format!(
                "{} was used but only {} args were provided",
                placeholder,
                args.len()
            )
        })?;
        used.insert(idx);
        let replacement = if shell_escape_values {
            shell_quote(value)
        } else {
            value.clone()
        };
        rendered = rendered.replace(placeholder, &replacement);
    }
    Ok(rendered)
}

fn append_unused_args(
    command: &mut String,
    args: &[String],
    used: &BTreeSet<usize>,
    shell_escape_values: bool,
) {
    let remaining = args
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used.contains(idx))
        .map(|(_, arg)| {
            if shell_escape_values {
                shell_quote(arg)
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>();

    if remaining.is_empty() {
        return;
    }
    if !command.trim().is_empty() {
        command.push(' ');
    }
    command.push_str(&remaining.join(" "));
}

fn join_shell_escaped(args: &[String]) -> String {
    args.iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn split_command(input: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some('\''), '\'') => quote = None,
            (Some('"'), '"') => quote = None,
            (Some(_), '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    current.push('\\');
                }
            }
            (Some(_), _) => current.push(ch),
            (None, '\'') | (None, '"') => quote = Some(ch),
            (None, '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    current.push('\\');
                }
            }
            (None, ch) if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            (None, _) => current.push(ch),
        }
    }

    if let Some(ch) = quote {
        bail!("unterminated quote {}", ch);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_template_expands_numbered_args_and_appends_rest() {
        let args = vec!["mypod".to_string(), "--tail".to_string(), "100".to_string()];

        let rendered = render_direct_args("logs -f {{arg1}}", &args).unwrap();

        assert_eq!(rendered, vec!["logs", "-f", "mypod", "--tail", "100"]);
    }

    #[test]
    fn direct_template_expands_all_args_as_separate_tokens() {
        let args = vec!["-q".to_string(), "tests/unit test.py".to_string()];

        let rendered = render_direct_args("pytest {{args}}", &args).unwrap();

        assert_eq!(rendered, vec!["pytest", "-q", "tests/unit test.py"]);
    }

    #[test]
    fn split_command_handles_quotes() {
        let tokens = split_command("get pods -l app='my api'").unwrap();

        assert_eq!(tokens, vec!["get", "pods", "-l", "app=my api"]);
    }
}
