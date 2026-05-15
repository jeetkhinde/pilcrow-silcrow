use crate::error::ClientError;
use pilcrow_core::AppResult;
use serde::{Serialize, de::DeserializeOwned};

#[derive(Clone)]
pub struct PilcrowClient {
    base_url: String,
    http: reqwest::Client,
}

impl PilcrowClient {
    pub(crate) fn new(base_url: String, http: reqwest::Client) -> Self {
        Self { base_url, http }
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> AppResult<T> {
        self.send(self.http.get(self.url(path))).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: B,
    ) -> AppResult<T> {
        self.send(self.http.post(self.url(path)).json(&body)).await
    }

    pub async fn put<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: B,
    ) -> AppResult<T> {
        self.send(self.http.put(self.url(path)).json(&body)).await
    }

    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> AppResult<T> {
        self.send(self.http.delete(self.url(path))).await
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }

    async fn send<T: DeserializeOwned>(&self, req: reqwest::RequestBuilder) -> AppResult<T> {
        let res = req.send().await.map_err(ClientError::Http)?;
        let status = res.status().as_u16();
        if status >= 400 {
            let body = res.text().await.unwrap_or_default();
            return Err(ClientError::Backend { status, body }.into());
        }
        Ok(res.json::<T>().await.map_err(ClientError::Http)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_formatting() {
        let client = PilcrowClient::new("http://localhost:3000/".into(), reqwest::Client::new());
        assert_eq!(client.url("/api/users"), "http://localhost:3000/api/users");

        let client2 = PilcrowClient::new("http://localhost:3000".into(), reqwest::Client::new());
        assert_eq!(client2.url("/api/users"), "http://localhost:3000/api/users");
    }
}
