use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::GoalProof;

static LOADED_PROOF_METADATA: OnceLock<Mutex<HashMap<String, Vec<Value>>>> = OnceLock::new();
static LOADED_PROOF_REVIEW_ARTIFACTS: OnceLock<Mutex<HashMap<String, Vec<Value>>>> =
    OnceLock::new();
static LOADED_PROOF_INTEGRATION_EVIDENCE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
static LOADED_PROOF_ORACLE_EVIDENCE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();

pub(crate) fn proof_delivery_metadata_from_value(value: &Value) -> Vec<Value> {
    value
        .get("delivery_metadata")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .cloned()
        .collect()
}

pub(crate) fn proof_review_artifacts_from_value(value: &Value) -> Vec<Value> {
    value
        .get("review_artifacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .cloned()
        .collect()
}

pub(crate) fn proof_integration_evidence_from_value(value: &Value) -> Option<Value> {
    value.get("integration_evidence").cloned()
}

pub(crate) fn proof_oracle_evidence_from_value(value: &Value) -> Option<Value> {
    value.get("oracle_evidence").cloned()
}

pub(crate) fn remember_goal_proof_delivery_metadata_for_value(
    proof_value: &Value,
    delivery_metadata: Vec<Value>,
) {
    let Some(key) = proof_cache_key_from_value(proof_value) else {
        return;
    };
    remember_goal_proof_delivery_metadata_with_key(key, delivery_metadata);
}

pub(crate) fn remember_goal_proof_delivery_metadata(
    proof: &GoalProof,
    delivery_metadata: Vec<Value>,
) {
    remember_goal_proof_delivery_metadata_with_key(proof_cache_key(proof), delivery_metadata);
}

pub(crate) fn remember_goal_proof_review_artifacts(
    proof: &GoalProof,
    review_artifacts: Vec<Value>,
) {
    remember_goal_proof_review_artifacts_with_key(proof_cache_key(proof), review_artifacts);
}

pub(crate) fn remember_goal_proof_acceptance_evidence(
    proof: &GoalProof,
    integration_evidence: Value,
    oracle_evidence: Value,
) {
    let key = proof_cache_key(proof);
    remember_value_with_key(
        LOADED_PROOF_INTEGRATION_EVIDENCE.get_or_init(|| Mutex::new(HashMap::new())),
        key.clone(),
        Some(integration_evidence),
    );
    remember_value_with_key(
        LOADED_PROOF_ORACLE_EVIDENCE.get_or_init(|| Mutex::new(HashMap::new())),
        key,
        Some(oracle_evidence),
    );
}

pub(crate) fn remember_goal_proof_acceptance_evidence_for_value(
    proof_value: &Value,
    integration_evidence: Option<Value>,
    oracle_evidence: Option<Value>,
) {
    let Some(key) = proof_cache_key_from_value(proof_value) else {
        return;
    };
    remember_value_with_key(
        LOADED_PROOF_INTEGRATION_EVIDENCE.get_or_init(|| Mutex::new(HashMap::new())),
        key.clone(),
        integration_evidence,
    );
    remember_value_with_key(
        LOADED_PROOF_ORACLE_EVIDENCE.get_or_init(|| Mutex::new(HashMap::new())),
        key,
        oracle_evidence,
    );
}

pub(crate) fn remembered_goal_proof_delivery_metadata(proof: &GoalProof) -> Option<Vec<Value>> {
    let cache = LOADED_PROOF_METADATA.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(cache) = cache.lock() else {
        return None;
    };
    cache.get(&proof_cache_key(proof)).cloned()
}

pub(crate) fn remembered_goal_proof_review_artifacts(proof: &GoalProof) -> Option<Vec<Value>> {
    let cache = LOADED_PROOF_REVIEW_ARTIFACTS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(cache) = cache.lock() else {
        return None;
    };
    cache.get(&proof_cache_key(proof)).cloned()
}

pub(crate) fn remembered_goal_proof_integration_evidence(proof: &GoalProof) -> Option<Value> {
    remembered_value(
        LOADED_PROOF_INTEGRATION_EVIDENCE.get_or_init(|| Mutex::new(HashMap::new())),
        &proof_cache_key(proof),
    )
}

pub(crate) fn remembered_goal_proof_oracle_evidence(proof: &GoalProof) -> Option<Value> {
    remembered_value(
        LOADED_PROOF_ORACLE_EVIDENCE.get_or_init(|| Mutex::new(HashMap::new())),
        &proof_cache_key(proof),
    )
}

fn proof_cache_key(proof: &GoalProof) -> String {
    let version = proof.version.to_string();
    let status = proof.status.to_string();
    proof_cache_key_parts(&[
        &version,
        &proof.goal_id,
        &status,
        &proof.readiness,
        &proof.summary,
    ])
}

fn proof_cache_key_from_value(value: &Value) -> Option<String> {
    let version = value.get("version")?.as_u64()?.to_string();
    let goal_id = value.get("goal_id")?.as_str()?;
    let status = value.get("status")?.as_str()?;
    let readiness = value.get("readiness")?.as_str()?;
    let summary = value.get("summary")?.as_str()?;
    Some(proof_cache_key_parts(&[
        &version, goal_id, status, readiness, summary,
    ]))
}

fn proof_cache_key_parts(parts: &[&str]) -> String {
    parts.join("\n")
}

fn remember_goal_proof_delivery_metadata_with_key(key: String, delivery_metadata: Vec<Value>) {
    let cache = LOADED_PROOF_METADATA.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut cache) = cache.lock() else {
        return;
    };
    if delivery_metadata.is_empty() {
        cache.remove(&key);
    } else {
        cache.insert(key, delivery_metadata);
    }
}

fn remember_goal_proof_review_artifacts_with_key(key: String, review_artifacts: Vec<Value>) {
    let cache = LOADED_PROOF_REVIEW_ARTIFACTS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut cache) = cache.lock() else {
        return;
    };
    if review_artifacts.is_empty() {
        cache.remove(&key);
    } else {
        cache.insert(key, review_artifacts);
    }
}

fn remember_value_with_key(
    cache: &Mutex<HashMap<String, Value>>,
    key: String,
    value: Option<Value>,
) {
    let Ok(mut cache) = cache.lock() else {
        return;
    };
    if let Some(value) = value {
        cache.insert(key, value);
    } else {
        cache.remove(&key);
    }
}

fn remembered_value(cache: &Mutex<HashMap<String, Value>>, key: &str) -> Option<Value> {
    let Ok(cache) = cache.lock() else {
        return None;
    };
    cache.get(key).cloned()
}
