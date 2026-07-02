use crate::notifier::webhook;
use crate::notifier::{NotificationEvent, NotificationStatus, NotificationTriggers, Notifier};
use serde::Deserialize;

/// Configuration for the `type: slack` notifier.
#[derive(Debug, Deserialize)]
pub struct SlackConfig {
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
        "text": build_message(event),
    })
}

impl Notifier for SlackConfig {
    /// Validate Slack webhook settings before attempting delivery.
    fn validate(&self, model_name: &str, notifier_name: &str) -> Result<(), String> {
        if self.url.trim().is_empty() {
            return Err(format!(
                "Model `{model_name}` notifier `{notifier_name}` has an empty Slack URL"
            ));
        }

        Ok(())
    }

    fn triggers(&self) -> &NotificationTriggers {
        &self.triggers
    }

    fn send(&self, event: &NotificationEvent) -> Result<(), String> {
        // Slack incoming webhooks accept a JSON POST with a top-level `text` field.
        webhook::post_json(&self.url, &build_payload(event), "Slack")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifier::NotifierConfig;

    #[test]
    fn parse_slack_notifier_with_defaults() {
        let yaml = r#"
type: slack
url: https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX
"#;

        let notifier: NotifierConfig = serde_yaml::from_str(yaml).unwrap();

        match notifier {
            NotifierConfig::Slack(config) => {
                assert_eq!(
                    config.url,
                    "https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX"
                );
                assert!(config.triggers.on_success);
                assert!(config.triggers.on_failure);
            }
            _ => panic!("expected Slack notifier"),
        }
    }

    #[test]
    fn parse_slack_notifier_with_custom_events() {
        let yaml = r#"
type: slack
url: https://hooks.slack.com/services/example
on_success: false
on_failure: true
"#;

        let notifier: NotifierConfig = serde_yaml::from_str(yaml).unwrap();

        match notifier {
            NotifierConfig::Slack(config) => {
                assert!(!config.triggers.on_success);
                assert!(config.triggers.on_failure);
            }
            _ => panic!("expected Slack notifier"),
        }
    }

    #[test]
    fn validate_rejects_empty_slack_url() {
        let config = SlackConfig {
            url: "".to_string(),
            triggers: NotificationTriggers {
                on_success: true,
                on_failure: true,
            },
        };

        let error = config.validate("my_app", "slack").unwrap_err();

        assert!(error.contains("empty Slack URL"));
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
    fn build_slack_payload() {
        let event = NotificationEvent::success("my_app");

        assert_eq!(
            build_payload(&event),
            serde_json::json!({
                "text": "✅ Backup `my_app` completed successfully",
            })
        );
    }
}
