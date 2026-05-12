use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use super::events::{
    current_millis, AgentAvailableCommand, AgentContentBlock, AgentDiff, AgentEvent,
    AgentEventKind, AgentEventOperation, AgentPlanSnapshot, AgentSessionInfo,
    AgentSessionStateUpdate, AgentTerminalRef, AgentToolPayload,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentTimelineSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_turn_id: Option<String>,
    pub sequence: u64,
    pub items: Vec<AgentTimelineItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<AgentPlanSnapshot>,
    pub session_state: AgentSessionState,
    pub pending_confirmations: Vec<AgentConfirmationSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentTimelinePatch {
    Reset {
        snapshot: AgentTimelineSnapshot,
    },
    UpsertItem {
        item: AgentTimelineItem,
    },
    UpdatePlan {
        plan: Option<AgentPlanSnapshot>,
    },
    UpdateSessionState {
        session_state: AgentSessionState,
    },
    ResolveConfirmation {
        confirmation_id: String,
        status: AgentConfirmationStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        item: Option<AgentTimelineItem>,
    },
    StreamError {
        item: AgentTimelineItem,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentTimelineItemKind {
    Thinking,
    Message,
    Tool,
    Diff,
    Confirmation,
    Error,
    Status,
    Fallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentTimelineItem {
    pub id: String,
    pub kind: AgentTimelineItemKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
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
    pub raw: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<AgentConfirmationSnapshot>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionState {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_commands: Vec<AgentAvailableCommand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_mode_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_options: Vec<Value>,
    pub session_info: AgentSessionInfoState,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionInfoState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfirmationSnapshot {
    pub confirmation_id: String,
    pub status: AgentConfirmationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentConfirmationStatus {
    Pending,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Default)]
pub struct AgentStreamRuntime {
    inner: Arc<Mutex<AgentTimelineReducer>>,
}

impl AgentStreamRuntime {
    pub fn snapshot(&self) -> AgentTimelineSnapshot {
        self.inner.lock().snapshot()
    }

    pub fn reset_session(&self, app: Option<&AppHandle>, session_id: Option<String>) {
        let patch = {
            let mut inner = self.inner.lock();
            inner.reset(session_id)
        };
        emit_timeline_patch(app, &patch);
    }

    pub fn begin_turn(&self, app: Option<&AppHandle>, turn_id: String) {
        let patch = {
            let mut inner = self.inner.lock();
            inner.begin_turn(turn_id)
        };
        emit_timeline_patch(app, &patch);
    }

    pub fn finish_turn(&self, turn_id: &str) {
        self.inner.lock().finish_turn(turn_id);
    }

    pub fn handle_agent_event(
        &self,
        app: Option<&AppHandle>,
        event: AgentEvent,
    ) -> Option<AgentTimelinePatch> {
        let patch = self.inner.lock().apply_event(event);
        if let Some(patch) = &patch {
            emit_timeline_patch(app, patch);
        }
        patch
    }

    pub fn resolve_confirmation(
        &self,
        app: Option<&AppHandle>,
        confirmation_id: &str,
        accepted: bool,
    ) -> Option<AgentTimelinePatch> {
        let patch = self
            .inner
            .lock()
            .resolve_confirmation(confirmation_id, accepted);
        if let Some(patch) = &patch {
            emit_timeline_patch(app, patch);
        }
        patch
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentTimelineReducer {
    session_id: Option<String>,
    current_turn_id: Option<String>,
    sequence: u64,
    items: Vec<AgentTimelineItem>,
    plan: Option<AgentPlanSnapshot>,
    session_state: AgentSessionState,
    pending_confirmations: HashMap<String, AgentConfirmationSnapshot>,
    closed_turn_ids: HashSet<String>,
}

impl AgentTimelineReducer {
    pub fn snapshot(&self) -> AgentTimelineSnapshot {
        AgentTimelineSnapshot {
            session_id: self.session_id.clone(),
            current_turn_id: self.current_turn_id.clone(),
            sequence: self.sequence,
            items: self.items.clone(),
            plan: self.plan.clone(),
            session_state: self.session_state.clone(),
            pending_confirmations: self.pending_confirmations.values().cloned().collect(),
        }
    }

    pub fn reset(&mut self, session_id: Option<String>) -> AgentTimelinePatch {
        *self = Self {
            session_id,
            ..Self::default()
        };
        self.sequence += 1;
        AgentTimelinePatch::Reset {
            snapshot: self.snapshot(),
        }
    }

    pub fn begin_turn(&mut self, turn_id: String) -> AgentTimelinePatch {
        self.sequence += 1;
        self.current_turn_id = Some(turn_id);
        AgentTimelinePatch::Reset {
            snapshot: self.snapshot(),
        }
    }

    pub fn finish_turn(&mut self, turn_id: &str) {
        if self.current_turn_id.as_deref() == Some(turn_id) {
            self.current_turn_id = None;
        }
        self.closed_turn_ids.insert(turn_id.to_string());
    }

    pub fn apply_event(&mut self, event: AgentEvent) -> Option<AgentTimelinePatch> {
        if let Some(plan) = event.plan {
            self.plan = Some(plan);
            return Some(AgentTimelinePatch::UpdatePlan {
                plan: self.plan.clone(),
            });
        }

        if let Some(update) = event.session_state {
            self.merge_session_state(update);
            return Some(AgentTimelinePatch::UpdateSessionState {
                session_state: self.session_state.clone(),
            });
        }

        if self.should_ignore_turn_event(&event) {
            return None;
        }

        let item = self.item_from_event(event);
        if matches!(item.kind, AgentTimelineItemKind::Error) {
            self.upsert_item(item.clone());
            return Some(AgentTimelinePatch::StreamError { item });
        }

        if let Some(confirmation) = &item.confirmation {
            if confirmation.status == AgentConfirmationStatus::Pending {
                self.pending_confirmations
                    .insert(confirmation.confirmation_id.clone(), confirmation.clone());
            }
        }

        let item = self.upsert_item(item);
        Some(AgentTimelinePatch::UpsertItem { item })
    }

    pub fn resolve_confirmation(
        &mut self,
        confirmation_id: &str,
        accepted: bool,
    ) -> Option<AgentTimelinePatch> {
        let status = if accepted {
            AgentConfirmationStatus::Accepted
        } else {
            AgentConfirmationStatus::Rejected
        };
        self.pending_confirmations.remove(confirmation_id)?;
        let item = self.items.iter_mut().find(|item| {
            item.confirmation
                .as_ref()
                .map(|confirmation| confirmation.confirmation_id == confirmation_id)
                .unwrap_or(false)
        });

        let item = item.map(|item| {
            self.sequence += 1;
            item.sequence = self.sequence;
            item.updated_at = current_millis();
            if let Some(confirmation) = &mut item.confirmation {
                confirmation.status = status;
            }
            item.clone()
        });

        Some(AgentTimelinePatch::ResolveConfirmation {
            confirmation_id: confirmation_id.to_string(),
            status,
            item,
        })
    }

    fn item_from_event(&mut self, event: AgentEvent) -> AgentTimelineItem {
        self.sequence += 1;
        let kind = timeline_kind(&event);
        let turn_id = if is_session_level(&event) {
            None
        } else {
            self.current_turn_id.clone()
        };
        let confirmation =
            event
                .confirmation_id
                .clone()
                .map(|confirmation_id| AgentConfirmationSnapshot {
                    confirmation_id,
                    status: confirmation_status(event.confirm_status.as_deref()),
                    turn_id: turn_id.clone(),
                    content: event.content.clone(),
                });

        AgentTimelineItem {
            id: stable_item_id(&event, &kind),
            kind,
            session_id: self.session_id.clone(),
            turn_id,
            sequence: self.sequence,
            message_id: event.message_id,
            tool_call_id: event.tool_call_id,
            title: event.title,
            content: event.content,
            content_blocks: event.content_blocks,
            tool: event.tool,
            diff: event.diff,
            terminal: event.terminal,
            raw: event.raw,
            confirmation,
            created_at: event.created_at,
            updated_at: current_millis(),
        }
    }

    fn upsert_item(&mut self, item: AgentTimelineItem) -> AgentTimelineItem {
        if let Some(index) = self.find_merge_target(&item) {
            let current = &mut self.items[index];
            merge_timeline_item(current, item);
            return current.clone();
        }

        self.items.push(item.clone());
        item
    }

    fn find_merge_target(&self, item: &AgentTimelineItem) -> Option<usize> {
        if item.kind == AgentTimelineItemKind::Tool {
            if let Some(tool_call_id) = &item.tool_call_id {
                return self.items.iter().position(|current| {
                    current.kind == AgentTimelineItemKind::Tool
                        && current.tool_call_id.as_ref() == Some(tool_call_id)
                });
            }
        }

        if matches!(
            item.kind,
            AgentTimelineItemKind::Message | AgentTimelineItemKind::Thinking
        ) {
            if let Some(message_id) = &item.message_id {
                return self.items.iter().rposition(|current| {
                    current.kind == item.kind && current.message_id.as_ref() == Some(message_id)
                });
            }

            return self.items.iter().rposition(|current| {
                current.kind == item.kind
                    && current.turn_id == item.turn_id
                    && current.message_id.is_none()
                    && current.tool_call_id.is_none()
            });
        }

        self.items.iter().position(|current| current.id == item.id)
    }

    fn should_ignore_turn_event(&self, event: &AgentEvent) -> bool {
        if is_session_level(event) || event.kind == AgentEventKind::Error {
            return false;
        }
        self.current_turn_id.is_none()
    }

    fn merge_session_state(&mut self, update: AgentSessionStateUpdate) {
        if !update.available_commands.is_empty() {
            self.session_state.available_commands = update.available_commands;
        }
        if let Some(current_mode_id) = update.current_mode_id {
            self.session_state.current_mode_id = Some(current_mode_id);
        }
        if !update.config_options.is_empty() {
            self.session_state.config_options = update.config_options;
        }
        if let Some(info) = update.session_info {
            merge_session_info(&mut self.session_state.session_info, info);
        }
    }
}

fn merge_timeline_item(current: &mut AgentTimelineItem, next: AgentTimelineItem) {
    current.sequence = next.sequence;
    current.updated_at = next.updated_at;
    current.title = next.title.or_else(|| current.title.clone());
    current.content = merge_append_text(&current.content, &next.content);
    current.raw = next.raw.or_else(|| current.raw.clone());
    current.diff = next.diff.or_else(|| current.diff.clone());
    current.terminal = next.terminal.or_else(|| current.terminal.clone());
    if !next.content_blocks.is_empty() {
        current.content_blocks.extend(next.content_blocks);
    }
    current.tool = merge_tool_payload(current.tool.clone(), next.tool);
    if next.confirmation.is_some() {
        current.confirmation = next.confirmation;
    }
}

fn merge_tool_payload(
    current: Option<AgentToolPayload>,
    next: Option<AgentToolPayload>,
) -> Option<AgentToolPayload> {
    match (current, next) {
        (None, None) => None,
        (Some(current), None) => Some(current),
        (None, Some(next)) => Some(next),
        (Some(current), Some(next)) => Some(AgentToolPayload {
            tool_call_id: next.tool_call_id,
            title: next.title.or(current.title),
            kind: next.kind.or(current.kind),
            status: next.status.or(current.status),
            content: if next.content.is_empty() {
                current.content
            } else {
                next.content
            },
            locations: if next.locations.is_empty() {
                current.locations
            } else {
                next.locations
            },
            raw_input: next.raw_input.or(current.raw_input),
            raw_output: next.raw_output.or(current.raw_output),
        }),
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

fn merge_session_info(current: &mut AgentSessionInfoState, update: AgentSessionInfo) {
    if let Some(title) = update.title {
        current.title = title;
    }
    if let Some(updated_at) = update.updated_at {
        current.updated_at = updated_at;
    }
}

fn timeline_kind(event: &AgentEvent) -> AgentTimelineItemKind {
    match event.kind {
        AgentEventKind::Thinking => AgentTimelineItemKind::Thinking,
        AgentEventKind::Tool => AgentTimelineItemKind::Tool,
        AgentEventKind::Result => AgentTimelineItemKind::Message,
        AgentEventKind::Diff => AgentTimelineItemKind::Diff,
        AgentEventKind::Confirm => AgentTimelineItemKind::Confirmation,
        AgentEventKind::Error => AgentTimelineItemKind::Error,
        AgentEventKind::Status => {
            if event.operation == Some(AgentEventOperation::Fallback) {
                AgentTimelineItemKind::Fallback
            } else {
                AgentTimelineItemKind::Status
            }
        }
    }
}

fn stable_item_id(event: &AgentEvent, kind: &AgentTimelineItemKind) -> String {
    if let Some(confirmation_id) = &event.confirmation_id {
        return format!("confirmation:{confirmation_id}");
    }
    if let Some(tool_call_id) = &event.tool_call_id {
        return format!("tool:{tool_call_id}");
    }
    if let Some(message_id) = &event.message_id {
        return format!("{kind:?}:{message_id}");
    }
    event.id.clone()
}

fn is_session_level(event: &AgentEvent) -> bool {
    event.plan.is_some()
        || event.session_state.is_some()
        || matches!(
            event.operation,
            Some(AgentEventOperation::SessionState | AgentEventOperation::Fallback)
        )
        || (event.kind == AgentEventKind::Status && event.operation.is_none())
}

fn confirmation_status(value: Option<&str>) -> AgentConfirmationStatus {
    match value {
        Some("accepted") => AgentConfirmationStatus::Accepted,
        Some("rejected") => AgentConfirmationStatus::Rejected,
        _ => AgentConfirmationStatus::Pending,
    }
}

fn emit_timeline_patch(app: Option<&AppHandle>, patch: &AgentTimelinePatch) {
    if let Some(app) = app {
        let _ = app.emit("agent-timeline-changed", patch);
    }
}

#[tauri::command]
pub fn get_agent_timeline(
    runtime: tauri::State<'_, super::session::AcpRuntime>,
) -> Result<AgentTimelineSnapshot, String> {
    Ok(runtime.stream_runtime().snapshot())
}

#[tauri::command]
pub async fn respond_agent_stream_confirmation(
    app: AppHandle,
    runtime: tauri::State<'_, super::session::AcpRuntime>,
    confirmation_id: String,
    accepted: bool,
) -> Result<(), String> {
    runtime
        .respond_confirmation(confirmation_id.clone(), accepted)
        .await?;
    runtime
        .stream_runtime()
        .resolve_confirmation(Some(&app), &confirmation_id, accepted);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::events::{AgentEventKind, AgentToolPayload};

    fn event(kind: AgentEventKind, content: &str) -> AgentEvent {
        AgentEvent::new(kind, None, content, None).with_operation(AgentEventOperation::Append)
    }

    #[test]
    fn merges_message_and_thinking_chunks() {
        let mut reducer = AgentTimelineReducer::default();
        reducer.begin_turn("turn-1".to_string());
        reducer.apply_event(
            event(AgentEventKind::Result, "hel").with_message_id(Some("message-1".to_string())),
        );
        reducer.apply_event(
            event(AgentEventKind::Result, "lo").with_message_id(Some("message-1".to_string())),
        );
        reducer.apply_event(event(AgentEventKind::Thinking, "I am "));
        reducer.apply_event(event(AgentEventKind::Thinking, "thinking"));

        let snapshot = reducer.snapshot();
        assert_eq!(snapshot.items.len(), 2);
        assert_eq!(snapshot.items[0].content, "hello");
        assert_eq!(snapshot.items[1].content, "I am thinking");
        assert_eq!(snapshot.sequence, 5);
    }

    #[test]
    fn updates_tool_items_by_tool_call_id() {
        let mut reducer = AgentTimelineReducer::default();
        reducer.begin_turn("turn-1".to_string());
        let mut created =
            AgentEvent::new(AgentEventKind::Tool, Some("Run".into()), "pending", None)
                .with_tool_call_id(Some("tool-1".to_string()));
        created.tool = Some(AgentToolPayload {
            tool_call_id: "tool-1".to_string(),
            title: Some("Run".to_string()),
            kind: Some("execute".to_string()),
            status: Some("pending".to_string()),
            content: Vec::new(),
            locations: Vec::new(),
            raw_input: None,
            raw_output: None,
        });
        let mut updated = AgentEvent::new(AgentEventKind::Tool, None, "completed", None)
            .with_tool_call_id(Some("tool-1".to_string()));
        updated.tool = Some(AgentToolPayload {
            tool_call_id: "tool-1".to_string(),
            title: None,
            kind: None,
            status: Some("completed".to_string()),
            content: Vec::new(),
            locations: Vec::new(),
            raw_input: None,
            raw_output: None,
        });

        reducer.apply_event(created);
        reducer.apply_event(updated);

        let snapshot = reducer.snapshot();
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(
            snapshot.items[0].tool.as_ref().unwrap().title.as_deref(),
            Some("Run")
        );
        assert_eq!(
            snapshot.items[0].tool.as_ref().unwrap().status.as_deref(),
            Some("completed")
        );
    }

    #[test]
    fn replaces_plan_and_resolves_confirmation() {
        let mut reducer = AgentTimelineReducer::default();
        reducer.begin_turn("turn-1".to_string());
        let mut plan = AgentEvent::status("Plan updated");
        plan.plan = Some(AgentPlanSnapshot {
            entries: vec![crate::acp::events::AgentPlanEntry {
                content: "One".to_string(),
                priority: "high".to_string(),
                status: "pending".to_string(),
            }],
        });
        reducer.apply_event(plan);

        let mut confirmation = AgentEvent::new(
            AgentEventKind::Confirm,
            Some("Permission".into()),
            "Apply?",
            Some("confirm-1".into()),
        );
        confirmation.confirm_status = Some("pending".to_string());
        reducer.apply_event(confirmation);
        let patch = reducer.resolve_confirmation("confirm-1", true).unwrap();

        assert!(reducer.snapshot().plan.is_some());
        assert!(reducer.snapshot().pending_confirmations.is_empty());
        assert!(matches!(
            patch,
            AgentTimelinePatch::ResolveConfirmation {
                status: AgentConfirmationStatus::Accepted,
                ..
            }
        ));
    }

    #[test]
    fn ignores_late_turn_events_after_turn_finishes() {
        let mut reducer = AgentTimelineReducer::default();
        reducer.begin_turn("turn-1".to_string());
        reducer.apply_event(event(AgentEventKind::Result, "visible"));
        reducer.finish_turn("turn-1");
        reducer.apply_event(event(AgentEventKind::Result, "late"));

        let snapshot = reducer.snapshot();
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].content, "visible");
    }

    #[test]
    fn reset_and_begin_turn_advance_sequence() {
        let mut reducer = AgentTimelineReducer::default();
        let reset = reducer.reset(Some("session-1".to_string()));
        let begin = reducer.begin_turn("turn-1".to_string());

        assert!(matches!(
            reset,
            AgentTimelinePatch::Reset {
                snapshot: AgentTimelineSnapshot { sequence: 1, .. }
            }
        ));
        assert!(matches!(
            begin,
            AgentTimelinePatch::Reset {
                snapshot: AgentTimelineSnapshot { sequence: 2, .. }
            }
        ));
    }
}
