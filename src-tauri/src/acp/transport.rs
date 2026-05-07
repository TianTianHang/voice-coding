use std::process::Stdio;

use agent_client_protocol::ByteStreams;
use tokio::process::{Child, Command};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use super::profile::AgentProfile;

pub type SdkTransport = ByteStreams<
    tokio_util::compat::Compat<tokio::process::ChildStdin>,
    tokio_util::compat::Compat<tokio::process::ChildStdout>,
>;

pub fn spawn_sdk_transport(profile: &AgentProfile) -> Result<(SdkTransport, Child), String> {
    log::info!(
        "spawning ACP agent: profile={} command={} args={:?} cwd={:?}",
        profile.name,
        profile.command,
        profile.args,
        profile.cwd
    );
    let mut command = Command::new(&profile.command);
    command
        .args(&profile.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(cwd) = &profile.cwd {
        command.current_dir(cwd);
    }

    for (key, value) in &profile.env {
        command.env(key, value);
    }

    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start ACP agent '{}': {e}", profile.command))?;
    log::info!("ACP agent process spawned");
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open ACP child stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to open ACP child stdout".to_string())?;

    Ok((
        ByteStreams::new(stdin.compat_write(), stdout.compat()),
        child,
    ))
}
