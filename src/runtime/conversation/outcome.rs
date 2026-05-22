use std::time::Instant;

use crate::runtime::classifier::Intent;

#[derive(Debug, Clone)]
pub enum RouteOutcome {
    Trivial {
        latency_ms: u32,
    },
    Small {
        worker_id: String,
        files_touched: u32,
        diff_summary: String,
    },
    Medium {
        plan: Vec<String>,
        started_at: Instant,
    },
    Large {
        goal_id: String,
        plan: Vec<String>,
    },
    Cancelled,
    Refused {
        reason: String,
    },
    Downgraded {
        from: Intent,
        to: Intent,
        outcome: Box<RouteOutcome>,
    },
    Queued {
        intent: Intent,
        position: usize,
    },
}
