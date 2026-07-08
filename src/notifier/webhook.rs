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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn post_json_sends_payload_to_http_endpoint() {
        let (url, handle) = spawn_one_request_server("HTTP/1.1 204 No Content\r\n\r\n");
        let payload = serde_json::json!({ "content": "hello" });

        post_json(&url, &payload, "Test").unwrap();

        let request = handle.join().unwrap();
        assert!(request.starts_with("POST / HTTP/1.1"));
        assert!(request.contains("application/json"));
        assert!(request.contains(r#"{"content":"hello"}"#));
    }

    #[test]
    fn post_json_returns_http_error_body() {
        let (url, handle) = spawn_one_request_server(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 6\r\n\r\nbroken",
        );
        let payload = serde_json::json!({ "content": "hello" });

        let error = post_json(&url, &payload, "Test").unwrap_err();

        handle.join().unwrap();
        assert!(error.contains("HTTP status 500 Internal Server Error"));
        assert!(error.contains("broken"));
    }

    fn spawn_one_request_server(response: &'static str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 8192];
            let read_count = stream.read(&mut buffer).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&buffer[..read_count]).into_owned()
        });

        (format!("http://{address}"), handle)
    }
}
