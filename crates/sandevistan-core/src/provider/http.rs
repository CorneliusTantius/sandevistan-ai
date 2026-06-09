use crate::ProviderConfig;
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: Option<ApiErrorBody>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    message: Option<String>,
}

const API_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
pub(super) const API_STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(45);
const API_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(180);
const API_TCP_KEEPALIVE: Duration = Duration::from_secs(60);
const API_RETRY_ATTEMPTS: usize = 2;

pub(super) async fn post_json<T: Serialize>(
    runtime: &ProviderConfig,
    path: &str,
    request: &T,
) -> Result<String, String> {
    let mut last_error = String::new();
    for attempt in 1..=API_RETRY_ATTEMPTS {
        match send_json_once(runtime, path, request).await {
            Ok(body) => return Ok(body),
            Err(error) if attempt < API_RETRY_ATTEMPTS && error.retryable => {
                last_error = error.message;
                retry_delay(attempt).await;
            }
            Err(error) => return Err(error.message),
        }
    }
    Err(last_error)
}

async fn send_json_once<T: Serialize>(
    runtime: &ProviderConfig,
    path: &str,
    request: &T,
) -> Result<String, ApiRequestError> {
    let response = tokio::time::timeout(
        API_REQUEST_TIMEOUT,
        request_builder(runtime, path, request).send(),
    )
    .await
    .map_err(|_| ApiRequestError::retryable("api request timed out"))?
    .map_err(|error| ApiRequestError::from_reqwest("api request failed", error))?;

    let status = response.status();
    let body = tokio::time::timeout(API_REQUEST_TIMEOUT, response.text())
        .await
        .map_err(|_| ApiRequestError::retryable("api response read timed out"))?
        .map_err(|error| ApiRequestError::from_reqwest("api response read failed", error))?;

    if !status.is_success() {
        return Err(ApiRequestError {
            message: api_error(status, body),
            retryable: is_retryable_status(status),
        });
    }

    Ok(body)
}

fn request_builder<T: Serialize>(
    runtime: &ProviderConfig,
    path: &str,
    request: &T,
) -> reqwest::RequestBuilder {
    let endpoint = format!("{}/{}", runtime.api_base.trim_end_matches('/'), path);
    let mut builder = http_client().post(endpoint).json(request);

    if let Some(key) = &runtime.api_key {
        if runtime.api_key_header == "api-key" {
            builder = builder.header("api-key", key);
        } else {
            builder = builder.bearer_auth(key);
        }
    }

    builder
}

pub(super) async fn open_stream_with_retry<T: Serialize>(
    runtime: &ProviderConfig,
    path: &str,
    request: &T,
) -> Result<reqwest::Response, String> {
    let mut last_error = String::new();
    for attempt in 1..=API_RETRY_ATTEMPTS {
        match open_stream_once(runtime, path, request).await {
            Ok(response) => return Ok(response),
            Err(error) if attempt < API_RETRY_ATTEMPTS && error.retryable => {
                last_error = error.message;
                retry_delay(attempt).await;
            }
            Err(error) => return Err(error.message),
        }
    }
    Err(last_error)
}

async fn open_stream_once<T: Serialize>(
    runtime: &ProviderConfig,
    path: &str,
    request: &T,
) -> Result<reqwest::Response, ApiRequestError> {
    let response = tokio::time::timeout(
        API_REQUEST_TIMEOUT,
        request_builder(runtime, path, request).send(),
    )
    .await
    .map_err(|_| ApiRequestError::retryable("api request timed out"))?
    .map_err(|error| ApiRequestError::from_reqwest("api request failed", error))?;

    let status = response.status();
    if !status.is_success() {
        let body = tokio::time::timeout(API_REQUEST_TIMEOUT, response.text())
            .await
            .map_err(|_| ApiRequestError::retryable("api response read timed out"))?
            .map_err(|error| ApiRequestError::from_reqwest("api response read failed", error))?;
        return Err(ApiRequestError {
            message: api_error(status, body),
            retryable: is_retryable_status(status),
        });
    }

    Ok(response)
}

#[derive(Debug)]
struct ApiRequestError {
    message: String,
    retryable: bool,
}

impl ApiRequestError {
    fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }

    fn from_reqwest(prefix: &str, error: reqwest::Error) -> Self {
        Self {
            retryable: error.is_timeout() || error.is_connect(),
            message: format!("{prefix}: {error}"),
        }
    }
}

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

async fn retry_delay(attempt: usize) {
    tokio::time::sleep(Duration::from_millis(750 * attempt as u64)).await;
}

fn api_error(status: reqwest::StatusCode, body: String) -> String {
    let message = serde_json::from_str::<ApiErrorResponse>(&body)
        .ok()
        .and_then(|value| value.error.and_then(|error| error.message))
        .unwrap_or(body);
    format!("api error {status}: {message}")
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(API_CONNECT_TIMEOUT)
            .pool_max_idle_per_host(8)
            .pool_idle_timeout(API_POOL_IDLE_TIMEOUT)
            .tcp_keepalive(Some(API_TCP_KEEPALIVE))
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

pub(super) fn flush_utf8_prefix(bytes: &mut Vec<u8>, output: &mut String) -> Result<(), String> {
    match std::str::from_utf8(bytes) {
        Ok(text) => {
            output.push_str(text);
            bytes.clear();
            Ok(())
        }
        Err(error) if error.error_len().is_none() => {
            let valid = error.valid_up_to();
            if valid > 0 {
                let text = std::str::from_utf8(&bytes[..valid])
                    .map_err(|error| format!("api stream utf-8 decode failed: {error}"))?;
                output.push_str(text);
                bytes.drain(..valid);
            }
            Ok(())
        }
        Err(error) => Err(format!("api stream utf-8 decode failed: {error}")),
    }
}

pub(super) fn normalize_newlines(value: &mut String) {
    if value.contains('\r') {
        *value = value.replace("\r\n", "\n").replace('\r', "\n");
    }
}

pub(super) fn next_sse_event(pending: &mut String) -> Option<String> {
    let index = pending.find("\n\n")?;
    let event = pending[..index].to_string();
    pending.drain(..index + 2);
    Some(event)
}
