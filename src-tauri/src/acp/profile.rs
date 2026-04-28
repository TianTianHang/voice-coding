use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AgentProfile {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
}

impl AgentProfile {
    pub fn from_environment() -> Result<Self, String> {
        let command = std::env::var("ACP_AGENT_CMD").map_err(|_| {
            "ACP_AGENT_CMD is not set. Set it to a compatible ACP agent command.".to_string()
        })?;
        let args = std::env::var("ACP_AGENT_ARGS")
            .ok()
            .map(|raw| shell_words(&raw))
            .unwrap_or_default();
        let cwd = std::env::var("ACP_AGENT_CWD").ok().map(PathBuf::from);

        Ok(Self {
            name: std::env::var("ACP_AGENT_NAME").unwrap_or_else(|_| "Configured ACP Agent".into()),
            command,
            args,
            cwd,
            env: HashMap::new(),
        })
    }
}

fn shell_words(raw: &str) -> Vec<String> {
    raw.split_whitespace().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::shell_words;

    #[test]
    fn parses_simple_arg_list() {
        assert_eq!(shell_words("acp --stdio"), vec!["acp", "--stdio"]);
    }
}
