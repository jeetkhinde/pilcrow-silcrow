use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Meta {
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiEnvelope<T> {
    pub data: T,
    pub meta: Meta,
}

impl<T> ApiEnvelope<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            meta: Meta { request_id: None },
        }
    }
}
