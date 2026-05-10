use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiscovery {
    pub port: u16,
    pub token: String,
}

pub fn app_data_dir() -> PathBuf {
    if let Ok(path) = std::env::var("DBX_APP_DATA_DIR") {
        return PathBuf::from(path);
    }

    let home = std::env::var(if cfg!(windows) { "APPDATA" } else { "HOME" })
        .unwrap_or_else(|_| ".".to_string());

    if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library/Application Support/com.dbx.app")
    } else if cfg!(windows) {
        PathBuf::from(home).join("com.dbx.app")
    } else {
        PathBuf::from(home).join(".config/com.dbx.app")
    }
}

pub fn load_runtime() -> Option<RuntimeDiscovery> {
    let path = app_data_dir().join("agent-runtime.json");
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

pub async fn get_json(path: &str) -> Result<serde_json::Value, String> {
    let runtime = load_runtime().ok_or_else(|| "runtime unavailable".to_string())?;
    let url = format!("http://127.0.0.1:{}{}", runtime.port, path);

    let response = reqwest::Client::new()
        .get(url)
        .bearer_auth(runtime.token)
        .send()
        .await
        .map_err(|err| err.to_string())?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("runtime request failed with status {status}"));
    }

    response
        .json()
        .await
        .map_err(|err| err.to_string())
}

pub async fn post_json(path: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
    let runtime = load_runtime().ok_or_else(|| "runtime unavailable".to_string())?;
    let url = format!("http://127.0.0.1:{}{}", runtime.port, path);

    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(runtime.token)
        .json(&body)
        .send()
        .await
        .map_err(|err| err.to_string())?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("runtime request failed with status {status}"));
    }

    response
        .json()
        .await
        .map_err(|err| err.to_string())
}
