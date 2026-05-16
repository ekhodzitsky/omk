use std::collections::HashMap;

use super::task::{Task, TaskId};

/// Tracks which worker owns which file paths to prevent silent overwrites.
#[derive(Debug)]
pub struct OwnershipMap {
    /// path -> (owner_worker, task_id)
    writes: HashMap<String, (String, TaskId)>,
}

impl OwnershipMap {
    pub fn new() -> Self {
        Self {
            writes: HashMap::new(),
        }
    }

    /// Register a task's write set. Returns conflicting paths if any.
    pub fn register_task(&mut self, task: &Task) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        let worker = match &task.owner {
            Some(w) => w.clone(),
            None => return conflicts,
        };

        for path in &task.write_set {
            if let Some((existing_worker, existing_task)) = self.writes.get(path) {
                if existing_worker != &worker {
                    conflicts.push(Conflict {
                        path: path.clone(),
                        existing_worker: existing_worker.clone(),
                        existing_task: existing_task.clone(),
                        new_worker: worker.clone(),
                        new_task: task.id.clone(),
                    });
                }
            } else {
                self.writes
                    .insert(path.clone(), (worker.clone(), task.id.clone()));
            }
        }

        conflicts
    }

    /// Release ownership for a completed or failed task.
    pub fn release_task(&mut self, task: &Task) {
        for path in &task.write_set {
            if let Some((_, tid)) = self.writes.get(path) {
                if tid == &task.id {
                    self.writes.remove(path);
                }
            }
        }
    }

    /// Check if a new task would conflict before claiming.
    pub fn would_conflict(&self, task: &Task, worker: &str) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        for path in &task.write_set {
            if let Some((existing_worker, existing_task)) = self.writes.get(path) {
                if existing_worker != worker {
                    conflicts.push(Conflict {
                        path: path.clone(),
                        existing_worker: existing_worker.clone(),
                        existing_task: existing_task.clone(),
                        new_worker: worker.to_string(),
                        new_task: task.id.clone(),
                    });
                }
            }
        }
        conflicts
    }

    /// Check read-after-write hazards: a task reads a file that another pending task will write.
    pub fn read_write_hazards(&self, task: &Task, pending_tasks: &[&Task]) -> Vec<ReadWriteHazard> {
        let mut hazards = Vec::new();
        for path in &task.read_set {
            for pending in pending_tasks {
                if pending.id == task.id {
                    continue;
                }
                if pending.write_set.contains(path) {
                    hazards.push(ReadWriteHazard {
                        path: path.clone(),
                        reader_task: task.id.clone(),
                        writer_task: pending.id.clone(),
                    });
                }
            }
        }
        hazards
    }
}

impl Default for OwnershipMap {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub path: String,
    pub existing_worker: String,
    pub existing_task: TaskId,
    pub new_worker: String,
    pub new_task: TaskId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadWriteHazard {
    pub path: String,
    pub reader_task: TaskId,
    pub writer_task: TaskId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_conflict_on_same_worker() {
        let mut map = OwnershipMap::new();
        let mut task = Task::new("t1", "task 1");
        task.owner = Some("worker-a".to_string());
        task.write_set = vec!["src/main.rs".to_string()];

        let conflicts = map.register_task(&task);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn conflict_on_different_workers() {
        let mut map = OwnershipMap::new();

        let mut t1 = Task::new("t1", "task 1");
        t1.owner = Some("worker-a".to_string());
        t1.write_set = vec!["src/main.rs".to_string()];
        map.register_task(&t1);

        let mut t2 = Task::new("t2", "task 2");
        t2.owner = Some("worker-b".to_string());
        t2.write_set = vec!["src/main.rs".to_string()];
        let conflicts = map.register_task(&t2);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].path, "src/main.rs");
        assert_eq!(conflicts[0].existing_worker, "worker-a");
        assert_eq!(conflicts[0].new_worker, "worker-b");
    }

    #[test]
    fn release_ownership() {
        let mut map = OwnershipMap::new();
        let mut task = Task::new("t1", "task 1");
        task.owner = Some("worker-a".to_string());
        task.write_set = vec!["src/main.rs".to_string()];
        map.register_task(&task);

        map.release_task(&task);

        let mut t2 = Task::new("t2", "task 2");
        t2.owner = Some("worker-b".to_string());
        t2.write_set = vec!["src/main.rs".to_string()];
        let conflicts = map.register_task(&t2);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn read_write_hazard_detected() {
        let map = OwnershipMap::new();
        let mut reader = Task::new("read", "read task");
        reader.read_set = vec!["src/main.rs".to_string()];

        let mut writer = Task::new("write", "write task");
        writer.write_set = vec!["src/main.rs".to_string()];

        let hazards = map.read_write_hazards(&reader, &[&writer]);
        assert_eq!(hazards.len(), 1);
        assert_eq!(hazards[0].path, "src/main.rs");
    }
}
