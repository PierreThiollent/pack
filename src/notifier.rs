pub mod discord;

use crate::config::Model;
use crate::logging::{LogTag, tag};
use crate::notifier::discord::DiscordConfig;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, warn};

const HTTP_TIMEOUT_SECONDS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationStatus {
    Success,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationEvent {
    pub model_name: String,
    pub status: NotificationStatus,
    pub error: Option<String>,
}

impl NotificationEvent {
    pub fn success(model_name: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            status: NotificationStatus::Success,
            error: None,
        }
    }

    pub fn failure(model_name: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            status: NotificationStatus::Failure,
            error: Some(error.into()),
        }
    }
}

/// Configuration for a notifier — the `type` field determines which variant is used.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum NotifierConfig {
    #[serde(rename = "discord")]
    Discord(DiscordConfig),
}

pub(crate) fn default_on_success() -> bool {
    true
}

pub(crate) fn default_on_failure() -> bool {
    true
}

pub fn notify_model(model: &Model, event: &NotificationEvent) {
    let notifiers_to_run: Vec<_> = model
        .notifiers
        .iter()
        .filter(|(_, notifier_config)| should_notify(notifier_config, event))
        .collect();

    if notifiers_to_run.is_empty() {
        return;
    }

    let http_client = match Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            warn!(
                pack_tag = %tag(LogTag::Run),
                "Failed to create notification HTTP client: {error}"
            );
            return;
        }
    };

    for (notifier_name, notifier_config) in notifiers_to_run {
        info!(
            pack_tag = %tag(LogTag::Notifier(notifier_name)),
            "Sending notification"
        );

        if let Err(error) = send_one(&http_client, notifier_config, event) {
            warn!(
                pack_tag = %tag(LogTag::Notifier(notifier_name)),
                "Failed to send notification: {error}"
            );
        }
    }
}

fn should_notify(notifier_config: &NotifierConfig, event: &NotificationEvent) -> bool {
    match (notifier_config, &event.status) {
        (NotifierConfig::Discord(config), NotificationStatus::Success) => config.on_success,
        (NotifierConfig::Discord(config), NotificationStatus::Failure) => config.on_failure,
    }
}

fn send_one(
    http_client: &Client,
    notifier_config: &NotifierConfig,
    event: &NotificationEvent,
) -> Result<(), String> {
    match notifier_config {
        NotifierConfig::Discord(config) => discord::send(http_client, config, event),
    }
}
