//! User supplied CLI command templates. The guest parses argv before substitution.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct CustomCli {
    pub name: String,
    pub install: Option<String>,
    pub interactive: String,
    pub headless: Option<String>,
    pub resume: Option<String>,
}

impl CustomCli {
    pub fn validate(&self, interactive: bool) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.len() > 80 || self.name.chars().any(char::is_control) {
            return Err("custom CLI needs a name of 1–80 characters".into());
        }
        if !interactive && self.headless.is_none() {
            return Err("custom CLI needs a headless command for automatic tasks".into());
        }
        for command in std::iter::once(&self.interactive).chain(self.headless.iter()).chain(self.resume.iter()) {
            if command.trim().is_empty() || command.len() > 8192 || command.chars().any(char::is_control) {
                return Err("custom CLI commands must be nonempty single lines".into());
            }
            let executable =
                command.trim_start().trim_start_matches(['\'', '"']).split_whitespace().next().unwrap_or_default();
            if executable.starts_with('-') || executable.contains('{') {
                return Err("custom CLI needs a fixed executable name".into());
            }
            let mut quote = None;
            let mut escaped = false;
            for c in command.chars() {
                if escaped {
                    escaped = false;
                    continue;
                }
                if c == '\\' && quote != Some('\'') {
                    escaped = true;
                    continue;
                }
                if let Some(q) = quote {
                    if c == q {
                        quote = None;
                    }
                } else if c == '\'' || c == '"' {
                    quote = Some(c);
                } else if ";|&<>`".contains(c) || c == '$' {
                    return Err("custom launch commands accept arguments, not shell expressions".into());
                }
            }
            if quote.is_some() || escaped {
                return Err("custom CLI command has an unfinished quote or escape".into());
            }
        }
        Ok(())
    }
}
