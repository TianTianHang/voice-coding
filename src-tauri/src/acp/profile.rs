use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_ENV: &str = "ACP_AGENT_CONFIG";
const DEFAULT_CONFIG_FILE: &str = "acp-agents.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfigFile {
    pub default_profile: Option<String>,
    pub profiles: Vec<JsonAgentProfile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonAgentProfile {
    pub id: Option<String>,
    pub name: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl AgentProfile {
    pub fn from_config_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read ACP agent config {}: {e}", path.display()))?;
        let config: AgentConfigFile = serde_json::from_str(&raw)
            .map_err(|e| format!("Failed to parse ACP agent config {}: {e}", path.display()))?;
        config.resolve_default()
    }

    pub fn from_environment() -> Result<Self, String> {
        let path = config_path_from_environment()?;
        Self::from_config_path(path)
    }
}

impl AgentConfigFile {
    pub fn resolve_default(self) -> Result<AgentProfile, String> {
        if self.profiles.is_empty() {
            return Err("ACP agent config must contain at least one profile".into());
        }

        let selected_id = match self.default_profile.as_deref() {
            Some(id) if !id.trim().is_empty() => Some(id),
            Some(_) => return Err("ACP agent defaultProfile must not be empty".into()),
            None if self.profiles.len() == 1 => None,
            None => {
                return Err("ACP agent config has multiple profiles but no defaultProfile".into())
            }
        };

        let selected = if let Some(id) = selected_id {
            self.profiles
                .into_iter()
                .find(|profile| profile.id.as_deref() == Some(id))
                .ok_or_else(|| format!("ACP agent defaultProfile '{id}' does not exist"))?
        } else {
            self.profiles
                .into_iter()
                .next()
                .expect("profiles is known to contain one item")
        };

        selected.validate()
    }
}

impl JsonAgentProfile {
    fn validate(self) -> Result<AgentProfile, String> {
        let id = required_field(self.id, "id")?;
        let name = required_field(self.name, "name")?;
        let command = required_field(self.command, "command")?;
        let env = self
            .env
            .into_iter()
            .map(|(key, value)| expand_env_value(&key, &value).map(|expanded| (key, expanded)))
            .collect::<Result<HashMap<_, _>, _>>()?;

        Ok(AgentProfile {
            id,
            name,
            command,
            args: self.args,
            cwd: self.cwd,
            env,
        })
    }
}

fn config_path_from_environment() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var(CONFIG_ENV) {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!(
            "{CONFIG_ENV} points to missing ACP agent config {}",
            path.display()
        ));
    }

    let current_dir = std::env::current_dir()
        .map_err(|e| format!("Failed to determine current directory: {e}"))?;
    discover_config_path(&current_dir).ok_or_else(|| {
        format!(
            "No ACP agent config found. Set {CONFIG_ENV} or create {DEFAULT_CONFIG_FILE} in the current directory or one of its parents."
        )
    })
}

fn discover_config_path(current_dir: &Path) -> Option<PathBuf> {
    current_dir
        .ancestors()
        .map(|dir| dir.join(DEFAULT_CONFIG_FILE))
        .find(|path| path.exists())
}

fn required_field(value: Option<String>, field: &str) -> Result<String, String> {
    let value =
        value.ok_or_else(|| format!("ACP agent profile is missing required field '{field}'"))?;
    if value.trim().is_empty() {
        Err(format!(
            "ACP agent profile field '{field}' must not be empty"
        ))
    } else {
        Ok(value)
    }
}

fn expand_env_value(key: &str, value: &str) -> Result<String, String> {
    let Some(name) = env_reference(value) else {
        return Ok(value.to_string());
    };

    std::env::var(name).map_err(|_| {
        format!("ACP agent env '{key}' references missing environment variable '{name}'")
    })
}

fn env_reference(value: &str) -> Option<&str> {
    value
        .strip_prefix("${")
        .and_then(|rest| rest.strip_suffix('}'))
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn config(raw: &str) -> AgentConfigFile {
        serde_json::from_str(raw).unwrap()
    }

    #[test]
    fn uses_declared_default_profile() {
        let profile = config(
            r#"{
                "defaultProfile": "opencode",
                "profiles": [
                    {"id": "codex", "name": "Codex", "command": "codex"},
                    {"id": "opencode", "name": "OpenCode", "command": "opencode", "args": ["acp"]}
                ]
            }"#,
        )
        .resolve_default()
        .unwrap();

        assert_eq!(profile.id, "opencode");
        assert_eq!(profile.args, vec!["acp"]);
    }

    #[test]
    fn uses_single_profile_without_default() {
        let profile = config(
            r#"{
                "profiles": [
                    {"id": "solo", "name": "Solo", "command": "solo-agent"}
                ]
            }"#,
        )
        .resolve_default()
        .unwrap();

        assert_eq!(profile.id, "solo");
    }

    #[test]
    fn rejects_missing_required_fields() {
        let err = config(r#"{"profiles": [{"id": "x", "name": "Agent"}]}"#)
            .resolve_default()
            .unwrap_err();

        assert!(err.contains("command"));
    }

    #[test]
    fn rejects_unknown_default_profile() {
        let err = config(
            r#"{
                "defaultProfile": "missing",
                "profiles": [
                    {"id": "x", "name": "Agent", "command": "agent"}
                ]
            }"#,
        )
        .resolve_default()
        .unwrap_err();

        assert!(err.contains("does not exist"));
    }

    #[test]
    fn rejects_multiple_profiles_without_default() {
        let err = config(
            r#"{
                "profiles": [
                    {"id": "a", "name": "A", "command": "a"},
                    {"id": "b", "name": "B", "command": "b"}
                ]
            }"#,
        )
        .resolve_default()
        .unwrap_err();

        assert!(err.contains("multiple profiles"));
    }

    #[test]
    fn keeps_args_as_array_items() {
        let profile = config(
            r#"{
                "profiles": [
                    {"id": "x", "name": "Agent", "command": "agent", "args": ["--title", "hello world"]}
                ]
            }"#,
        )
        .resolve_default()
        .unwrap();

        assert_eq!(profile.args, vec!["--title", "hello world"]);
    }

    #[test]
    fn expands_env_references_and_keeps_literals() {
        let _guard = env_guard();
        std::env::set_var("ACP_PROFILE_TEST_SECRET", "expanded");

        let profile = config(
            r#"{
                "profiles": [
                    {
                        "id": "x",
                        "name": "Agent",
                        "command": "agent",
                        "env": {
                            "TOKEN": "${ACP_PROFILE_TEST_SECRET}",
                            "MODE": "literal-${ACP_PROFILE_TEST_SECRET}"
                        }
                    }
                ]
            }"#,
        )
        .resolve_default()
        .unwrap();

        assert_eq!(
            profile.env.get("TOKEN").map(String::as_str),
            Some("expanded")
        );
        assert_eq!(
            profile.env.get("MODE").map(String::as_str),
            Some("literal-${ACP_PROFILE_TEST_SECRET}")
        );

        std::env::remove_var("ACP_PROFILE_TEST_SECRET");
    }

    #[test]
    fn rejects_missing_env_reference() {
        let _guard = env_guard();
        std::env::remove_var("ACP_PROFILE_TEST_MISSING");

        let err = config(
            r#"{
                "profiles": [
                    {
                        "id": "x",
                        "name": "Agent",
                        "command": "agent",
                        "env": {"TOKEN": "${ACP_PROFILE_TEST_MISSING}"}
                    }
                ]
            }"#,
        )
        .resolve_default()
        .unwrap_err();

        assert!(err.contains("ACP_PROFILE_TEST_MISSING"));
    }

    #[test]
    fn discovers_config_in_parent_directory() {
        let root = std::env::temp_dir().join(format!(
            "voice-coding-acp-profile-test-{}",
            uuid::Uuid::new_v4()
        ));
        let nested = root.join("src-tauri");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join(DEFAULT_CONFIG_FILE), "{}").unwrap();

        let discovered = discover_config_path(&nested).unwrap();

        assert_eq!(discovered, root.join(DEFAULT_CONFIG_FILE));
        std::fs::remove_dir_all(root).unwrap();
    }
}
