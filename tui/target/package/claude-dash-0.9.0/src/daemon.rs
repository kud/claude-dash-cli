use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

use crate::app::AppEvent;
use crate::types::{DaemonMessage, TuiDecision};

const SOCKET_PATH: &str = "/tmp/claude-dash-tui.sock";

pub enum DaemonCommand {
    SendDecision {
        connection_id: String,
        decision: String,
    },
}

pub async fn run(tx: mpsc::UnboundedSender<AppEvent>, mut cmd_rx: mpsc::UnboundedReceiver<DaemonCommand>) {
    loop {
        match UnixStream::connect(SOCKET_PATH).await {
            Ok(stream) => {
                let _ = tx.send(AppEvent::DaemonConnected);
                connect(stream, &tx, &mut cmd_rx).await;
                let _ = tx.send(AppEvent::DaemonDisconnected);
            }
            Err(_) => {
                let _ = tx.send(AppEvent::DaemonDisconnected);
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn connect(
    stream: UnixStream,
    tx: &mpsc::UnboundedSender<AppEvent>,
    cmd_rx: &mut mpsc::UnboundedReceiver<DaemonCommand>,
) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();

    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if let Ok(msg) = serde_json::from_str::<DaemonMessage>(&line) {
                            let _ = tx.send(AppEvent::DaemonMessage(msg));
                        }
                    }
                    _ => return,
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(DaemonCommand::SendDecision { connection_id, decision }) => {
                        let msg = TuiDecision {
                            msg_type: "PermissionDecision",
                            connection_id,
                            decision,
                        };
                        let line = serde_json::to_string(&msg).unwrap_or_default() + "\n";
                        if writer.write_all(line.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                    None => return,
                }
            }
        }
    }
}
