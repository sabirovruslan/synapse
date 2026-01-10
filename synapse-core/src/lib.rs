use std::time::{Duration, Instant};

use moka::Expiry;
use moka::future::Cache;
use serde::{Deserialize, Serialize};

pub const MAX_FRAME_LENGTH: usize = 64 * 1024 * 1024;

pub const OP_GET: u8 = 1;
pub const OP_SET: u8 = 2;

pub const RES_OK: u8 = 0;
pub const RES_HIT: u8 = 1;
pub const RES_MISS: u8 = 2;
pub const RES_ERR: u8 = 3;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CacheCommand {
    Get {
        key: String,
    },
    Set {
        key: String,
        value: Vec<u8>,
        ttl_secs: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CacheResponce {
    Ok,
    Hit(Vec<u8>),
    Miss,
    Error(String),
}

#[derive(Clone)]
struct Entry {
    value: Vec<u8>,
    expires_at: Option<Instant>,
}

struct EntryExpiry;

impl<K> Expiry<K, Entry> for EntryExpiry {
    fn expire_after_create(
        &self,
        _key: &K,
        value: &Entry,
        _created_at: Instant,
    ) -> Option<Duration> {
        value
            .expires_at
            .map(|t| t.saturating_duration_since(Instant::now()))
    }
}

#[derive(Clone)]
pub struct L1Cache {
    inner: Cache<String, Entry>,
}

impl L1Cache {
    pub fn new(max_capacity: u64) -> Self {
        let inner = Cache::builder()
            .expire_after(EntryExpiry)
            .max_capacity(max_capacity)
            .build();

        Self { inner }
    }

    pub async fn get(&self, key: &str) -> CacheResponce {
        match self.inner.get(key).await {
            Some(entry) => CacheResponce::Hit(entry.value),
            None => CacheResponce::Miss,
        }
    }

    pub async fn set(&self, key: String, value: Vec<u8>, ttl_secs: Option<u64>) {
        let ttl = ttl_secs.map(|s| Instant::now() + Duration::from_secs(s));
        let entry = Entry {
            value,
            expires_at: ttl,
        };
        self.inner.insert(key, entry).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{CacheResponce, L1Cache};
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn cache_get_miss() {
        let cache = L1Cache::new(10);
        assert!(matches!(cache.get("missing").await, CacheResponce::Miss));
    }

    #[tokio::test]
    async fn cache_set_get_hit() {
        let cache = L1Cache::new(10);
        cache
            .set("alpha".to_string(), b"hello".to_vec(), None)
            .await;
        match cache.get("alpha").await {
            CacheResponce::Hit(bytes) => assert_eq!(bytes, b"hello".to_vec()),
            other => panic!("Expected Hit, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn cache_ttl_expiry() {
        let cache = L1Cache::new(10);
        cache
            .set("beta".to_string(), b"value".to_vec(), Some(1))
            .await;
        sleep(Duration::from_millis(1100)).await;
        assert!(matches!(cache.get("beta").await, CacheResponce::Miss));
    }

    #[tokio::test]
    async fn cache_capacity() {
        let ttl_secs = Some(30);
        let cache = L1Cache::new(3);
        cache
            .set("key_1".to_string(), b"value 1".to_vec(), ttl_secs)
            .await;
        cache
            .set("key_2".to_string(), b"value 2".to_vec(), ttl_secs)
            .await;
        cache
            .set("key_3".to_string(), b"value 3".to_vec(), ttl_secs)
            .await;
        cache
            .set("key_4".to_string(), b"value 4".to_vec(), ttl_secs)
            .await;

        cache.inner.run_pending_tasks().await;
        let count = cache.inner.entry_count();
        assert!(
            count == 3,
            "capacity=3 should not retain all keys: count={}",
            count
        );
    }
}
