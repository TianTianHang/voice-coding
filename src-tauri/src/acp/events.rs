use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl AgentStatus {
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            profile_name: None,
            session_id: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentEventKind {
    Thinking,
    Tool,
    Result,
    Diff,
    Confirm,
    Error,
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentEventOperation {
    Append,
    Create,
    Update,
    Replace,
    SessionState,
    Fallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentContentBlock {
    pub kind: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentDiff {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_text: Option<String>,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentTerminalRef {
    pub terminal_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolContent {
    pub kind: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<AgentContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<AgentDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<AgentTerminalRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolLocation {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolPayload {
    pub tool_call_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<AgentToolContent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<AgentToolLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlanEntry {
    pub content: String,
    pub priority: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlanSnapshot {
    pub entries: Vec<AgentPlanEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionStateUpdate {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_commands: Vec<AgentAvailableCommand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_mode_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_options: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_info: Option<AgentSessionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentAvailableCommand {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub id: String,
    pub kind: AgentEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<AgentEventOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_blocks: Vec<AgentContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<AgentToolPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<AgentDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<AgentTerminalRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<AgentPlanSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_state: Option<AgentSessionStateUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirm_status: Option<String>,
    pub created_at: u64,
}

impl AgentEvent {
    pub fn status(content: impl Into<String>) -> Self {
        Self::new(AgentEventKind::Status, None, content, None)
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self::new(AgentEventKind::Error, None, content, None)
    }

    pub fn new(
        kind: AgentEventKind,
        title: Option<String>,
        content: impl Into<String>,
        confirmation_id: Option<String>,
    ) -> Self {
        let created_at = current_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            message_id: None,
            tool_call_id: None,
            operation: None,
            title,
            content: content.into(),
            content_blocks: Vec::new(),
            tool: None,
            diff: None,
            terminal: None,
            plan: None,
            session_state: None,
            raw: None,
            confirmation_id,
            confirm_status: None,
            created_at,
        }
    }

    pub fn with_message_id(mut self, message_id: Option<String>) -> Self {
        self.message_id = message_id;
        self
    }

    pub fn with_tool_call_id(mut self, tool_call_id: Option<String>) -> Self {
        self.tool_call_id = tool_call_id;
        self
    }

    pub fn with_operation(mut self, operation: AgentEventOperation) -> Self {
        self.operation = Some(operation);
        self
    }
}

pub fn current_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
