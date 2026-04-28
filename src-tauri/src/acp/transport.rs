use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

use super::profile::AgentProfile;

pub struct JsonRpcTransport {
    child: Child,
    stdin: ChildStdin,
}

impl JsonRpcTransport {
    pub fn spawn(profile: &AgentProfile) -> Result<(Self, mpsc::UnboundedReceiver<String>), String> {
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

        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to open ACP child stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to open ACP child stdout".to_string())?;

        let (tx, rx) = mpsc::unbounded_channel();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        Ok((Self { child, stdin }, rx))
    }

    pub async fn write_json<T: serde::Serialize>(&mut self, value: &T) -> Result<(), String> {
        let mut line = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        line.push(b'\n');
        self.stdin.write_all(&line).await.map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())
    }

    pub async fn shutdown(&mut self) {
        let _ = self.stdin.shutdown().await;
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                let _ = self.child.kill().await;
                let _ = self.child.wait().await;
            }
            Err(_) => {
                let _ = self.child.kill().await;
            }
        }
    }
}
