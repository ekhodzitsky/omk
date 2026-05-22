use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::adapter::to_child_event;
use super::events::ChildGoalEvent;
use crate::runtime::events::Event;

pub async fn tail_goal_events_into(
    goal_state_dir: PathBuf,
    sender: broadcast::Sender<ChildGoalEvent>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let events_path = goal_state_dir.join(crate::runtime::config::EVENTS_FILE);

    // Poll for file appearance with 30s timeout
    let file_appears = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while !events_path.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await;

    if file_appears.is_err() {
        anyhow::bail!("events.jsonl did not appear within 30s");
    }

    let mut file_pos = 0u64;

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                match File::open(&events_path).await {
                    Ok(file) => {
                        let mut reader = BufReader::new(file);
                        if reader.seek(SeekFrom::Start(file_pos)).await.is_err() {
                            continue;
                        }
                        loop {
                            let mut line = String::new();
                            match reader.read_line(&mut line).await {
                                Ok(0) => break,
                                Ok(_) => {
                                    file_pos += line.len() as u64;
                                    if let Ok(event) = serde_json::from_str::<Event>(&line) {
                                        if let Some(child_event) = to_child_event(&event) {
                                            let _ = sender.send(child_event);
                                        }
                                    }
                                }
                                Err(_) => break,
                            }
                            if shutdown.is_cancelled() {
                                break;
                            }
                        }
                    }
                    Err(_) => {
                        // File may be temporarily unavailable; retry next cycle
                    }
                }
            }
        }
    }

    Ok(())
}
