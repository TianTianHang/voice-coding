use serde::{Deserialize, Serialize};

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
pub struct AgentEvent {
    pub id: String,
    pub kind: AgentEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub content: String,
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
            title,
            content: content.into(),
            confirmation_id,
            confirm_status: None,
            created_at,
        }
    }
}

pub fn current_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
