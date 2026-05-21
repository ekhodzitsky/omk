use lru::LruCache;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use super::types::ClassifierOutput;

pub fn new_session_cache() -> LruCache<u64, ClassifierOutput> {
    // SAFETY: 50 is statically known to be non-zero.
    #[allow(clippy::unwrap_used)]
    LruCache::new(NonZeroUsize::new(50).unwrap())
}

pub fn cache_key(prompt: &str) -> u64 {
    let normalized = prompt.trim().to_lowercase();
    let collapsed = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut hasher = DefaultHasher::new();
    collapsed.hash(&mut hasher);
    hasher.finish()
}

pub fn invalidate(cache: &mut LruCache<u64, ClassifierOutput>) {
    cache.clear();
}
