#![cfg(feature = "http-mailbox")]

use bytes::Bytes;
use reqwest::StatusCode;

#[derive(Clone)]
pub struct HttpMailboxClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum HttpMailboxError {
    #[error("http error: {0}")]
    Http(String),
    #[error("bad response: {0}")]
    BadResponse(String),
}

impl HttpMailboxClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, HttpMailboxError> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .map_err(|e| HttpMailboxError::Http(e.to_string()))?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client,
        })
    }

    fn mailbox_url(&self, rid32: &[u8; 32]) -> String {
        let rid_hex = hex::encode(rid32);
        format!("{}/v1/mailbox/{}", self.base_url, rid_hex)
    }

    /// POST envelope bytes to recipient mailbox.
    pub async fn post(&self, rid32: &[u8; 32], envelope_bytes: &[u8]) -> Result<(), HttpMailboxError> {
        let url = self.mailbox_url(rid32);
        let resp = self
            .client
            .post(url)
            .body(envelope_bytes.to_vec())
            .send()
            .await
            .map_err(|e| HttpMailboxError::Http(e.to_string()))?;

        if resp.status() == StatusCode::ACCEPTED {
            Ok(())
        } else {
            Err(HttpMailboxError::BadResponse(format!(
                "status={} body={:?}",
                resp.status(),
                resp.text().await.ok()
            )))
        }
    }

    /// Long-poll: GET next envelope bytes for this mailbox. Returns None on 204.
    pub async fn poll(&self, my_id32: &[u8; 32], wait_ms: u64) -> Result<Option<Bytes>, HttpMailboxError> {
        let mut url = self.mailbox_url(my_id32);
        url.push_str(&format!("?wait_ms={}", wait_ms));

        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| HttpMailboxError::Http(e.to_string()))?;

        match resp.status() {
            StatusCode::OK => {
                let b = resp.bytes().await.map_err(|e| HttpMailboxError::Http(e.to_string()))?;
                Ok(Some(Bytes::from(b.to_vec())))
            }
            StatusCode::NO_CONTENT => Ok(None),
            other => Err(HttpMailboxError::BadResponse(format!(
                "status={} body={:?}",
                other,
                resp.text().await.ok()
            ))),
        }
    }
}

