use agent_client_protocol::schema::{
    ContentBlock, PermissionOption, PermissionOptionKind, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
    SessionNotification, SessionUpdate,
};
use agent_client_protocol::Responder;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;

use super::events::{AgentEvent, AgentEventKind};
use super::session::emit_agent_event;

pub type PendingPermissions = Arc<Mutex<HashMap<String, PendingPermission>>>;

pub struct PendingPermission {
    responder: Responder<RequestPermissionResponse>,
    accept_option: Option<String>,
    reject_option: Option<String>,
}

#[derive(Clone)]
pub struct VoiceCodingAcpClient {
    app: AppHandle,
    pending: PendingPermissions,
}

impl VoiceCodingAcpClient {
    pub fn new(app: AppHandle, pending: PendingPermissions) -> Self {
        Self { app, pending }
    }

    pub async fn handle_notification(&self, notification: SessionNotification) {
        emit_agent_event(&self.app, event_from_notification(notification));
    }

    pub fn handle_permission_request(
        &self,
        request: RequestPermissionRequest,
        responder: Responder<RequestPermissionResponse>,
    ) -> Result<(), String> {
        let confirmation_id = uuid::Uuid::new_v4().to_string();
        let (accept_option, reject_option) = permission_option_ids(&request.options);

        self.pending.lock().insert(
            confirmation_id.clone(),
            PendingPermission {
                responder,
                accept_option,
                reject_option,
            },
        );

        let mut event = AgentEvent::new(
            AgentEventKind::Confirm,
            Some("Permission requested".into()),
            permission_content(&request),
            Some(confirmation_id),
        );
        event.confirm_status = Some("pending".into());
        emit_agent_event(&self.app, event);
        Ok(())
    }
}

pub fn respond_pending_permission(
    pending: &PendingPermissions,
    confirmation_id: &str,
    accepted: bool,
) -> Result<(), String> {
    let pending_permission = pending
        .lock()
        .remove(confirmation_id)
        .ok_or_else(|| format!("Unknown ACP confirmation id '{confirmation_id}'"))?;

    let outcome = if accepted {
        let option_id = pending_permission
            .accept_option
            .ok_or_else(|| "Permission request did not include an allow option".to_string())?;
        RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id))
    } else if let Some(option_id) = pending_permission.reject_option {
        RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id))
    } else {
        RequestPermissionOutcome::Cancelled
    };

    pending_permission
        .responder
        .respond(RequestPermissionResponse::new(outcome))
        .map_err(|e| e.to_string())
}

pub fn event_from_notification(notification: SessionNotification) -> AgentEvent {
    let (kind, title, content) = describe_update(&notification.update);
    AgentEvent::new(kind, title, content, None)
}

fn describe_update(update: &SessionUpdate) -> (AgentEventKind, Option<String>, String) {
    match update {
        SessionUpdate::AgentMessageChunk(chunk) => (
            AgentEventKind::Result,
            None,
            content_block_text(&chunk.content).unwrap_or_else(|| stringify(update)),
        ),
        SessionUpdate::AgentThoughtChunk(chunk) => (
            AgentEventKind::Thinking,
            Some("Thinking".into()),
            content_block_text(&chunk.content).unwrap_or_else(|| stringify(update)),
        ),
        SessionUpdate::ToolCall(tool_call) => (
            AgentEventKind::Tool,
            Some("Tool call".into()),
            stringify(tool_call),
        ),
        SessionUpdate::ToolCallUpdate(tool_call) => (
            AgentEventKind::Tool,
            Some("Tool update".into()),
            stringify(tool_call),
        ),
        SessionUpdate::Plan(plan) => (AgentEventKind::Status, Some("Plan".into()), stringify(plan)),
        SessionUpdate::CurrentModeUpdate(mode) => (
            AgentEventKind::Status,
            Some("Mode".into()),
            format!("Current mode: {}", mode.current_mode_id),
        ),
        SessionUpdate::SessionInfoUpdate(info) => (
            AgentEventKind::Status,
            Some("Session".into()),
            stringify(info),
        ),
        other => (
            AgentEventKind::Status,
            Some("Update".into()),
            stringify(other),
        ),
    }
}

fn content_block_text(content: &ContentBlock) -> Option<String> {
    match content {
        ContentBlock::Text(text) => Some(text.text.clone()),
        _ => None,
    }
}

fn permission_content(request: &RequestPermissionRequest) -> String {
    let mut lines = vec![stringify(&request.tool_call)];
    if !request.options.is_empty() {
        lines.push("Options:".into());
        lines.extend(
            request
                .options
                .iter()
                .map(|option| format!("- {} ({:?})", option.name, option.kind)),
        );
    }
    lines.join("\n")
}

fn permission_option_ids(options: &[PermissionOption]) -> (Option<String>, Option<String>) {
    let accept = options
        .iter()
        .find(|option| {
            matches!(
                option.kind,
                PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
            )
        })
        .or_else(|| options.first())
        .map(|option| option.option_id.to_string());

    let reject = options
        .iter()
        .find(|option| {
            matches!(
                option.kind,
                PermissionOptionKind::RejectOnce | PermissionOptionKind::RejectAlways
            )
        })
        .map(|option| option.option_id.to_string());

    (accept, reject)
}

fn stringify<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::{
        ContentChunk, PermissionOption, PermissionOptionKind, SessionId, TextContent,
    };

    #[test]
    fn maps_result_notification_text() {
        let event = event_from_notification(SessionNotification::new(
            SessionId::new("s1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("done"),
            ))),
        ));

        assert_eq!(event.kind, AgentEventKind::Result);
        assert_eq!(event.content, "done");
    }

    #[test]
    fn maps_tool_notification() {
        let value = serde_json::json!({
            "sessionId": "s1",
            "update": {
                "sessionUpdate": "tool_call",
                "toolCallId": "t1",
                "title": "Run tests"
            }
        });
        let notification = serde_json::from_value::<SessionNotification>(value).unwrap();
        let event = event_from_notification(notification);

        assert_eq!(event.kind, AgentEventKind::Tool);
    }

    #[test]
    fn selects_allow_and_reject_options() {
        let (allow, reject) = permission_option_ids(&[
            PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
            PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
        ]);

        assert_eq!(allow.as_deref(), Some("allow"));
        assert_eq!(reject.as_deref(), Some("reject"));
    }
}
