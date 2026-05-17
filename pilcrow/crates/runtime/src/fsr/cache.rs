#[cfg(feature = "live-props-redis")]
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Payload published on `pilcrow:invalidate` when deps are marked stale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidatePayload {
    pub route: String,
    pub slots: Vec<String>,
    pub deps: Vec<String>,
}

/// Payload published on `pilcrow:patch` after a slot is re-baked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchPayload {
    pub route: String,
    pub slot: String,
    pub value: serde_json::Value,
}

#[cfg(feature = "live-props-redis")]
mod inner {
    use super::*;
    use redis::{AsyncCommands, Client, aio::ConnectionManager};

    /// Redis cache layer for FSR.
    ///
    /// Wraps a multiplexed `ConnectionManager` for regular commands.
    /// Pub/sub connections are opened via `client()` and managed by the caller
    /// (pub/sub requires a dedicated connection separate from the command pool).
    #[derive(Clone)]
    pub struct RedisCache {
        client: Client,
        conn: ConnectionManager,
    }

    impl std::fmt::Debug for RedisCache {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("RedisCache")
                .field("client", &self.client.get_connection_info())
                .finish_non_exhaustive()
        }
    }

    impl RedisCache {
        /// Connect and return a `RedisCache`. Fails fast if the URL is unreachable.
        pub async fn connect(url: &str) -> redis::RedisResult<Self> {
            let client = Client::open(url)?;
            let conn = ConnectionManager::new(client.clone()).await?;
            Ok(Self { client, conn })
        }

        /// Raw `Client` — use to open dedicated pub/sub connections.
        pub fn client(&self) -> &Client {
            &self.client
        }

        /// `GET pilcrow:html:<route>`
        pub async fn get_html(&self, route: &str) -> redis::RedisResult<Option<String>> {
            let mut c = self.conn.clone();
            c.get(html_key(route)).await
        }

        /// `SET pilcrow:html:<route>`
        pub async fn set_html(&self, route: &str, html: &str) -> redis::RedisResult<()> {
            let mut c = self.conn.clone();
            c.set(html_key(route), html).await
        }

        /// `HSET pilcrow:slot:<route> <slot> <value>`
        pub async fn patch_slot(
            &self,
            route: &str,
            slot: &str,
            value: &str,
        ) -> redis::RedisResult<()> {
            let mut c = self.conn.clone();
            c.hset(slot_key(route), slot, value).await
        }

        /// `HGETALL pilcrow:slot:<route>`
        pub async fn get_slots(
            &self,
            route: &str,
        ) -> redis::RedisResult<HashMap<String, String>> {
            let mut c = self.conn.clone();
            c.hgetall(slot_key(route)).await
        }

        /// `SET pilcrow:json:<route>` (serialised JSON string)
        pub async fn set_json(
            &self,
            route: &str,
            json: &serde_json::Value,
        ) -> redis::RedisResult<()> {
            let mut c = self.conn.clone();
            c.set(json_key(route), json.to_string()).await
        }

        /// `GET pilcrow:json:<route>`
        pub async fn get_json(
            &self,
            route: &str,
        ) -> redis::RedisResult<Option<serde_json::Value>> {
            let mut c = self.conn.clone();
            let s: Option<String> = c.get(json_key(route)).await?;
            Ok(s.and_then(|s| serde_json::from_str(&s).ok()))
        }

        /// `PUBLISH pilcrow:invalidate <json>`
        pub async fn publish_invalidate(
            &self,
            payload: &InvalidatePayload,
        ) -> redis::RedisResult<()> {
            let mut c = self.conn.clone();
            let msg = serde_json::to_string(payload).unwrap_or_default();
            redis::cmd("PUBLISH")
                .arg("pilcrow:invalidate")
                .arg(msg)
                .query_async(&mut c)
                .await
        }

        /// `PUBLISH pilcrow:patch <json>`
        pub async fn publish_patch(&self, payload: &PatchPayload) -> redis::RedisResult<()> {
            let mut c = self.conn.clone();
            let msg = serde_json::to_string(payload).unwrap_or_default();
            redis::cmd("PUBLISH")
                .arg("pilcrow:patch")
                .arg(msg)
                .query_async(&mut c)
                .await
        }

        /// `DEL pilcrow:html:<route> pilcrow:slot:<route> pilcrow:json:<route>`
        ///
        /// Called by tombstone to evict all cached artifacts for a deleted route.
        /// Non-fatal: returns Ok even when keys are absent.
        pub async fn delete_route_keys(&self, route: &str) -> redis::RedisResult<()> {
            let mut c = self.conn.clone();
            c.del(&[html_key(route), slot_key(route), json_key(route)])
                .await
        }
    }

    fn html_key(route: &str) -> String {
        format!("pilcrow:html:{route}")
    }

    fn slot_key(route: &str) -> String {
        format!("pilcrow:slot:{route}")
    }

    fn json_key(route: &str) -> String {
        format!("pilcrow:json:{route}")
    }
}

#[cfg(feature = "live-props-redis")]
pub use inner::RedisCache;
