use std::collections::HashMap;

use tokio::sync::{oneshot, Mutex};

use crate::runtime::conversation::bus::PreflightAction;

pub type TicketId = String;

#[derive(Debug)]
pub struct PreflightInbox {
    pending: Mutex<HashMap<TicketId, oneshot::Sender<PreflightAction>>>,
}

pub struct PreflightTicket {
    pub id: TicketId,
    pub rx: oneshot::Receiver<PreflightAction>,
}

impl std::fmt::Debug for PreflightTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreflightTicket")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl PreflightInbox {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub async fn arm(&self) -> PreflightTicket {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), tx);
        PreflightTicket { id, rx }
    }

    pub async fn submit(&self, ticket_id: TicketId, action: PreflightAction) {
        if let Some(tx) = self.pending.lock().await.remove(&ticket_id) {
            let _ = tx.send(action);
        }
    }

    pub async fn cancel(&self, ticket_id: &str) {
        self.pending.lock().await.remove(ticket_id);
    }
}

impl Default for PreflightInbox {
    fn default() -> Self {
        Self::new()
    }
}
