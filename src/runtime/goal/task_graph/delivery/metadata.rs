use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalTaskDeliveryStatus {
    Planned,
    InProgress,
    Blocked,
    ReadyForReview,
    Delivered,
    Merged,
    Other(String),
}

impl GoalTaskDeliveryStatus {
    pub fn as_str(&self) -> &str {
        match self {
            GoalTaskDeliveryStatus::Planned => "planned",
            GoalTaskDeliveryStatus::InProgress => "in_progress",
            GoalTaskDeliveryStatus::Blocked => "blocked",
            GoalTaskDeliveryStatus::ReadyForReview => "ready_for_review",
            GoalTaskDeliveryStatus::Delivered => "delivered",
            GoalTaskDeliveryStatus::Merged => "merged",
            GoalTaskDeliveryStatus::Other(status) => status.as_str(),
        }
    }
}

impl From<String> for GoalTaskDeliveryStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "planned" => GoalTaskDeliveryStatus::Planned,
            "in_progress" => GoalTaskDeliveryStatus::InProgress,
            "blocked" => GoalTaskDeliveryStatus::Blocked,
            "ready_for_review" => GoalTaskDeliveryStatus::ReadyForReview,
            "delivered" => GoalTaskDeliveryStatus::Delivered,
            "merged" => GoalTaskDeliveryStatus::Merged,
            _ => GoalTaskDeliveryStatus::Other(value),
        }
    }
}

impl Serialize for GoalTaskDeliveryStatus {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for GoalTaskDeliveryStatus {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(String::deserialize(deserializer)?.into())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GoalTaskDeliveryMetadata {
    pub owner: Option<String>,
    pub write_scope: Vec<String>,
    pub branch: Option<String>,
    pub worktree_path: Option<PathBuf>,
    pub pr_url: Option<String>,
    pub commit_sha: Option<String>,
    pub verification_summary: Option<String>,
    pub status: Option<GoalTaskDeliveryStatus>,
    pub extra: Map<String, Value>,
}

impl GoalTaskDeliveryMetadata {
    pub fn from_value(value: &Value) -> Self {
        let Some(object) = value.as_object() else {
            return Self::default();
        };
        let mut extra = object.clone();
        let owner = take_string(&mut extra, "owner");
        let write_scope = take_string_array(&mut extra, "write_scope").unwrap_or_default();
        let branch = take_string(&mut extra, "branch");
        let worktree_path = take_string(&mut extra, "worktree_path").map(PathBuf::from);
        let pr_url = take_string(&mut extra, "pr_url");
        let commit_sha = take_string(&mut extra, "commit_sha");
        let verification_summary = take_string(&mut extra, "verification_summary");
        let status = take_string(&mut extra, "status").map(GoalTaskDeliveryStatus::from);

        Self {
            owner,
            write_scope,
            branch,
            worktree_path,
            pr_url,
            commit_sha,
            verification_summary,
            status,
            extra,
        }
    }

    pub fn to_value(&self) -> Value {
        let mut object = self.extra.clone();
        insert_string(&mut object, "owner", self.owner.as_deref());
        if !self.write_scope.is_empty() {
            object.insert(
                "write_scope".to_string(),
                serde_json::json!(self.write_scope),
            );
        }
        insert_string(&mut object, "branch", self.branch.as_deref());
        let worktree_path = self
            .worktree_path
            .as_ref()
            .map(|path| path.display().to_string());
        insert_string(&mut object, "worktree_path", worktree_path.as_deref());
        insert_string(&mut object, "pr_url", self.pr_url.as_deref());
        insert_string(&mut object, "commit_sha", self.commit_sha.as_deref());
        insert_string(
            &mut object,
            "verification_summary",
            self.verification_summary.as_deref(),
        );
        insert_string(
            &mut object,
            "status",
            self.status.as_ref().map(GoalTaskDeliveryStatus::as_str),
        );
        Value::Object(object)
    }

    pub fn merge_update(&mut self, update: GoalTaskDeliveryMetadataUpdate) {
        replace_if_some(&mut self.owner, update.owner);
        if let Some(write_scope) = update.write_scope {
            self.write_scope = write_scope;
        }
        replace_if_some(&mut self.branch, update.branch);
        replace_if_some(&mut self.worktree_path, update.worktree_path);
        replace_if_some(&mut self.pr_url, update.pr_url);
        replace_if_some(&mut self.commit_sha, update.commit_sha);
        replace_if_some(&mut self.verification_summary, update.verification_summary);
        replace_if_some(&mut self.status, update.status);
        self.extra.extend(update.extra);
    }

    pub fn is_empty(&self) -> bool {
        matches!(self.to_value(), Value::Object(object) if object.is_empty())
    }
}

impl Serialize for GoalTaskDeliveryMetadata {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GoalTaskDeliveryMetadata {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if !value.is_object() {
            return Err(D::Error::custom("delivery metadata must be a JSON object"));
        }
        Ok(Self::from_value(&value))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GoalTaskDeliveryMetadataUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_scope: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<GoalTaskDeliveryStatus>,
    #[serde(default, flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GoalTaskDeliveryRecord {
    pub task_id: String,
    #[serde(flatten)]
    pub metadata: GoalTaskDeliveryMetadata,
}

impl Serialize for GoalTaskDeliveryRecord {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = self.metadata.to_value();
        if let Some(object) = value.as_object_mut() {
            object.insert("task_id".to_string(), Value::String(self.task_id.clone()));
        }
        value.serialize(serializer)
    }
}

fn take_string(object: &mut Map<String, Value>, key: &str) -> Option<String> {
    let value = object.get(key)?.as_str()?.to_string();
    object.remove(key);
    Some(value)
}

fn take_string_array(object: &mut Map<String, Value>, key: &str) -> Option<Vec<String>> {
    let values = serde_json::from_value(object.get(key)?.clone()).ok()?;
    object.remove(key);
    Some(values)
}

fn insert_string(object: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        object.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn replace_if_some<T>(target: &mut Option<T>, value: Option<T>) {
    if let Some(value) = value {
        *target = Some(value);
    }
}
