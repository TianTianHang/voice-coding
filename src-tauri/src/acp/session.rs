use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex as AsyncMutex;

use super::events::{AgentEvent, AgentEventKind, AgentStatus};
use super::json_rpc::{notification, parse_incoming, request, IncomingMessage, RequestIds};
use super::profile::AgentProfile;
use super::transport::JsonRpcTransport;

struct ActiveSession {
    profile: AgentProfile,
    session_id: String,
    transport: Arc<AsyncMutex<JsonRpcTransport>>,
}

#[derive(Default)]
pub struct AcpRuntime {
    active: Mutex<Option<ActiveSession>>,
    request_ids: RequestIds,
    confirmations: Mutex<HashMap<String, String>>,
}

impl AcpRuntime {
    pub fn status(&self) -> AgentStatus {
        self.active
            .lock()
            .as_ref()
            .map(|active| AgentStatus {
                connected: true,
                profile_name: Some(active.profile.name.clone()),
                session_id: Some(active.session_id.clone()),
                error: None,
            })
            .unwrap_or_else(AgentStatus::disconnected)
    }

    pub async fn connect_from_environment(&self, app: AppHandle) -> Result<AgentStatus, String> {
        let profile = AgentProfile::from_environment()?;
        self.connect(app, profile).await
    }

    pub async fn connect(
        &self,
        app: AppHandle,
        profile: AgentProfile,
    ) -> Result<AgentStatus, String> {
        if self.active.lock().is_some() {
            return Err("An ACP agent is already connected".into());
        }

        emit_agent_event(&app, AgentEvent::status("Connecting ACP agent"));
        let (transport, mut rx) = JsonRpcTransport::spawn(&profile)?;
        let transport = Arc::new(AsyncMutex::new(transport));
        let session_id = uuid::Uuid::new_v4().to_string();

        {
            let mut writer = transport.lock().await;
            writer
                .write_json(&request(
                    self.request_ids.next(),
                    "initialize",
                    Some(json!({
                        "clientInfo": {
                            "name": "voice-coding",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    })),
                ))
                .await?;
            writer
                .write_json(&request(
                    self.request_ids.next(),
                    "session/new",
                    Some(json!({ "sessionId": session_id })),
                ))
                .await?;
        }

        {
            let mut active = self.active.lock();
            *active = Some(ActiveSession {
                profile,
                session_id,
                transport: transport.clone(),
            });
        }

        let app_for_reader = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(line) = rx.recv().await {
                match normalize_line(&line) {
                    Ok(event) => emit_agent_event(&app_for_reader, event),
                    Err(err) => emit_agent_event(&app_for_reader, AgentEvent::error(err)),
                }
            }
            emit_agent_status(
                &app_for_reader,
                AgentStatus {
                    connected: false,
                    profile_name: None,
                    session_id: None,
                    error: Some("ACP agent output stream closed".into()),
                },
            );
        });

        let status = self.status();
        emit_agent_status(&app, status.clone());
        emit_agent_event(&app, AgentEvent::status("ACP agent connected"));
        Ok(status)
    }

    pub async fn disconnect(&self, app: AppHandle) -> Result<AgentStatus, String> {
        let active = self.active.lock().take();
        if let Some(active) = active {
            active.transport.lock().await.shutdown().await;
        }

        let status = AgentStatus::disconnected();
        emit_agent_status(&app, status.clone());
        emit_agent_event(&app, AgentEvent::status("ACP agent disconnected"));
        Ok(status)
    }

    pub async fn send_prompt(&self, app: AppHandle, prompt: String) -> Result<(), String> {
        let (session_id, transport) = {
            let active = self.active.lock();
            let active = active
                .as_ref()
                .ok_or_else(|| "No active ACP agent session".to_string())?;
            (active.session_id.clone(), active.transport.clone())
        };

        transport
            .lock()
            .await
            .write_json(&request(
                self.request_ids.next(),
                "session/prompt",
                Some(json!({
                    "sessionId": session_id,
                    "prompt": prompt,
                })),
            ))
            .await?;

        emit_agent_event(&app, AgentEvent::status("Sent current sentence to agent"));
        Ok(())
    }

    pub async fn respond_confirmation(
        &self,
        confirmation_id: String,
        accepted: bool,
    ) -> Result<(), String> {
        let transport = {
            let active = self.active.lock();
            let active = active
                .as_ref()
                .ok_or_else(|| "No active ACP agent session".to_string())?;
            active.transport.clone()
        };
        let method = if accepted {
            "confirmation/accept"
        } else {
            "confirmation/reject"
        };
        self.confirmations
            .lock()
            .insert(confirmation_id.clone(), method.into());

        let result = transport
            .lock()
            .await
            .write_json(&notification(
                method,
                Some(json!({ "confirmationId": confirmation_id })),
            ))
            .await;
        result
    }
}

pub fn emit_agent_event(app: &AppHandle, event: AgentEvent) {
    let _ = app.emit("agent-event", event);
}

pub fn emit_agent_status(app: &AppHandle, status: AgentStatus) {
    let _ = app.emit("agent-status", status);
}

fn normalize_line(line: &str) -> Result<AgentEvent, String> {
    match parse_incoming(line)? {
        IncomingMessage::Response(response) => {
            if let Some(error) = response.error {
                return Ok(AgentEvent::error(error.message));
            }
            Ok(AgentEvent::new(
                AgentEventKind::Result,
                Some("Response".into()),
                stringify_value(response.result.unwrap_or(Value::Null)),
                None,
            ))
        }
        IncomingMessage::Notification(notification) => {
            let params = notification.params.unwrap_or(Value::Null);
            let content = content_from_params(&params);
            let method = notification.method.to_ascii_lowercase();
            let kind = kind_from_method(&method);
            let confirmation_id = if kind == AgentEventKind::Confirm {
                params
                    .get("confirmationId")
                    .or_else(|| params.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| Some(uuid::Uuid::new_v4().to_string()))
            } else {
                None
            };
            let mut event = AgentEvent::new(
                kind,
                Some(notification.method),
                content,
                confirmation_id,
            );
            if event.kind == AgentEventKind::Confirm {
                event.confirm_status = Some("pending".into());
            }
            Ok(event)
        }
    }
}

fn kind_from_method(method: &str) -> AgentEventKind {
    if method.contains("confirm") || method.contains("permission") {
        AgentEventKind::Confirm
    } else if method.contains("tool") || method.contains("call") {
        AgentEventKind::Tool
    } else if method.contains("diff") || method.contains("patch") {
        AgentEventKind::Diff
    } else if method.contains("thinking") || method.contains("reason") {
        AgentEventKind::Thinking
    } else if method.contains("error") {
        AgentEventKind::Error
    } else if method.contains("status") || method.contains("progress") {
        AgentEventKind::Status
    } else {
        AgentEventKind::Result
    }
}

fn content_from_params(params: &Value) -> String {
    params
        .get("text")
        .or_else(|| params.get("content"))
        .or_else(|| params.get("message"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| stringify_value(params.clone()))
}

fn stringify_value(value: Value) -> String {
    match value {
        Value::String(text) => text,
        other => serde_json::to_string_pretty(&other).unwrap_or_else(|_| String::new()),
    }
}

#[tauri::command]
pub async fn connect_agent(
    app: AppHandle,
    runtime: tauri::State<'_, AcpRuntime>,
) -> Result<AgentStatus, String> {
    runtime.connect_from_environment(app).await
}

#[tauri::command]
pub async fn disconnect_agent(
    app: AppHandle,
    runtime: tauri::State<'_, AcpRuntime>,
) -> Result<AgentStatus, String> {
    runtime.disconnect(app).await
}

#[tauri::command]
pub fn get_agent_status(runtime: tauri::State<'_, AcpRuntime>) -> Result<AgentStatus, String> {
    Ok(runtime.status())
}

#[tauri::command]
pub async fn send_agent_prompt(
    app: AppHandle,
    runtime: tauri::State<'_, AcpRuntime>,
    prompt: String,
) -> Result<(), String> {
    runtime.send_prompt(app, prompt).await
}

#[tauri::command]
pub async fn respond_agent_confirmation(
    runtime: tauri::State<'_, AcpRuntime>,
    confirmation_id: String,
    accepted: bool,
) -> Result<(), String> {
    runtime.respond_confirmation(confirmation_id, accepted).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_tool_notification() {
        let event = normalize_line(
            r#"{"jsonrpc":"2.0","method":"tool/call","params":{"text":"run tests"}}"#,
        )
        .unwrap();

        assert_eq!(event.kind, AgentEventKind::Tool);
        assert_eq!(event.content, "run tests");
    }

    #[test]
    fn normalizes_confirmation_notification() {
        let event = normalize_line(
            r#"{"jsonrpc":"2.0","method":"confirm","params":{"confirmationId":"c1","message":"apply?"}}"#,
        )
        .unwrap();

        assert_eq!(event.kind, AgentEventKind::Confirm);
        assert_eq!(event.confirmation_id.as_deref(), Some("c1"));
        assert_eq!(event.confirm_status.as_deref(), Some("pending"));
    }
}
