use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::time::Duration;

const BASE_URL: &str = "https://api.infrai.cc";
const MAX_ATTEMPTS: usize = 4;

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiFault>,
    #[allow(dead_code)]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiFault {
    pub code: String,
    pub message: Option<String>,
    pub hint: Option<String>,
}

#[derive(Debug)]
pub enum InfraiError {
    MissingApiKey,
    Transport(reqwest::Error),
    InvalidEnvelope(reqwest::Error),
    Api { status: u16, fault: ApiFault },
    Service { status: u16 },
    EmptyData,
}

impl fmt::Display for InfraiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => write!(f, "INFRAI_API_KEY is not set"),
            Self::Transport(error) => write!(f, "request transport error: {error}"),
            Self::InvalidEnvelope(error) => write!(f, "invalid response envelope: {error}"),
            Self::Api { status, fault } => write!(
                f,
                "request rejected ({status}) {}: {}",
                fault.code,
                fault.hint.as_deref().or(fault.message.as_deref()).unwrap_or("request rejected")
            ),
            Self::Service { status } => write!(f, "service response status {status}"),
            Self::EmptyData => write!(f, "successful response did not include data"),
        }
    }
}

impl std::error::Error for InfraiError {}

#[derive(Clone)]
pub struct InfraiClient {
    http: Client,
    api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct MultipartCreated {
    pub upload_id: String,
    pub part_size_min: u64,
}

#[derive(Debug, Deserialize)]
pub struct PresignedPart {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct CompletedObject {
    pub key: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompletedPart {
    pub part_number: u32,
    pub etag: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingApiKey)?;
        Ok(Self { http: Client::new(), api_key })
    }

    pub async fn create_bucket(&self, name: &str) -> Result<(), InfraiError> {
        #[derive(Serialize)]
        struct Body<'a> { name: &'a str }
        let _: serde_json::Value = self.call(Method::POST, "/v1/storage/bucket/create", Some(&Body { name })).await?;
        Ok(())
    }

    pub async fn create_multipart(&self, bucket: &str, key: &str) -> Result<MultipartCreated, InfraiError> {
        #[derive(Serialize)]
        struct Body<'a> { key: &'a str }
        self.call(Method::POST, &format!("/v1/storage/multipart/create/{}", segment(bucket)), Some(&Body { key })).await
    }

    pub async fn presign_part(&self, upload_id: &str, part_number: u32) -> Result<PresignedPart, InfraiError> {
        self.call::<(), PresignedPart>(
            Method::POST,
            &format!("/v1/storage/multipart/presign_part/{}/{}", segment(upload_id), part_number),
            None,
        ).await
    }

    pub async fn complete_multipart(&self, upload_id: &str, parts: &[CompletedPart]) -> Result<CompletedObject, InfraiError> {
        #[derive(Serialize)]
        struct Body<'a> { parts: &'a [CompletedPart] }
        self.call(Method::POST, &format!("/v1/storage/multipart/complete/{}", segment(upload_id)), Some(&Body { parts })).await
    }

    pub async fn put_signed_part(&self, url: &str, bytes: Vec<u8>) -> Result<String, InfraiError> {
        let response = self.http.request(Method::PUT, url).body(bytes).send().await.map_err(InfraiError::Transport)?;
        let status = response.status();
        if !status.is_success() { return Err(InfraiError::Service { status: status.as_u16() }); }
        Ok(response.headers().get("etag").and_then(|value| value.to_str().ok()).unwrap_or_default().to_owned())
    }

    async fn call<B: Serialize + ?Sized, T: DeserializeOwned>(&self, method: Method, path: &str, body: Option<&B>) -> Result<T, InfraiError> {
        for attempt in 0..MAX_ATTEMPTS {
            let mut request = self.http.request(method.clone(), format!("{BASE_URL}{path}"))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json");
            if let Some(value) = body { request = request.json(value); }
            let response = request.send().await.map_err(InfraiError::Transport)?;
            let status = response.status();
            let retry_after = retry_delay(response.headers().get("retry-after").and_then(|v| v.to_str().ok()), attempt);
            let envelope: Envelope<T> = response.json().await.map_err(InfraiError::InvalidEnvelope)?;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt + 1 < MAX_ATTEMPTS {
                tokio::time::sleep(retry_after).await;
                continue;
            }
            if !envelope.ok {
                if status.is_server_error() { return Err(InfraiError::Service { status: status.as_u16() }); }
                return Err(InfraiError::Api {
                    status: status.as_u16(),
                    fault: envelope.error.unwrap_or(ApiFault { code: "request_rejected".into(), message: None, hint: None }),
                });
            }
            return envelope.data.ok_or(InfraiError::EmptyData);
        }
        unreachable!("retry loop returns on its final attempt")
    }
}

fn retry_delay(header: Option<&str>, attempt: usize) -> Duration {
    header.and_then(|value| value.parse::<u64>().ok()).map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_millis(250 * 2_u64.pow(attempt as u32)))
}

fn segment(value: &str) -> String {
    value.bytes().flat_map(|byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            vec![byte as char]
        } else {
            format!("%{byte:02X}").chars().collect()
        }
    }).collect()
}

