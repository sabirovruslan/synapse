use std::{env, error::Error, sync::Arc};

use bincode::{Decode, Encode};
use redis::{Client, pipe};
use serde::{Deserialize, Serialize};

const DEFAULT_REDIS_CHANNEL: &str = "synapse:cache_updates";
const DEFAULT_REDIS_PREFIX: &str = "synapse:cache:";

#[derive(Clone)]
pub struct RedisSync {
    pub client: Arc<Client>,
    pub key_prefix: String,
    pub channel: String,
}

#[derive(Serialize, Deserialize, Encode, Decode)]
pub(super) struct CacheUpdate {
    pub key: String,
    pub ttl_secs: Option<u64>,
}

impl RedisSync {
    pub fn from_env() -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let url = match env::var("SYNAPSE_REDIS_URL") {
            Ok(url) => url,
            Err(_) => return Ok(None),
        };
        let key_prefix =
            env::var("SYNAPSE_REDIS_PREFIX").unwrap_or_else(|_| DEFAULT_REDIS_PREFIX.to_string());
        let channel =
            env::var("SYNAPSE_REDIS_CHANNEL").unwrap_or_else(|_| DEFAULT_REDIS_CHANNEL.to_string());
        let client = Client::open(url)?;

        Ok(Some(Self {
            client: Arc::new(client),
            key_prefix,
            channel,
        }))
    }

    pub async fn set(
        &self,
        key: &str,
        value: &[u8],
        ttl_secs: Option<u64>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let redis_key = self.prefixed_key(key);

        let mut p = pipe();
        match ttl_secs {
            Some(ttl) => p.atomic().set_ex(&redis_key, value, ttl),
            None => p.atomic().set(&redis_key, value),
        };

        let message = CacheUpdate {
            key: key.to_string(),
            ttl_secs,
        };
        let payload = bincode::encode_to_vec(message, bincode::config::standard())?;
        p.publish(&self.channel, payload);

        let (_, _): ((), i64) = p.query_async(&mut conn).await?;

        Ok(())
    }

    pub(super) fn prefixed_key(&self, key: &str) -> String {
        if self.key_prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}{}", self.key_prefix, key)
        }
    }
}
