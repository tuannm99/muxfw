use crate::paths::AppPaths;
use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub rules: Vec<RestoreRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreRule {
    pub cwd_regex: String,
    pub on_restore: String,
}

#[derive(Debug, Clone)]
pub struct Ruleset {
    rules: Vec<CompiledRule>,
}

#[derive(Debug, Clone)]
struct CompiledRule {
    regex: Regex,
    on_restore: String,
}

impl Ruleset {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        let config = load_config(paths)?;
        let mut rules = Vec::new();
        for rule in config.rules {
            let regex = Regex::new(&rule.cwd_regex)
                .with_context(|| format!("invalid cwd_regex '{}'", rule.cwd_regex))?;
            rules.push(CompiledRule {
                regex,
                on_restore: rule.on_restore,
            });
        }
        Ok(Self { rules })
    }

    pub fn hook_for(&self, cwd: &str) -> Option<&str> {
        self.rules
            .iter()
            .find(|rule| rule.regex.is_match(cwd))
            .map(|rule| rule.on_restore.as_str())
    }
}

pub fn load_config(paths: &AppPaths) -> Result<AppConfig> {
    let path = paths.config_file();
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("invalid YAML in {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_matching_rule_wins() {
        let rules = Ruleset {
            rules: vec![
                CompiledRule {
                    regex: Regex::new(".*/frontend$").unwrap(),
                    on_restore: "pnpm install".to_string(),
                },
                CompiledRule {
                    regex: Regex::new(".*").unwrap(),
                    on_restore: "echo fallback".to_string(),
                },
            ],
        };

        assert_eq!(rules.hook_for("/tmp/frontend"), Some("pnpm install"));
    }
}
