pub mod discord;
pub mod mail;
pub mod slack;
pub mod webhook;

use crate::config::Model;
use crate::logging::{LogTag, tag};
use crate::notifier::discord::DiscordConfig;
use crate::notifier::mail::MailConfig;
use crate::notifier::slack::SlackConfig;
use serde::Deserialize;
use tracing::{info, warn};

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

/// Delivery conditions shared by all notifier types.
///
/// These fields decide whether a notifier is executed after a successful or
/// failed backup run.
#[derive(Debug, Deserialize)]
pub struct NotificationTriggers {
    #[serde(default = "default_on_success")]
    pub on_success: bool,

    #[serde(default = "default_on_failure")]
    pub on_failure: bool,
}

/// Configuration for a notifier — the `type` field determines which variant is used.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum NotifierConfig {
    #[serde(rename = "discord")]
    Discord(DiscordConfig),

    #[serde(rename = "mail")]
    Mail(MailConfig),

    #[serde(rename = "slack")]
    Slack(SlackConfig),
}

pub(crate) fn default_on_success() -> bool {
    true
}

pub(crate) fn default_on_failure() -> bool {
    true
}

/// Common behavior required from every notifier backend.
///
/// Each backend owns its validation and delivery mechanism, while the shared
/// default method keeps success/failure filtering consistent for all notifiers.
pub(crate) trait Notifier {
    fn validate(&self, model_name: &str, notifier_name: &str) -> Result<(), String>;
    fn triggers(&self) -> &NotificationTriggers;
    fn send(&self, event: &NotificationEvent) -> Result<(), String>;

    fn should_notify(&self, event: &NotificationEvent) -> bool {
        match event.status {
            NotificationStatus::Success => self.triggers().on_success,
            NotificationStatus::Failure => self.triggers().on_failure,
        }
    }
}

impl NotifierConfig {
    /// Return this enum variant as the common notifier trait object.
    ///
    /// New notifier types should only need to extend this dispatch point, then
    /// implement `Notifier` in their own module.
    fn as_dyn_notifier(&self) -> &dyn Notifier {
        match self {
            NotifierConfig::Discord(config) => config,
            NotifierConfig::Mail(config) => config,
            NotifierConfig::Slack(config) => config,
        }
    }

    pub(crate) fn validate(&self, model_name: &str, notifier_name: &str) -> Result<(), String> {
        self.as_dyn_notifier().validate(model_name, notifier_name)
    }

    fn should_notify(&self, event: &NotificationEvent) -> bool {
        self.as_dyn_notifier().should_notify(event)
    }

    fn send(&self, event: &NotificationEvent) -> Result<(), String> {
        self.as_dyn_notifier().send(event)
    }
}

/// Execute the notifiers enabled for this model and event.
///
/// Notification failures are deliberately non-fatal: the backup result remains
/// the source of truth, and notification errors are reported as warnings.
pub fn notify_model(model: &Model, event: &NotificationEvent) {
    let notifiers_to_run: Vec<_> = model
        .notifiers
        .iter()
        .filter(|(_, notifier_config)| notifier_config.should_notify(event))
        .collect();

    if notifiers_to_run.is_empty() {
        return;
    }

    for (notifier_name, notifier_config) in notifiers_to_run {
        info!(
            pack_tag = %tag(LogTag::Notifier(notifier_name)),
            "Sending notification"
        );

        if let Err(error) = notifier_config.send(event) {
            warn!(
                pack_tag = %tag(LogTag::Notifier(notifier_name)),
                "Failed to send notification: {error}"
            );
        }
    }
}
