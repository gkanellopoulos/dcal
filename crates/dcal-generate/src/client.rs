use std::env;
use std::future::Future;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("ANTHROPIC_API_KEY not set")]
    MissingApiKey,

    #[error("API request failed: {0}")]
    Request(String),

    #[error("API returned error: {status} — {message}")]
    Response { status: u16, message: String },

    #[error("failed to parse API response: {0}")]
    Parse(String),
}

/// A message in the Anthropic Messages API format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Request to the Anthropic Messages API.
#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
}

/// Response from the Anthropic Messages API.
#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub content: String,
}

/// Trait for Anthropic API clients.
///
/// Generic over implementations so tests can use a mock without
/// dynamic dispatch or async-trait.
pub trait AnthropicClient {
    fn send(&self, request: ApiRequest) -> impl Future<Output = Result<ApiResponse, ApiError>> + Send;
}

/// Real API client using reqwest.
pub struct ReqwestClient {
    api_key: String,
    http: reqwest::Client,
}

impl ReqwestClient {
    /// Create a client with an explicit API key.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            http: reqwest::Client::new(),
        }
    }

    /// Create a client, reading the API key from the environment.
    pub fn from_env() -> Result<Self, ApiError> {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .map_err(|_| ApiError::MissingApiKey)?;

        Ok(Self {
            api_key,
            http: reqwest::Client::new(),
        })
    }
}

impl AnthropicClient for ReqwestClient {
    async fn send(&self, request: ApiRequest) -> Result<ApiResponse, ApiError> {
        let mut body = serde_json::json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "messages": request.messages,
        });

        if let Some(system) = &request.system {
            body["system"] = serde_json::json!(system);
        }

        let resp = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Request(e.to_string()))?;

        let status = resp.status().as_u16();

        if status != 200 {
            let text = resp.text().await.unwrap_or_default();
            return Err(ApiError::Response {
                status,
                message: text,
            });
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ApiError::Parse(e.to_string()))?;

        let content = json["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|block| block["text"].as_str())
            .ok_or_else(|| ApiError::Parse("no text content in response".to_string()))?
            .to_string();

        Ok(ApiResponse { content })
    }
}

/// Mock API client for testing.
#[cfg(test)]
pub mod mock {
    use super::*;

    pub struct MockClient {
        responses: std::sync::Mutex<Vec<Result<String, ApiError>>>,
    }

    impl MockClient {
        pub fn new(responses: Vec<Result<String, ApiError>>) -> Self {
            Self {
                responses: std::sync::Mutex::new(responses),
            }
        }

        pub fn with_response(content: &str) -> Self {
            Self::new(vec![Ok(content.to_string())])
        }
    }

    impl AnthropicClient for MockClient {
        async fn send(&self, _request: ApiRequest) -> Result<ApiResponse, ApiError> {
            let mut responses = self.responses.lock().unwrap();
            let result = if responses.is_empty() {
                Err(ApiError::Request("no more mock responses".to_string()))
            } else {
                responses.remove(0)
            };

            match result {
                Ok(content) => Ok(ApiResponse { content }),
                Err(e) => Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::mock::MockClient;

    #[tokio::test]
    async fn mock_client_returns_response() {
        let client = MockClient::with_response("hello world");
        let req = ApiRequest {
            model: "claude-haiku-4-5".to_string(),
            system: None,
            messages: vec![Message {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
            max_tokens: 100,
        };

        let resp = client.send(req).await.unwrap();
        assert_eq!(resp.content, "hello world");
    }

    #[tokio::test]
    async fn mock_client_returns_multiple_responses() {
        let client = MockClient::new(vec![
            Ok("first".to_string()),
            Ok("second".to_string()),
        ]);

        let req = ApiRequest {
            model: "test".to_string(),
            system: None,
            messages: vec![Message {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
            max_tokens: 100,
        };

        let r1 = client.send(req.clone()).await.unwrap();
        assert_eq!(r1.content, "first");

        let r2 = client.send(req).await.unwrap();
        assert_eq!(r2.content, "second");
    }

    #[tokio::test]
    async fn mock_client_returns_error() {
        let client = MockClient::new(vec![
            Err(ApiError::Response {
                status: 429,
                message: "rate limited".to_string(),
            }),
        ]);

        let req = ApiRequest {
            model: "test".to_string(),
            system: None,
            messages: vec![],
            max_tokens: 100,
        };

        let result = client.send(req).await;
        assert!(result.is_err());
    }
}
