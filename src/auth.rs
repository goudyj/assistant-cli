use keyring::Entry;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

const SERVICE_NAME: &str = "assistant-cli";
const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

#[derive(Debug)]
pub enum AuthError {
    NoClientId,
    DeviceFlowError(String),
    TokenExpired,
    KeyringError(String),
    NetworkError(String),
    UserDenied,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::NoClientId => write!(f, "No GitHub OAuth client_id configured"),
            AuthError::DeviceFlowError(msg) => write!(f, "Device flow error: {}", msg),
            AuthError::TokenExpired => write!(f, "Authorization expired, please try again"),
            AuthError::KeyringError(msg) => write!(f, "Keyring error: {}", msg),
            AuthError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            AuthError::UserDenied => write!(f, "Authorization denied by user"),
        }
    }
}

impl std::error::Error for AuthError {}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

pub struct DeviceFlowAuth {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub client_id: String,
    interval: u64,
    expires_in: u64,
}

impl DeviceFlowAuth {
    pub async fn start(client_id: &str) -> Result<Self, AuthError> {
        let client = Client::new();

        let response = client
            .post(GITHUB_DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .form(&[("client_id", client_id), ("scope", "repo")])
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(AuthError::DeviceFlowError(text));
        }

        let data: DeviceCodeResponse = response
            .json()
            .await
            .map_err(|e| AuthError::DeviceFlowError(e.to_string()))?;

        Ok(Self {
            user_code: data.user_code,
            verification_uri: data.verification_uri,
            device_code: data.device_code,
            client_id: client_id.to_string(),
            interval: data.interval,
            expires_in: data.expires_in,
        })
    }

    pub fn open_browser(&self) -> Result<(), AuthError> {
        open::that(&self.verification_uri).map_err(|e| AuthError::DeviceFlowError(e.to_string()))
    }

    pub async fn poll_for_token(&self) -> Result<String, AuthError> {
        let client = Client::new();
        let max_attempts = self.expires_in / self.interval;

        for _ in 0..max_attempts {
            tokio::time::sleep(Duration::from_secs(self.interval)).await;

            let response = client
                .post(GITHUB_ACCESS_TOKEN_URL)
                .header("Accept", "application/json")
                .form(&[
                    ("client_id", self.client_id.as_str()),
                    ("device_code", self.device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .map_err(|e| AuthError::NetworkError(e.to_string()))?;

            let response_text = response
                .text()
                .await
                .map_err(|e| AuthError::DeviceFlowError(e.to_string()))?;

            let data: AccessTokenResponse = serde_json::from_str(&response_text)
                .map_err(|e| AuthError::DeviceFlowError(format!("Parse error: {} - Response: {}", e, response_text)))?;

            if let Some(token) = data.access_token {
                return Ok(token);
            }

            if let Some(error) = data.error {
                match error.as_str() {
                    "authorization_pending" => continue,
                    "slow_down" => {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                    "expired_token" => return Err(AuthError::TokenExpired),
                    "access_denied" => return Err(AuthError::UserDenied),
                    _ => return Err(AuthError::DeviceFlowError(error)),
                }
            }

            // No token and no error - unexpected response
            return Err(AuthError::DeviceFlowError(format!("Unexpected response: {}", response_text)));
        }

        Err(AuthError::TokenExpired)
    }
}

pub fn get_stored_token() -> Result<String, AuthError> {
    let entry = Entry::new(SERVICE_NAME, "github_token")
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    entry
        .get_password()
        .map_err(|e| AuthError::KeyringError(e.to_string()))
}

pub fn store_token(token: &str) -> Result<(), AuthError> {
    let entry = Entry::new(SERVICE_NAME, "github_token")
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    // Delete existing entry if present (ignore errors)
    let _ = entry.delete_credential();

    entry
        .set_password(token)
        .map_err(|e| AuthError::KeyringError(e.to_string()))
}

pub fn delete_token() -> Result<(), AuthError> {
    let entry = Entry::new(SERVICE_NAME, "github_token")
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    entry
        .delete_credential()
        .map_err(|e| AuthError::KeyringError(e.to_string()))
}

pub fn is_logged_in() -> bool {
    get_stored_token().is_ok()
}
