use agent_client_protocol::schema::{
    CreateTerminalRequest, InitializeRequest, KillTerminalRequest, ProtocolVersion,
    ReadTextFileRequest, ReleaseTerminalRequest, RequestPermissionRequest, SessionNotification,
    TerminalOutputRequest, WaitForTerminalExitRequest, WriteTextFileRequest,
};
use agent_client_protocol::{Agent, Client, ConnectionTo, JsonRpcResponse, Responder};
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::process::Child;
use tokio::sync::{Mutex as AsyncMutex, mpsc, oneshot};

use super::client::{
    AgentResultTracker, PendingPermissions, VoiceCodingAcpClient, respond_pending_permission,
};
use super::events::{AgentEvent, AgentStatus};
use super::profile::AgentProfile;
use super::transport::spawn_sdk_transport;

struct PromptCommand {
    prompt: String,
    result: oneshot::Sender<Result<(), String>>,
}

type ReadySender = oneshot::Sender<Result<(String, mpsc::UnboundedSender<PromptCommand>), String>>;

struct ActiveAgent {
    profile: AgentProfile,
    session_id: String,
    prompt_tx: mpsc::UnboundedSender<PromptCommand>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    child: Arc<AsyncMutex<Child>>,
    task: tauri::async_runtime::JoinHandle<()>,
}

#[derive(Default)]
pub struct AcpRuntime {
    active: Arc<Mutex<Option<ActiveAgent>>>,
    pending_permissions: PendingPermissions,
    result_tracker: AgentResultTracker,
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
        let (transport, child) = spawn_sdk_transport(&profile)?;
        let child = Arc::new(AsyncMutex::new(child));
        let (ready_tx, ready_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let active = self.active.clone();
        let pending_permissions = self.pending_permissions.clone();
        let result_tracker = self.result_tracker.clone();
        let profile_for_task = profile.clone();
        let app_for_task = app.clone();
        let child_for_task = child.clone();

        let task = tauri::async_runtime::spawn(async move {
            let result = run_sdk_connection(
                app_for_task.clone(),
                profile_for_task.clone(),
                transport,
                pending_permissions,
                result_tracker,
                ready_tx,
                shutdown_rx,
            )
            .await;

            if let Err(err) = result {
                emit_agent_event(&app_for_task, AgentEvent::error(err.clone()));
                emit_agent_status(
                    &app_for_task,
                    AgentStatus {
                        connected: false,
                        profile_name: None,
                        session_id: None,
                        error: Some(err),
                    },
                );
            }

            let _ = child_for_task.lock().await.kill().await;
            *active.lock() = None;
        });

        let (session_id, prompt_tx) = ready_rx
            .await
            .map_err(|_| "ACP agent connection closed before session was ready".to_string())??;

        {
            let mut active = self.active.lock();
            *active = Some(ActiveAgent {
                profile,
                session_id,
                prompt_tx,
                shutdown_tx: Some(shutdown_tx),
                child,
                task,
            });
        }

        let status = self.status();
        emit_agent_status(&app, status.clone());
        emit_agent_event(&app, AgentEvent::status("ACP agent connected"));
        Ok(status)
    }

    pub async fn disconnect(&self, app: AppHandle) -> Result<AgentStatus, String> {
        let active = self.active.lock().take();
        if let Some(mut active) = active {
            if let Some(shutdown_tx) = active.shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
            let _ = active.child.lock().await.kill().await;
            active.task.abort();
        }
        self.pending_permissions.lock().clear();

        let status = AgentStatus::disconnected();
        emit_agent_status(&app, status.clone());
        emit_agent_event(&app, AgentEvent::status("ACP agent disconnected"));
        Ok(status)
    }

    pub async fn send_prompt(&self, app: AppHandle, prompt: String) -> Result<(), String> {
        let prompt_tx = {
            let active = self.active.lock();
            active
                .as_ref()
                .ok_or_else(|| "No active ACP agent session".to_string())?
                .prompt_tx
                .clone()
        };
        let (result_tx, result_rx) = oneshot::channel();
        prompt_tx
            .send(PromptCommand {
                prompt,
                result: result_tx,
            })
            .map_err(|_| "ACP agent prompt worker is not running".to_string())?;
        result_rx
            .await
            .map_err(|_| "ACP agent prompt worker stopped before responding".to_string())??;

        emit_agent_event(&app, AgentEvent::status("Sent current sentence to agent"));
        Ok(())
    }

    pub async fn respond_confirmation(
        &self,
        confirmation_id: String,
        accepted: bool,
    ) -> Result<(), String> {
        respond_pending_permission(&self.pending_permissions, &confirmation_id, accepted)
    }
}

async fn run_sdk_connection(
    app: AppHandle,
    profile: AgentProfile,
    transport: super::transport::SdkTransport,
    pending_permissions: PendingPermissions,
    result_tracker: AgentResultTracker,
    ready_tx: ReadySender,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let client =
        VoiceCodingAcpClient::new(app.clone(), pending_permissions, result_tracker.clone());
    let notification_client = client.clone();
    let permission_client = client.clone();
    let unsupported_app = app.clone();
    let ready_tx = Arc::new(Mutex::new(Some(ready_tx)));

    let result = Client
        .builder()
        .name("voice-coding")
        .on_receive_notification(
            async move |notification: SessionNotification, _cx| {
                notification_client.handle_notification(notification).await;
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request: RequestPermissionRequest, responder, _cx| {
                permission_client
                    .handle_permission_request(request, responder)
                    .map_err(agent_client_protocol::util::internal_error)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let app = unsupported_app.clone();
                async move |_request: WriteTextFileRequest, responder, _cx| {
                    reject_unsupported(&app, responder, "fs.writeTextFile")
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let app = unsupported_app.clone();
                async move |_request: ReadTextFileRequest, responder, _cx| {
                    reject_unsupported(&app, responder, "fs.readTextFile")
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let app = unsupported_app.clone();
                async move |_request: CreateTerminalRequest, responder, _cx| {
                    reject_unsupported(&app, responder, "terminal.create")
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let app = unsupported_app.clone();
                async move |_request: TerminalOutputRequest, responder, _cx| {
                    reject_unsupported(&app, responder, "terminal.output")
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let app = unsupported_app.clone();
                async move |_request: ReleaseTerminalRequest, responder, _cx| {
                    reject_unsupported(&app, responder, "terminal.release")
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let app = unsupported_app.clone();
                async move |_request: KillTerminalRequest, responder, _cx| {
                    reject_unsupported(&app, responder, "terminal.kill")
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let app = unsupported_app.clone();
                async move |_request: WaitForTerminalExitRequest, responder, _cx| {
                    reject_unsupported(&app, responder, "terminal.waitForExit")
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(transport, {
            let ready_tx = ready_tx.clone();
            async move |connection: ConnectionTo<Agent>| {
                connection
                    .send_request(InitializeRequest::new(ProtocolVersion::V1))
                    .block_task()
                    .await?;

                let cwd = profile.cwd.clone().unwrap_or(
                    std::env::current_dir().map_err(agent_client_protocol::util::internal_error)?,
                );
                let session = connection
                    .build_session(cwd)
                    .block_task()
                    .start_session()
                    .await?;
                let session_id = session.session_id().to_string();
                let (prompt_tx, prompt_rx) = mpsc::unbounded_channel();

                if let Some(tx) = ready_tx.lock().take() {
                    let _ = tx.send(Ok((session_id, prompt_tx)));
                }

                run_prompt_worker(app, result_tracker, session, prompt_rx, shutdown_rx).await
            }
        })
        .await;

    if let Err(err) = result {
        if let Some(tx) = ready_tx.lock().take() {
            let _ = tx.send(Err(err.to_string()));
        }
        return Err(err.to_string());
    }

    Ok(())
}

fn reject_unsupported<T: JsonRpcResponse>(
    app: &AppHandle,
    responder: Responder<T>,
    capability: &str,
) -> Result<(), agent_client_protocol::Error> {
    let message = format!("ACP client capability '{capability}' is not supported");
    emit_agent_event(app, AgentEvent::error(message.clone()));
    responder.respond_with_internal_error(message)
}

async fn run_prompt_worker(
    app: AppHandle,
    result_tracker: AgentResultTracker,
    mut session: agent_client_protocol::ActiveSession<'static, Agent>,
    mut prompt_rx: mpsc::UnboundedReceiver<PromptCommand>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), agent_client_protocol::Error> {
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => return Ok(()),
            command = prompt_rx.recv() => {
                let Some(command) = command else {
                    return Ok(());
                };
                let result = send_prompt_and_wait(&app, &result_tracker, &mut session, command.prompt).await;
                let _ = command.result.send(result.map_err(|e| e.to_string()));
            }
        }
    }
}

async fn send_prompt_and_wait(
    app: &AppHandle,
    result_tracker: &AgentResultTracker,
    session: &mut agent_client_protocol::ActiveSession<'static, Agent>,
    prompt: String,
) -> Result<(), agent_client_protocol::Error> {
    session.send_prompt(prompt)?;
    loop {
        match session.read_update().await? {
            agent_client_protocol::SessionMessage::StopReason(_) => {
                trigger_auto_tts_for_latest_result(app.clone(), result_tracker.latest());
                return Ok(());
            }
            agent_client_protocol::SessionMessage::SessionMessage(_) => {}
            _ => {}
        }
    }
}

fn trigger_auto_tts_for_latest_result(
    app: AppHandle,
    result: Option<super::client::TrackedAgentResult>,
) {
    let Some(result) = result else {
        return;
    };
    if result.content.trim().is_empty() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let runtime = app.state::<crate::tts::TtsRuntime>();
        let _ = runtime
            .speak_agent_result(app.clone(), Some(result.id), result.content)
            .await;
    });
}

pub fn emit_agent_event(app: &AppHandle, event: AgentEvent) {
    let _ = app.emit("agent-event", event);
}

pub fn emit_agent_status(app: &AppHandle, status: AgentStatus) {
    let _ = app.emit("agent-status", status);
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
    runtime
        .respond_confirmation(confirmation_id, accepted)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_status_is_disconnected() {
        assert_eq!(AcpRuntime::default().status(), AgentStatus::disconnected());
    }

    #[tokio::test]
    async fn rejects_unknown_confirmation_id() {
        let runtime = AcpRuntime::default();
        let err = runtime
            .respond_confirmation("missing".into(), true)
            .await
            .unwrap_err();

        assert!(err.contains("Unknown ACP confirmation id"));
    }
}
