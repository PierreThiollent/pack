use crate::notifier::{NotificationEvent, NotificationStatus};
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DiscordConfig {
    pub url: String,

    #[serde(default = "crate::notifier::default_on_success")]
    pub on_success: bool,

    #[serde(default = "crate::notifier::default_on_failure")]
    pub on_failure: bool,
}

pub(crate) fn build_message(event: &NotificationEvent) -> String {
    match event.status {
        NotificationStatus::Success => {
            format!("✅ Backup `{}` completed successfully", event.model_name)
        }
        NotificationStatus::Failure => {
            let error = event.error.as_deref().unwrap_or("Unknown error");
            format!("⛔️ Backup `{}` failed\n\n{}", event.model_name, error)
        }
    }
}

pub(crate) fn build_payload(event: &NotificationEvent) -> serde_json::Value {
    serde_json::json!({
        "content": build_message(event),
    })
}

pub fn send(
    client: &Client,
    config: &DiscordConfig,
    event: &NotificationEvent,
) -> Result<(), String> {
    let response = client
        .post(&config.url)
        .json(&build_payload(event))
        .send()
        .map_err(|error| format!("Failed to send Discord notification: {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .unwrap_or_else(|error| format!("Failed to read Discord error response body: {error}"));

        return Err(format!(
            "Discord notification failed with HTTP status {status}: {body}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifier::NotificationEvent;

    #[test]
    fn build_success_message() {
        let event = NotificationEvent::success("my_app");

        assert_eq!(
            build_message(&event),
            "✅ Backup `my_app` completed successfully"
        );
    }

    #[test]
    fn build_failure_message() {
        let event = NotificationEvent::failure("my_app", "Failed to upload backup");

        assert_eq!(
            build_message(&event),
            "⛔️ Backup `my_app` failed\n\nFailed to upload backup"
        );
    }

    #[test]
    fn build_discord_payload() {
        let event = NotificationEvent::success("my_app");

        assert_eq!(
            build_payload(&event),
            serde_json::json!({
                "content": "✅ Backup `my_app` completed successfully",
            })
        );
    }
}
