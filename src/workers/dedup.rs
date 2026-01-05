use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::debug;

/// Deduplication cache to prevent duplicate message processing
/// Uses content hash with TTL-based expiration
#[derive(Clone)]
pub struct DedupCache {
    cache: Arc<RwLock<HashMap<u64, Instant>>>,
    ttl_seconds: u64,
}

impl DedupCache {
    /// Create a new dedup cache with specified TTL in seconds
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl_seconds,
        }
    }

    /// Check if message is a duplicate. Returns true if duplicate, false if new.
    /// If new, adds to cache automatically.
    pub fn is_duplicate(&self, content: &str) -> bool {
        let hash = Self::hash_content(content);
        let now = Instant::now();
        let ttl = Duration::from_secs(self.ttl_seconds);

        // First, try to read without write lock
        {
            let cache = self.cache.read().unwrap();
            if let Some(timestamp) = cache.get(&hash) {
                if now.duration_since(*timestamp) < ttl {
                    debug!("Duplicate message detected (hash: {})", hash);
                    return true;
                }
            }
        }

        // Not a duplicate or expired, add to cache with write lock
        {
            let mut cache = self.cache.write().unwrap();
            
            // Clean up expired entries periodically (every 100 entries)
            if cache.len() > 100 {
                cache.retain(|_, timestamp| now.duration_since(*timestamp) < ttl);
            }

            cache.insert(hash, now);
        }

        false
    }

    /// Simple hash function using FNV-1a
    fn hash_content(content: &str) -> u64 {
        const FNV_OFFSET: u64 = 14695981039346656037;
        const FNV_PRIME: u64 = 1099511628211;

        let mut hash = FNV_OFFSET;
        for byte in content.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    /// Get TTL in seconds
    pub fn ttl_seconds(&self) -> u64 {
        self.ttl_seconds
    }
}

/// Get dedup TTL from environment, default 5 seconds
pub fn get_dedup_ttl() -> u64 {
    std::env::var("DEDUP_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
}

/// Get max messages per credential from environment, default 50
pub fn get_max_messages_per_credential() -> i64 {
    std::env::var("MAX_MESSAGES_PER_CREDENTIAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_dedup_cache() {
        let cache = DedupCache::new(1); // 1 second TTL
        
        // First message should not be duplicate
        assert!(!cache.is_duplicate("test message"));
        
        // Same message should be duplicate
        assert!(cache.is_duplicate("test message"));
        
        // Different message should not be duplicate
        assert!(!cache.is_duplicate("different message"));
        
        // Wait for TTL to expire
        sleep(Duration::from_secs(2));
        
        // Same message should no longer be duplicate
        assert!(!cache.is_duplicate("test message"));
    }
}
