use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyCode};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::runtime::events::EventKind;
use crate::runtime::state::TeamState;
use crate::runtime::watchdog::Watchdog;
use crate::vis::event_stream::EventStream;
use crate::vis::hud::HudState;

mod guard;
mod render;

use guard::RawModeGuard;

pub struct HudTui {
    hud_state: HudState,
    event_stream: EventStream,
    watchdog: Watchdog,
    state_dir: PathBuf,
    team_name: String,
    team_state: Option<TeamState>,
}

impl HudTui {
    pub fn new(team_name: &str, state_dir: PathBuf) -> Self {
        let events_path = state_dir.join("events.jsonl");
        let event_stream = EventStream::new(&events_path);
        let watchdog = Watchdog::new(crate::runtime::watchdog::WatchdogConfig {
            ..Default::default()
        });
        let run_id = team_name.to_string();
        let hud_state = HudState::new(team_name, &run_id);
        Self {
            hud_state,
            event_stream,
            watchdog,
            state_dir,
            team_name: team_name.to_string(),
            team_state: None,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Guard restores raw mode / alt screen / mouse capture on Drop, so a
        // panic or any `?` short-circuit inside run_loop no longer leaves the
        // user's terminal corrupted.
        let _guard = RawModeGuard::enter()?;
        let backend = CrosstermBackend::new(std::io::stdout());
        let mut terminal = Terminal::new(backend)?;
        self.run_loop(&mut terminal).await
    }

    async fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        let mut last_tick = std::time::Instant::now();
        let tick_rate = std::time::Duration::from_millis(1000);

        // Initial refresh
        self.hud_state
            .refresh(&mut self.event_stream, &self.watchdog, &self.state_dir)
            .await?;
        self.team_state = TeamState::load(&self.state_dir).await.ok();

        loop {
            terminal.draw(|f| self.draw(f))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| std::time::Duration::from_secs(0));

            if event::poll(timeout)? {
                if let CrosstermEvent::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('r') => {
                            self.hud_state
                                .refresh(&mut self.event_stream, &self.watchdog, &self.state_dir)
                                .await?;
                            self.team_state = TeamState::load(&self.state_dir).await.ok();
                        }
                        _ => {}
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.hud_state
                    .refresh(&mut self.event_stream, &self.watchdog, &self.state_dir)
                    .await?;
                self.team_state = TeamState::load(&self.state_dir).await.ok();
                last_tick = std::time::Instant::now();
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn worker_task_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for event in &self.hud_state.events {
            if let Some(ref payload) = event.payload {
                let worker_id = match event.actor.clone() {
                    Some(id) => id,
                    None => continue,
                };

                if let Some(task_id) = payload.get("task_id").and_then(|v| v.as_str()) {
                    match event.kind {
                        EventKind::TaskClaimed | EventKind::TaskStarted => {
                            map.insert(worker_id, task_id.to_string());
                        }
                        EventKind::TaskCompleted | EventKind::TaskFailed => {
                            map.remove(&worker_id);
                        }
                        EventKind::WorkerHeartbeat => {
                            map.entry(worker_id).or_insert_with(|| task_id.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn hud_tui_draw_does_not_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let hud = HudTui::new("test-team", tmp.path().to_path_buf());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| hud.draw(f)).unwrap();
    }

    #[test]
    fn hud_tui_worker_colors() {
        let tmp = tempfile::tempdir().unwrap();
        let mut hud = HudTui::new("test-team", tmp.path().to_path_buf());

        use crate::runtime::watchdog::{HealthStatus, WorkerHealth};
        use chrono::Utc;

        hud.hud_state.workers = vec![
            WorkerHealth {
                worker_id: "w1".to_string(),
                status: HealthStatus::Healthy,
                last_heartbeat: Some(Utc::now()),
                heartbeat_content: None,
                inbox_count: 0,
                outbox_count: 0,
                message: "ok".to_string(),
            },
            WorkerHealth {
                worker_id: "w2".to_string(),
                status: HealthStatus::Stalled,
                last_heartbeat: Some(Utc::now()),
                heartbeat_content: None,
                inbox_count: 0,
                outbox_count: 0,
                message: "stalled".to_string(),
            },
            WorkerHealth {
                worker_id: "w3".to_string(),
                status: HealthStatus::Dead,
                last_heartbeat: None,
                heartbeat_content: None,
                inbox_count: 0,
                outbox_count: 0,
                message: "dead".to_string(),
            },
        ];

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| hud.draw(f)).unwrap();
        // If we got here without panic, color mapping is valid.
    }
}
