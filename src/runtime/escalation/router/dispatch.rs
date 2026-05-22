use std::sync::Arc;
use std::time::Instant;

use crate::runtime::classifier::{ClassifierOutput, Intent};
use crate::runtime::conversation::{
    bus::{ActiveMode, BusEvent, PreflightKind},
    disclosure::format_disclosure,
    outcome::RouteOutcome,
    session::SessionCtx,
};
use crate::runtime::escalation::{
    backends::{MediumPlanResult, SmallEditResult},
    planner::plan_medium,
};

use super::Router;

impl Router {
    pub(super) async fn dispatch_from_output(
        &self,
        prompt: &str,
        output: &ClassifierOutput,
        session: &Arc<SessionCtx>,
    ) -> RouteOutcome {
        if let Some(hard) = self.config.cost_cap_usd_hard {
            let cost = *session.cumulative_cost_usd.lock().await;
            if cost >= hard && output.intent != Intent::Trivial {
                let reason = format!(
                    "→ refused: hard cost cap exceeded (${:.2}). /cost for details, raise cap in .omk/config.toml",
                    hard
                );
                self.event_bus.publish(BusEvent::Refused {
                    reason: reason.clone(),
                    intent: output.intent,
                });
                return RouteOutcome::Refused { reason };
            }
        }

        let preflight_kind = self.preflight_kind(output, prompt, session).await;
        if let Some(kind) = preflight_kind {
            if self.config.interactive_preflight {
                let action = self.run_preflight(kind, output).await;
                return self
                    .handle_preflight_action(prompt, output, action, session)
                    .await;
            }

            self.event_bus.publish(BusEvent::AutonomousProceed {
                kind,
                intent: output.intent,
                confidence: output.confidence,
                reasoning: output.reasoning.clone(),
            });

            let queued = matches!(
                kind,
                PreflightKind::QueueLargeOnActiveLarge | PreflightKind::QueueMediumAtConcurrencyCap
            );
            if queued {
                return RouteOutcome::Queued {
                    intent: output.intent,
                    position: session.active_medium_goals.lock().await.len(),
                };
            }

            return self.dispatch_direct(prompt, output, session).await;
        }

        self.dispatch_direct(prompt, output, session).await
    }

    pub(super) async fn dispatch_direct(
        &self,
        prompt: &str,
        output: &ClassifierOutput,
        session: &Arc<SessionCtx>,
    ) -> RouteOutcome {
        match output.intent {
            Intent::Trivial => self.dispatch_trivial(prompt, session).await,
            Intent::Small => self.dispatch_small(prompt, session).await,
            Intent::Medium => self.dispatch_medium(prompt, session).await,
            Intent::Large => self.dispatch_large(prompt, output, session).await,
        }
    }

    async fn dispatch_trivial(&self, prompt: &str, _session: &Arc<SessionCtx>) -> RouteOutcome {
        let start = Instant::now();
        match self.llm_direct.answer_direct(prompt, &[]).await {
            Ok(_) => {
                let latency_ms = start.elapsed().as_millis() as u32;
                RouteOutcome::Trivial { latency_ms }
            }
            Err(e) => RouteOutcome::Refused {
                reason: format!("trivial dispatch failed: {}", e),
            },
        }
    }

    async fn dispatch_small(&self, prompt: &str, session: &Arc<SessionCtx>) -> RouteOutcome {
        self.event_bus.publish(BusEvent::RouterEscalating {
            intent: Intent::Small,
            target_mode: ActiveMode::WireWorker,
            preflight: false,
        });
        let task_summary = Some(prompt);
        if let Some(line) = format_disclosure(Intent::Small, ActiveMode::WireWorker, task_summary) {
            self.event_bus.publish(BusEvent::DisclosureLine(line));
        }

        let worker_id = format!("small-{}", uuid::Uuid::new_v4());
        self.event_bus.publish(BusEvent::WorkerStarted {
            worker_id: worker_id.clone(),
            kind: "small".to_string(),
            task: prompt.to_string(),
        });

        session
            .active_small_workers
            .lock()
            .await
            .push(worker_id.clone());

        let result = self.wire_worker.run_small_edit(prompt).await;

        session
            .active_small_workers
            .lock()
            .await
            .retain(|x| x != &worker_id);

        match result {
            Ok(SmallEditResult {
                files_touched,
                diff_summary,
                ..
            }) => {
                self.event_bus.publish(BusEvent::WorkerCompleted {
                    worker_id: worker_id.clone(),
                    files_touched,
                    ok: true,
                });
                RouteOutcome::Small {
                    worker_id,
                    files_touched,
                    diff_summary,
                }
            }
            Err(e) => {
                self.event_bus.publish(BusEvent::WorkerCompleted {
                    worker_id: worker_id.clone(),
                    files_touched: 0,
                    ok: false,
                });
                RouteOutcome::Refused {
                    reason: format!("small dispatch failed: {}", e),
                }
            }
        }
    }

    async fn dispatch_medium(&self, prompt: &str, session: &Arc<SessionCtx>) -> RouteOutcome {
        let plan = match plan_medium(prompt) {
            Ok(p) => p,
            Err(e) => {
                return RouteOutcome::Refused {
                    reason: format!("planning failed: {}", e),
                }
            }
        };

        let task_summary = if plan.len() == 1 {
            plan[0].clone()
        } else {
            format!("{}-step plan, sequential workers", plan.len())
        };

        self.event_bus.publish(BusEvent::RouterEscalating {
            intent: Intent::Medium,
            target_mode: ActiveMode::PlannerWorkers,
            preflight: false,
        });
        if let Some(line) = format_disclosure(
            Intent::Medium,
            ActiveMode::PlannerWorkers,
            Some(&task_summary),
        ) {
            self.event_bus.publish(BusEvent::DisclosureLine(line));
        }

        let plan_id = format!("plan-{}", uuid::Uuid::new_v4());
        session
            .active_medium_goals
            .lock()
            .await
            .push(plan_id.clone());

        self.event_bus.publish(BusEvent::WorkerStarted {
            worker_id: plan_id.clone(),
            kind: "medium".to_string(),
            task: prompt.to_string(),
        });

        let started_at = Instant::now();
        let result = self.wire_worker.run_medium_plan(&plan).await;

        session
            .active_medium_goals
            .lock()
            .await
            .retain(|x| x != &plan_id);

        match result {
            Ok(MediumPlanResult {
                workers,
                steps_completed: _,
                steps_failed,
            }) => {
                for w in &workers {
                    self.event_bus.publish(BusEvent::WorkerStarted {
                        worker_id: w.clone(),
                        kind: "medium".to_string(),
                        task: prompt.to_string(),
                    });
                    self.event_bus.publish(BusEvent::WorkerCompleted {
                        worker_id: w.clone(),
                        files_touched: 0,
                        ok: steps_failed == 0,
                    });
                }

                self.event_bus.publish(BusEvent::WorkerCompleted {
                    worker_id: plan_id,
                    files_touched: 0,
                    ok: steps_failed == 0,
                });

                RouteOutcome::Medium { plan, started_at }
            }
            Err(e) => RouteOutcome::Refused {
                reason: format!("medium dispatch failed: {}", e),
            },
        }
    }

    async fn dispatch_large(
        &self,
        prompt: &str,
        output: &ClassifierOutput,
        session: &Arc<SessionCtx>,
    ) -> RouteOutcome {
        self.event_bus.publish(BusEvent::RouterEscalating {
            intent: Intent::Large,
            target_mode: ActiveMode::GoalRun,
            preflight: false,
        });
        if let Some(line) = format_disclosure(Intent::Large, ActiveMode::GoalRun, None) {
            self.event_bus.publish(BusEvent::DisclosureLine(line));
        }

        let plan = vec![output.reasoning.clone()];

        let req = crate::runtime::goal::chat_api::CreateChildRequest {
            session_id: session.session_id.clone(),
            parent_conv_id: session.session_id.clone(),
            prompt: prompt.to_string(),
            config: crate::runtime::goal::chat_api::ChildGoalConfig {
                merge_policy: crate::runtime::goal::GoalMergePolicy::Disabled,
                enforce_protection: false,
                wire_pool_size: self.config.wire_pool_size,
                max_budget_usd: self.config.cost_cap_usd_hard,
            },
        };

        match self.goal_bridge.create_child(req).await {
            Ok(handle) => {
                session
                    .active_large_goal
                    .lock()
                    .await
                    .replace(handle.goal_id.clone());
                self.event_bus.publish(BusEvent::ChildGoalCreated {
                    goal_id: handle.goal_id.clone(),
                    parent_conv_id: session.session_id.clone(),
                    plan: plan.clone(),
                });
                RouteOutcome::Large {
                    goal_id: handle.goal_id,
                    plan,
                }
            }
            Err(e) => RouteOutcome::Refused {
                reason: format!("large dispatch failed: {}", e),
            },
        }
    }
}
