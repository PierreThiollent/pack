use crate::notifier::webhook;
use crate::notifier::{NotificationEvent, NotificationStatus, NotificationTriggers, Notifier};
use serde::Deserialize;

/// Configuration for the `type: discord` notifier.
#[derive(Debug, Deserialize)]
pub struct DiscordConfig {
    pub url: String,

    // Shared delivery conditions parsed from `on_success` and `on_failure`.
    #[serde(flatten)]
    pub triggers: NotificationTriggers,
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

impl Notifier for DiscordConfig {
    /// Validate Discord webhook settings before attempting delivery.
    fn validate(&self, model_name: &str, notifier_name: &str) -> Result<(), String> {
        if self.url.trim().is_empty() {
            return Err(format!(
                "Model `{model_name}` notifier `{notifier_name}` has an empty Discord URL"
            ));
        }

        Ok(())
    }

    fn triggers(&self) -> &NotificationTriggers {
        &self.triggers
    }

    fn send(&self, event: &NotificationEvent) -> Result<(), String> {
        // Discord delivery is a JSON POST to the configured webhook URL.
        webhook::post_json(&self.url, &build_payload(event), "Discord")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifier::{NotificationEvent, NotifierConfig};

    #[test]
    fn parse_discord_notifier_with_defaults() {
        let yaml = r#"
type: discord
url: https://discord.com/api/webhooks/example
"#;

        let notifier: NotifierConfig = serde_yaml::from_str(yaml).unwrap();

        match notifier {
            NotifierConfig::Discord(config) => {
                assert_eq!(config.url, "https://discord.com/api/webhooks/example");
                assert!(config.triggers.on_success);
                assert!(config.triggers.on_failure);
            }
            NotifierConfig::Mail(_) => panic!("expected Discord notifier"),
        }
    }

    #[test]
    fn parse_discord_notifier_with_custom_events() {
        let yaml = r#"
type: discord
url: https://discord.com/api/webhooks/example
on_success: false
on_failure: true
"#;

        let notifier: NotifierConfig = serde_yaml::from_str(yaml).unwrap();

        match notifier {
            NotifierConfig::Discord(config) => {
                assert!(!config.triggers.on_success);
                assert!(config.triggers.on_failure);
            }
            NotifierConfig::Mail(_) => panic!("expected Discord notifier"),
        }
    }

    #[test]
    fn validate_rejects_empty_discord_url() {
        let config = DiscordConfig {
            url: "".to_string(),
            triggers: NotificationTriggers {
                on_success: true,
                on_failure: true,
            },
        };

        let error = config.validate("my_app", "discord").unwrap_err();

        assert!(error.contains("empty Discord URL"));
    }

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
