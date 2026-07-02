use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;

const HTTP_TIMEOUT_SECONDS: u64 = 30;

/// Send a JSON payload to an HTTP webhook endpoint.
///
/// Centralizes timeout, request execution, status checking, and error-body
/// extraction for webhook-based notifiers.
pub fn post_json(url: &str, payload: &Value, notifier_name: &str) -> Result<(), String> {
    let client = build_http_client()?;

    let response = client
        .post(url)
        .json(payload)
        .send()
        .map_err(|error| format!("Failed to send {notifier_name} notification: {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_else(|error| {
            format!("Failed to read {notifier_name} error response body: {error}")
        });

        return Err(format!(
            "{notifier_name} notification failed with HTTP status {status}: {body}"
        ));
    }

    Ok(())
}

fn build_http_client() -> Result<Client, String> {
    // Keep HTTP client construction local to webhook delivery so non-HTTP
    // notifiers do not depend on reqwest plumbing.
    Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| format!("Failed to create notification HTTP client: {error}"))
}
