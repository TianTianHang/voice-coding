use agent_client_protocol::schema::{
    AvailableCommandInput, ConfigOptionUpdate, ContentBlock, ContentChunk, CurrentModeUpdate,
    EmbeddedResourceResource, PermissionOption, PermissionOptionKind, Plan,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionInfoUpdate, SessionNotification, SessionUpdate, ToolCall,
    ToolCallContent, ToolCallUpdate, ToolCallUpdateFields,
};
use agent_client_protocol::Responder;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;

use super::events::{
    AgentAvailableCommand, AgentContentBlock, AgentDiff, AgentEvent, AgentEventKind,
    AgentEventOperation, AgentPlanEntry, AgentPlanSnapshot, AgentSessionInfo,
    AgentSessionStateUpdate, AgentTerminalRef, AgentToolContent, AgentToolLocation,
    AgentToolPayload,
};
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
    result_tracker: AgentResultTracker,
}

impl VoiceCodingAcpClient {
    pub fn new(
        app: AppHandle,
        pending: PendingPermissions,
        result_tracker: AgentResultTracker,
    ) -> Self {
        Self {
            app,
            pending,
            result_tracker,
        }
    }

    pub async fn handle_notification(&self, notification: SessionNotification) {
        let event = event_from_notification(notification);
        self.result_tracker.observe(&event);
        emit_agent_event(&self.app, event);
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

#[derive(Clone, Default)]
pub struct AgentResultTracker {
    latest: Arc<Mutex<Option<TrackedAgentResult>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedAgentResult {
    pub id: String,
    pub content: String,
}

impl AgentResultTracker {
    pub fn observe(&self, event: &AgentEvent) {
        if event.kind != AgentEventKind::Result {
            return;
        }

        let result_id = event.message_id.clone().unwrap_or_else(|| event.id.clone());
        let mut latest = self.latest.lock();
        match latest.as_mut() {
            Some(current) if current.id == result_id => {
                current.content = merge_append_text(&current.content, &event.content);
            }
            _ => {
                *latest = Some(TrackedAgentResult {
                    id: result_id,
                    content: event.content.clone(),
                });
            }
        }
    }

    pub fn latest(&self) -> Option<TrackedAgentResult> {
        self.latest.lock().clone()
    }
}

fn merge_append_text(current: &str, next: &str) -> String {
    if current.is_empty() {
        return next.to_string();
    }
    if next.is_empty() {
        return current.to_string();
    }
    if next.starts_with(current) {
        return next.to_string();
    }
    if current.ends_with(next) {
        return current.to_string();
    }
    format!("{current}{next}")
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
    event_from_update(&notification.update)
}

fn event_from_update(update: &SessionUpdate) -> AgentEvent {
    match update {
        SessionUpdate::UserMessageChunk(chunk) => text_chunk_event(
            AgentEventKind::Status,
            Some("User message".into()),
            chunk,
            update,
        ),
        SessionUpdate::AgentMessageChunk(chunk) => {
            text_chunk_event(AgentEventKind::Result, None, chunk, update)
        }
        SessionUpdate::AgentThoughtChunk(chunk) => text_chunk_event(
            AgentEventKind::Thinking,
            Some("Thinking".into()),
            chunk,
            update,
        ),
        SessionUpdate::ToolCall(tool_call) => tool_call_event(tool_call),
        SessionUpdate::ToolCallUpdate(tool_call) => tool_call_update_event(tool_call),
        SessionUpdate::Plan(plan) => plan_event(plan),
        SessionUpdate::AvailableCommandsUpdate(commands) => session_state_event(
            "Available commands updated",
            AgentSessionStateUpdate {
                available_commands: commands
                    .available_commands
                    .iter()
                    .map(|command| AgentAvailableCommand {
                        name: command.name.clone(),
                        description: command.description.clone(),
                        input_hint: command.input.as_ref().map(command_input_hint),
                    })
                    .collect(),
                current_mode_id: None,
                config_options: Vec::new(),
                session_info: None,
            },
            commands,
        ),
        SessionUpdate::CurrentModeUpdate(mode) => current_mode_event(mode),
        SessionUpdate::ConfigOptionUpdate(config) => config_event(config),
        SessionUpdate::SessionInfoUpdate(info) => session_info_event(info),
        other => fallback_event(other),
    }
}

fn text_chunk_event(
    kind: AgentEventKind,
    title: Option<String>,
    chunk: &ContentChunk,
    raw_update: &SessionUpdate,
) -> AgentEvent {
    let block = content_block_payload(&chunk.content);
    let content = content_block_text(&chunk.content).unwrap_or_else(|| block.summary.clone());
    let mut event = AgentEvent::new(kind, title, content, None)
        .with_message_id(chunk.message_id.clone())
        .with_operation(AgentEventOperation::Append);
    event.content_blocks = vec![block];
    event.raw = Some(to_value(raw_update));
    event
}

fn tool_call_event(tool_call: &ToolCall) -> AgentEvent {
    let tool = tool_call_payload(tool_call);
    let mut event = AgentEvent::new(
        AgentEventKind::Tool,
        Some(tool_call.title.clone()),
        tool_summary(&tool),
        None,
    )
    .with_tool_call_id(Some(tool.tool_call_id.clone()))
    .with_operation(AgentEventOperation::Create);
    event.tool = Some(tool);
    event.raw = Some(to_value(tool_call));
    event
}

fn tool_call_update_event(update: &ToolCallUpdate) -> AgentEvent {
    let tool = tool_call_update_payload(update);
    let mut event = AgentEvent::new(
        AgentEventKind::Tool,
        tool.title.clone().or_else(|| Some("Tool update".into())),
        tool_summary(&tool),
        None,
    )
    .with_tool_call_id(Some(tool.tool_call_id.clone()))
    .with_operation(AgentEventOperation::Update);
    event.tool = Some(tool);
    event.raw = Some(to_value(update));
    event
}

fn plan_event(plan: &Plan) -> AgentEvent {
    let snapshot = AgentPlanSnapshot {
        entries: plan
            .entries
            .iter()
            .map(|entry| AgentPlanEntry {
                content: entry.content.clone(),
                priority: json_string(&entry.priority),
                status: json_string(&entry.status),
            })
            .collect(),
    };
    let mut event = AgentEvent::new(
        AgentEventKind::Status,
        Some("Plan".into()),
        format!("Plan updated: {} entries", snapshot.entries.len()),
        None,
    )
    .with_operation(AgentEventOperation::Replace);
    event.plan = Some(snapshot);
    event.raw = Some(to_value(plan));
    event
}

fn current_mode_event(mode: &CurrentModeUpdate) -> AgentEvent {
    session_state_event(
        &format!("Current mode: {}", mode.current_mode_id),
        AgentSessionStateUpdate {
            available_commands: Vec::new(),
            current_mode_id: Some(mode.current_mode_id.to_string()),
            config_options: Vec::new(),
            session_info: None,
        },
        mode,
    )
}

fn config_event(config: &ConfigOptionUpdate) -> AgentEvent {
    session_state_event(
        &format!(
            "Configuration updated: {} options",
            config.config_options.len()
        ),
        AgentSessionStateUpdate {
            available_commands: Vec::new(),
            current_mode_id: None,
            config_options: config.config_options.iter().map(to_value).collect(),
            session_info: None,
        },
        config,
    )
}

fn session_info_event(info: &SessionInfoUpdate) -> AgentEvent {
    let state = AgentSessionStateUpdate {
        available_commands: Vec::new(),
        current_mode_id: None,
        config_options: Vec::new(),
        session_info: Some(AgentSessionInfo {
            title: info.title.as_opt_ref().map(|value| value.cloned()),
            updated_at: info.updated_at.as_opt_ref().map(|value| value.cloned()),
        }),
    };
    session_state_event("Session info updated", state, info)
}

fn session_state_event(
    content: &str,
    session_state: AgentSessionStateUpdate,
    raw: &impl serde::Serialize,
) -> AgentEvent {
    let mut event = AgentEvent::new(
        AgentEventKind::Status,
        Some("Session state".into()),
        content,
        None,
    )
    .with_operation(AgentEventOperation::SessionState);
    event.session_state = Some(session_state);
    event.raw = Some(to_value(raw));
    event
}

fn fallback_event(raw: &impl serde::Serialize) -> AgentEvent {
    let mut event = AgentEvent::new(
        AgentEventKind::Status,
        Some("Unknown update".into()),
        stringify(raw),
        None,
    )
    .with_operation(AgentEventOperation::Fallback);
    event.raw = Some(to_value(raw));
    event
}

fn tool_call_payload(tool_call: &ToolCall) -> AgentToolPayload {
    AgentToolPayload {
        tool_call_id: tool_call.tool_call_id.to_string(),
        title: Some(tool_call.title.clone()),
        kind: Some(json_string(&tool_call.kind)),
        status: Some(json_string(&tool_call.status)),
        content: tool_call.content.iter().map(tool_content_payload).collect(),
        locations: tool_call
            .locations
            .iter()
            .map(|location| AgentToolLocation {
                path: location.path.display().to_string(),
                line: location.line,
            })
            .collect(),
        raw_input: tool_call.raw_input.clone(),
        raw_output: tool_call.raw_output.clone(),
    }
}

fn tool_call_update_payload(update: &ToolCallUpdate) -> AgentToolPayload {
    let ToolCallUpdateFields {
        kind,
        status,
        title,
        content,
        locations,
        raw_input,
        raw_output,
        ..
    } = &update.fields;

    AgentToolPayload {
        tool_call_id: update.tool_call_id.to_string(),
        title: title.clone(),
        kind: kind.as_ref().map(json_string),
        status: status.as_ref().map(json_string),
        content: content
            .as_ref()
            .map(|items| items.iter().map(tool_content_payload).collect())
            .unwrap_or_default(),
        locations: locations
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .map(|location| AgentToolLocation {
                        path: location.path.display().to_string(),
                        line: location.line,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        raw_input: raw_input.clone(),
        raw_output: raw_output.clone(),
    }
}

fn tool_content_payload(content: &ToolCallContent) -> AgentToolContent {
    match content {
        ToolCallContent::Content(content) => {
            let block = content_block_payload(&content.content);
            AgentToolContent {
                kind: block.kind.clone(),
                summary: block.summary.clone(),
                content: Some(block),
                diff: None,
                terminal: None,
            }
        }
        ToolCallContent::Diff(diff) => {
            let payload = AgentDiff {
                path: diff.path.display().to_string(),
                old_text: diff.old_text.clone(),
                new_text: diff.new_text.clone(),
            };
            AgentToolContent {
                kind: "diff".into(),
                summary: format!("Diff: {}", payload.path),
                content: None,
                diff: Some(payload),
                terminal: None,
            }
        }
        ToolCallContent::Terminal(terminal) => {
            let payload = AgentTerminalRef {
                terminal_id: terminal.terminal_id.to_string(),
            };
            AgentToolContent {
                kind: "terminal".into(),
                summary: format!("Terminal: {}", payload.terminal_id),
                content: None,
                diff: None,
                terminal: Some(payload),
            }
        }
        other => AgentToolContent {
            kind: "unknown".into(),
            summary: stringify(other),
            content: None,
            diff: None,
            terminal: None,
        },
    }
}

fn content_block_text(content: &ContentBlock) -> Option<String> {
    match content {
        ContentBlock::Text(text) => Some(text.text.clone()),
        _ => None,
    }
}

fn content_block_payload(content: &ContentBlock) -> AgentContentBlock {
    match content {
        ContentBlock::Text(text) => AgentContentBlock {
            kind: "text".into(),
            summary: text.text.clone(),
            text: Some(text.text.clone()),
            mime_type: None,
            uri: None,
            name: None,
            raw: Some(to_value(content)),
        },
        ContentBlock::Image(image) => AgentContentBlock {
            kind: "image".into(),
            summary: format!(
                "Image content ({}){}",
                image.mime_type,
                image
                    .uri
                    .as_ref()
                    .map(|uri| format!(": {uri}"))
                    .unwrap_or_default()
            ),
            text: None,
            mime_type: Some(image.mime_type.clone()),
            uri: image.uri.clone(),
            name: None,
            raw: Some(to_value(content)),
        },
        ContentBlock::Audio(audio) => AgentContentBlock {
            kind: "audio".into(),
            summary: format!("Audio content ({})", audio.mime_type),
            text: None,
            mime_type: Some(audio.mime_type.clone()),
            uri: None,
            name: None,
            raw: Some(to_value(content)),
        },
        ContentBlock::ResourceLink(resource) => AgentContentBlock {
            kind: "resourceLink".into(),
            summary: format!("Resource link: {} ({})", resource.name, resource.uri),
            text: None,
            mime_type: resource.mime_type.clone(),
            uri: Some(resource.uri.clone()),
            name: Some(resource.name.clone()),
            raw: Some(to_value(content)),
        },
        ContentBlock::Resource(resource) => match &resource.resource {
            EmbeddedResourceResource::TextResourceContents(text) => AgentContentBlock {
                kind: "resource".into(),
                summary: format!("Embedded text resource: {}", text.uri),
                text: Some(text.text.clone()),
                mime_type: text.mime_type.clone(),
                uri: Some(text.uri.clone()),
                name: None,
                raw: Some(to_value(content)),
            },
            EmbeddedResourceResource::BlobResourceContents(blob) => AgentContentBlock {
                kind: "resource".into(),
                summary: format!("Embedded binary resource: {}", blob.uri),
                text: None,
                mime_type: blob.mime_type.clone(),
                uri: Some(blob.uri.clone()),
                name: None,
                raw: Some(to_value(content)),
            },
            other => AgentContentBlock {
                kind: "resource".into(),
                summary: stringify(other),
                text: None,
                mime_type: None,
                uri: None,
                name: None,
                raw: Some(to_value(content)),
            },
        },
        other => AgentContentBlock {
            kind: "unknown".into(),
            summary: stringify(other),
            text: None,
            mime_type: None,
            uri: None,
            name: None,
            raw: Some(to_value(content)),
        },
    }
}

fn tool_summary(tool: &AgentToolPayload) -> String {
    let mut parts = Vec::new();
    if let Some(status) = &tool.status {
        parts.push(format!("status: {status}"));
    }
    if let Some(kind) = &tool.kind {
        parts.push(format!("kind: {kind}"));
    }
    parts.extend(tool.content.iter().map(|content| content.summary.clone()));
    if parts.is_empty() {
        tool.title
            .clone()
            .unwrap_or_else(|| format!("Tool {}", tool.tool_call_id))
    } else {
        parts.join("\n")
    }
}

fn command_input_hint(input: &AvailableCommandInput) -> String {
    stringify(input)
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

fn to_value<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

fn json_string<T: serde::Serialize>(value: &T) -> String {
    match to_value(value) {
        serde_json::Value::String(value) => value,
        value => value.to_string(),
    }
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
            SessionUpdate::AgentMessageChunk(
                ContentChunk::new(ContentBlock::Text(TextContent::new("done")))
                    .message_id(Some("550e8400-e29b-41d4-a716-446655440000".into())),
            ),
        ));

        assert_eq!(event.kind, AgentEventKind::Result);
        assert_eq!(event.operation, Some(AgentEventOperation::Append));
        assert_eq!(
            event.message_id.as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(event.content, "done");
    }

    #[test]
    fn result_tracker_only_records_result_events_and_merges_chunks() {
        let tracker = AgentResultTracker::default();
        tracker.observe(&AgentEvent::status("Working"));
        assert!(tracker.latest().is_none());

        let first = AgentEvent::new(AgentEventKind::Result, None, "hel", None)
            .with_message_id(Some("message-1".to_string()))
            .with_operation(AgentEventOperation::Append);
        tracker.observe(&first);
        let second = AgentEvent::new(AgentEventKind::Result, None, "lo", None)
            .with_message_id(Some("message-1".to_string()))
            .with_operation(AgentEventOperation::Append);
        tracker.observe(&second);

        assert_eq!(
            tracker.latest(),
            Some(TrackedAgentResult {
                id: "message-1".to_string(),
                content: "hello".to_string(),
            })
        );

        tracker.observe(&second);
        assert_eq!(tracker.latest().unwrap().content, "hello");
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
        assert_eq!(event.tool_call_id.as_deref(), Some("t1"));
        assert_eq!(event.operation, Some(AgentEventOperation::Create));
        assert_eq!(
            event.tool.as_ref().unwrap().title.as_deref(),
            Some("Run tests")
        );
    }

    #[test]
    fn maps_user_and_thought_chunks() {
        let user = event_from_notification(notification_json(serde_json::json!({
            "sessionUpdate": "user_message_chunk",
            "content": { "type": "text", "text": "hello" },
            "messageId": "550e8400-e29b-41d4-a716-446655440001"
        })));
        let thought = event_from_notification(notification_json(serde_json::json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": { "type": "text", "text": "hmm" },
            "messageId": "550e8400-e29b-41d4-a716-446655440002"
        })));

        assert_eq!(user.kind, AgentEventKind::Status);
        assert_eq!(user.operation, Some(AgentEventOperation::Append));
        assert_eq!(
            user.message_id.as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440001")
        );
        assert_eq!(thought.kind, AgentEventKind::Thinking);
        assert_eq!(thought.operation, Some(AgentEventOperation::Append));
        assert_eq!(
            thought.message_id.as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440002")
        );
    }

    #[test]
    fn maps_tool_update_and_content_variants() {
        let event = event_from_notification(notification_json(serde_json::json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "tool-1",
            "title": "Edit file",
            "kind": "edit",
            "status": "completed",
            "content": [
                {
                    "type": "content",
                    "content": { "type": "image", "data": "abc", "mimeType": "image/png", "uri": "file:///tmp/a.png" }
                },
                {
                    "type": "diff",
                    "path": "src/main.rs",
                    "oldText": "old",
                    "newText": "new"
                },
                {
                    "type": "terminal",
                    "terminalId": "term-1"
                }
            ],
            "locations": [{ "path": "src/main.rs", "line": 7 }],
            "rawInput": { "path": "src/main.rs" },
            "rawOutput": { "ok": true }
        })));

        let tool = event.tool.as_ref().unwrap();
        assert_eq!(event.kind, AgentEventKind::Tool);
        assert_eq!(event.operation, Some(AgentEventOperation::Update));
        assert_eq!(tool.tool_call_id, "tool-1");
        assert_eq!(tool.kind.as_deref(), Some("edit"));
        assert_eq!(tool.status.as_deref(), Some("completed"));
        assert_eq!(tool.locations[0].path, "src/main.rs");
        assert_eq!(tool.content[0].kind, "image");
        assert_eq!(tool.content[1].diff.as_ref().unwrap().path, "src/main.rs");
        assert_eq!(
            tool.content[2].terminal.as_ref().unwrap().terminal_id,
            "term-1"
        );
    }

    #[test]
    fn maps_plan_snapshot() {
        let event = event_from_notification(notification_json(serde_json::json!({
            "sessionUpdate": "plan",
            "entries": [
                { "content": "Implement", "priority": "high", "status": "in_progress" },
                { "content": "Verify", "priority": "medium", "status": "pending" }
            ]
        })));

        assert_eq!(event.operation, Some(AgentEventOperation::Replace));
        let plan = event.plan.as_ref().unwrap();
        assert_eq!(plan.entries.len(), 2);
        assert_eq!(plan.entries[0].priority, "high");
        assert_eq!(plan.entries[0].status, "in_progress");
    }

    #[test]
    fn maps_session_state_updates() {
        let commands = event_from_notification(notification_json(serde_json::json!({
            "sessionUpdate": "available_commands_update",
            "availableCommands": [
                { "name": "create_plan", "description": "Create a plan" }
            ]
        })));
        let mode = event_from_notification(notification_json(serde_json::json!({
            "sessionUpdate": "current_mode_update",
            "currentModeId": "build"
        })));
        let config = event_from_notification(notification_json(serde_json::json!({
            "sessionUpdate": "config_option_update",
            "configOptions": []
        })));
        let info = event_from_notification(notification_json(serde_json::json!({
            "sessionUpdate": "session_info_update",
            "title": "Voice task",
            "updatedAt": "2026-04-28T00:00:00Z"
        })));

        assert_eq!(commands.operation, Some(AgentEventOperation::SessionState));
        assert_eq!(
            commands.session_state.as_ref().unwrap().available_commands[0].name,
            "create_plan"
        );
        assert_eq!(
            mode.session_state
                .as_ref()
                .unwrap()
                .current_mode_id
                .as_deref(),
            Some("build")
        );
        assert!(config
            .session_state
            .as_ref()
            .unwrap()
            .config_options
            .is_empty());
        assert_eq!(
            info.session_state
                .as_ref()
                .unwrap()
                .session_info
                .as_ref()
                .unwrap()
                .title
                .as_ref()
                .unwrap()
                .as_deref(),
            Some("Voice task")
        );
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

    fn notification_json(update: serde_json::Value) -> SessionNotification {
        serde_json::from_value(serde_json::json!({
            "sessionId": "s1",
            "update": update,
        }))
        .unwrap()
    }
}
