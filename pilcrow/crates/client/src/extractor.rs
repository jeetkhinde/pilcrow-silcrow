use crate::client::PilcrowClient;
use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use pilcrow_core::PilcrowConfig;
use std::sync::Arc;

#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for PilcrowClient {
    type Rejection = (axum::http::StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let config = parts
            .extensions
            .get::<Arc<PilcrowConfig>>()
            .ok_or_else(|| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "PilcrowConfig not in extensions — was pilcrow::start() called?".to_string(),
                )
            })?;

        let http = parts
            .extensions
            .get::<reqwest::Client>()
            .cloned()
            .unwrap_or_default();

        Ok(PilcrowClient::new(config.web.backend_url.clone(), http))
    }
}
