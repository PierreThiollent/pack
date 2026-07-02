use crate::notifier::{NotificationEvent, NotificationStatus, NotificationTriggers, Notifier};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport, message::Mailbox};
use serde::Deserialize;

/// SMTP transport security mode requested by the mail notifier.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MailEncryption {
    None,
    StartTls,
    Tls,
}

/// Configuration for the `type: mail` notifier.
///
/// It contains SMTP connection settings, optional authentication, the effective
/// sender fallback inputs, recipients, encryption mode, and delivery triggers.
#[derive(Debug, Deserialize)]
pub struct MailConfig {
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    pub username: Option<String>,
    pub password: Option<String>,
    pub from: Option<String>,
    pub to: Vec<String>,

    #[serde(default = "default_encryption")]
    pub encryption: MailEncryption,

    // Shared notification delivery conditions parsed from `on_success` and
    // `on_failure` at the same YAML level as the mail-specific fields.
    #[serde(flatten)]
    pub triggers: NotificationTriggers,
}

fn default_port() -> u16 {
    587
}

fn default_encryption() -> MailEncryption {
    MailEncryption::StartTls
}

impl MailConfig {
    /// Resolve the sender address used in the SMTP envelope and email headers.
    ///
    /// Explicit `from` has priority; `username` is the fallback for SMTP setups
    /// where the authenticated account is also the sender address.
    pub(crate) fn sender(&self) -> Result<&str, String> {
        self.from
            .as_deref()
            .or(self.username.as_deref())
            .ok_or_else(|| "Mail notifier requires either `from` or `username`".to_string())
    }
}

pub(crate) fn build_subject(event: &NotificationEvent) -> String {
    match event.status {
        NotificationStatus::Success => {
            format!("[pack] Backup {} completed successfully", event.model_name)
        }
        NotificationStatus::Failure => format!("[pack] Backup {} failed", event.model_name),
    }
}

pub(crate) fn build_body(event: &NotificationEvent) -> String {
    match event.status {
        NotificationStatus::Success => format!(
            "Backup completed successfully.\n\nModel: {}\nStatus: success",
            event.model_name
        ),
        NotificationStatus::Failure => {
            let error = event.error.as_deref().unwrap_or("Unknown error");
            format!(
                "Backup failed.\n\nModel: {}\nStatus: failure\n\nError:\n{error}",
                event.model_name
            )
        }
    }
}

pub(crate) fn build_transport(config: &MailConfig) -> Result<SmtpTransport, String> {
    let mut builder = match config.encryption {
        MailEncryption::None => SmtpTransport::builder_dangerous(&config.host),
        MailEncryption::StartTls => SmtpTransport::starttls_relay(&config.host)
            .map_err(|error| format!("Failed to configure STARTTLS SMTP transport: {error}"))?,
        MailEncryption::Tls => SmtpTransport::relay(&config.host)
            .map_err(|error| format!("Failed to configure TLS SMTP transport: {error}"))?,
    }
    .port(config.port);

    if let (Some(username), Some(password)) = (&config.username, &config.password) {
        builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
    }

    Ok(builder.build())
}

pub(crate) fn build_message(
    config: &MailConfig,
    event: &NotificationEvent,
) -> Result<Message, String> {
    let sender: Mailbox = config
        .sender()?
        .parse()
        .map_err(|error| format!("Invalid mail sender address: {error}"))?;

    let mut builder = Message::builder()
        .from(sender)
        .subject(build_subject(event))
        .header(ContentType::TEXT_PLAIN);

    for recipient in &config.to {
        let mailbox: Mailbox = recipient
            .parse()
            .map_err(|error| format!("Invalid mail recipient address `{recipient}`: {error}"))?;
        builder = builder.to(mailbox);
    }

    builder
        .body(build_body(event))
        .map_err(|error| format!("Failed to build mail message: {error}"))
}

impl Notifier for MailConfig {
    /// Validate SMTP-specific invariants before a backup run can start.
    ///
    /// This catches unusable mail settings early: missing host, invalid port,
    /// missing recipients, empty optional fields, invalid auth pairing, and
    /// unresolved sender address.
    fn validate(&self, model_name: &str, notifier_name: &str) -> Result<(), String> {
        if self.host.trim().is_empty() {
            return Err(format!(
                "Model `{model_name}` notifier `{notifier_name}` has an empty mail host"
            ));
        }

        if self.port == 0 {
            return Err(format!(
                "Model `{model_name}` notifier `{notifier_name}` has an invalid mail port"
            ));
        }

        validate_optional_field(
            model_name,
            notifier_name,
            "username",
            self.username.as_deref(),
        )?;
        validate_optional_field(
            model_name,
            notifier_name,
            "password",
            self.password.as_deref(),
        )?;
        validate_optional_field(model_name, notifier_name, "from", self.from.as_deref())?;

        match self.encryption {
            MailEncryption::None | MailEncryption::StartTls | MailEncryption::Tls => {}
        }

        if self.to.is_empty() {
            return Err(format!(
                "Model `{model_name}` notifier `{notifier_name}` must define at least one mail recipient"
            ));
        }

        if self.to.iter().any(|recipient| recipient.trim().is_empty()) {
            return Err(format!(
                "Model `{model_name}` notifier `{notifier_name}` has an empty mail recipient"
            ));
        }

        if self.password.is_some() && self.username.is_none() {
            return Err(format!(
                "Model `{model_name}` notifier `{notifier_name}` has a mail password but no username"
            ));
        }

        self.sender().map_err(|error| {
            format!("Model `{model_name}` notifier `{notifier_name}` is invalid: {error}")
        })?;

        Ok(())
    }

    fn triggers(&self) -> &NotificationTriggers {
        &self.triggers
    }

    fn send(&self, event: &NotificationEvent) -> Result<(), String> {
        let message = build_message(self, event)?;
        let transport = build_transport(self)?;

        transport
            .send(&message)
            .map_err(|error| format!("Failed to send mail notification: {error}"))?;

        Ok(())
    }
}

fn validate_optional_field(
    model_name: &str,
    notifier_name: &str,
    field_name: &str,
    value: Option<&str>,
) -> Result<(), String> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        return Err(format!(
            "Model `{model_name}` notifier `{notifier_name}` has an empty mail {field_name}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifier::NotifierConfig;

    #[test]
    fn parse_mail_notifier_with_defaults() {
        let yaml = r#"
type: mail
host: smtp.example.com
to:
  - admin@example.com
"#;

        let notifier: NotifierConfig = serde_yaml::from_str(yaml).unwrap();

        match notifier {
            NotifierConfig::Mail(config) => {
                assert_eq!(config.host, "smtp.example.com");
                assert_eq!(config.port, 587);
                assert_eq!(config.encryption, MailEncryption::StartTls);
                assert!(config.username.is_none());
                assert!(config.password.is_none());
                assert!(config.from.is_none());
                assert_eq!(config.to, vec!["admin@example.com"]);
                assert!(config.triggers.on_success);
                assert!(config.triggers.on_failure);
            }
            NotifierConfig::Discord(_) => panic!("expected mail notifier"),
        }
    }

    #[test]
    fn parse_mail_notifier_with_custom_fields() {
        let yaml = r#"
type: mail
host: smtp.example.com
port: 465
encryption: tls
username: user@example.com
password: secret
from: backup@example.com
to:
  - admin@example.com
  - ops@example.com
on_success: false
on_failure: true
"#;

        let notifier: NotifierConfig = serde_yaml::from_str(yaml).unwrap();

        match notifier {
            NotifierConfig::Mail(config) => {
                assert_eq!(config.host, "smtp.example.com");
                assert_eq!(config.port, 465);
                assert_eq!(config.encryption, MailEncryption::Tls);
                assert_eq!(config.username.as_deref(), Some("user@example.com"));
                assert_eq!(config.password.as_deref(), Some("secret"));
                assert_eq!(config.from.as_deref(), Some("backup@example.com"));
                assert_eq!(config.to, vec!["admin@example.com", "ops@example.com"]);
                assert!(!config.triggers.on_success);
                assert!(config.triggers.on_failure);
            }
            NotifierConfig::Discord(_) => panic!("expected mail notifier"),
        }
    }

    #[test]
    fn parse_mail_notifier_with_no_encryption() {
        let yaml = r#"
type: mail
host: smtp.example.com
from: backup@example.com
to:
  - admin@example.com
encryption: none
"#;

        let notifier: NotifierConfig = serde_yaml::from_str(yaml).unwrap();

        match notifier {
            NotifierConfig::Mail(config) => {
                assert_eq!(config.encryption, MailEncryption::None);
            }
            NotifierConfig::Discord(_) => panic!("expected mail notifier"),
        }
    }

    #[test]
    fn parse_mail_notifier_rejects_unknown_encryption() {
        let yaml = r#"
type: mail
host: smtp.example.com
from: backup@example.com
to:
  - admin@example.com
encryption: sslish
"#;

        let result = serde_yaml::from_str::<NotifierConfig>(yaml);

        assert!(result.is_err());
    }

    #[test]
    fn sender_uses_from_when_present() {
        let mut config = mail_config_with("smtp.example.com", vec!["admin@example.com"]);
        config.username = Some("user@example.com".to_string());
        config.from = Some("backup@example.com".to_string());

        assert_eq!(config.sender().unwrap(), "backup@example.com");
    }

    #[test]
    fn sender_falls_back_to_username() {
        let mut config = mail_config_with("smtp.example.com", vec!["admin@example.com"]);
        config.username = Some("user@example.com".to_string());

        assert_eq!(config.sender().unwrap(), "user@example.com");
    }

    #[test]
    fn validate_rejects_missing_sender() {
        let config = mail_config_with("smtp.example.com", vec!["admin@example.com"]);

        let error = config.validate("my_app", "mail").unwrap_err();

        assert!(error.contains("requires either `from` or `username`"));
    }

    #[test]
    fn validate_rejects_password_without_username() {
        let mut config = mail_config_with("smtp.example.com", vec!["admin@example.com"]);
        config.from = Some("backup@example.com".to_string());
        config.password = Some("secret".to_string());

        let error = config.validate("my_app", "mail").unwrap_err();

        assert!(error.contains("password but no username"));
    }

    #[test]
    fn build_success_subject_and_body() {
        let event = NotificationEvent::success("my_app");

        assert_eq!(
            build_subject(&event),
            "[pack] Backup my_app completed successfully"
        );
        assert_eq!(
            build_body(&event),
            "Backup completed successfully.\n\nModel: my_app\nStatus: success"
        );
    }

    #[test]
    fn build_failure_subject_and_body() {
        let event = NotificationEvent::failure("my_app", "Failed to upload backup");

        assert_eq!(build_subject(&event), "[pack] Backup my_app failed");
        assert_eq!(
            build_body(&event),
            "Backup failed.\n\nModel: my_app\nStatus: failure\n\nError:\nFailed to upload backup"
        );
    }

    #[test]
    fn build_transport_accepts_starttls_config() {
        let mut config = mail_config_with("smtp.example.com", vec!["admin@example.com"]);
        config.username = Some("user@example.com".to_string());
        config.password = Some("secret".to_string());

        let result = build_transport(&config);

        assert!(result.is_ok());
    }

    #[test]
    fn build_transport_accepts_tls_config() {
        let mut config = mail_config_with("smtp.example.com", vec!["admin@example.com"]);
        config.encryption = MailEncryption::Tls;
        config.port = 465;

        let result = build_transport(&config);

        assert!(result.is_ok());
    }

    #[test]
    fn build_transport_accepts_unencrypted_config() {
        let mut config = mail_config_with("localhost", vec!["admin@example.com"]);
        config.encryption = MailEncryption::None;
        config.port = 25;

        let result = build_transport(&config);

        assert!(result.is_ok());
    }

    #[test]
    fn build_message_uses_sender_recipients_subject_and_body() {
        let mut config = mail_config_with(
            "smtp.example.com",
            vec!["admin@example.com", "ops@example.com"],
        );
        config.from = Some("backup@example.com".to_string());
        let event = NotificationEvent::success("my_app");

        let message = build_message(&config, &event).unwrap();
        let formatted = String::from_utf8(message.formatted()).unwrap();

        assert!(formatted.contains("From: backup@example.com"));
        assert!(formatted.contains("To: admin@example.com, ops@example.com"));
        assert!(formatted.contains("Subject: [pack] Backup my_app completed successfully"));
        assert!(formatted.contains("Content-Type: text/plain"));
        assert!(formatted.contains("Backup completed successfully."));
        assert!(formatted.contains("Model: my_app"));
        assert!(formatted.contains("Status: success"));
    }

    #[test]
    fn build_message_rejects_invalid_sender() {
        let mut config = mail_config_with("smtp.example.com", vec!["admin@example.com"]);
        config.from = Some("not an email".to_string());
        let event = NotificationEvent::success("my_app");

        let error = build_message(&config, &event).unwrap_err();

        assert!(error.contains("Invalid mail sender address"));
    }

    #[test]
    fn build_message_rejects_invalid_recipient() {
        let mut config = mail_config_with("smtp.example.com", vec!["not an email"]);
        config.from = Some("backup@example.com".to_string());
        let event = NotificationEvent::success("my_app");

        let error = build_message(&config, &event).unwrap_err();

        assert!(error.contains("Invalid mail recipient address"));
    }

    #[test]
    fn validate_rejects_empty_mail_host() {
        let config = mail_config_with("", vec!["admin@example.com"]);

        let error = config.validate("my_app", "mail").unwrap_err();

        assert!(error.contains("empty mail host"));
    }

    #[test]
    fn validate_rejects_empty_mail_recipients() {
        let config = mail_config_with("smtp.example.com", vec![]);

        let error = config.validate("my_app", "mail").unwrap_err();

        assert!(error.contains("at least one mail recipient"));
    }

    #[test]
    fn validate_rejects_blank_mail_recipient() {
        let config = mail_config_with("smtp.example.com", vec![""]);

        let error = config.validate("my_app", "mail").unwrap_err();

        assert!(error.contains("empty mail recipient"));
    }

    fn mail_config_with(host: &str, to: Vec<&str>) -> MailConfig {
        MailConfig {
            host: host.to_string(),
            port: 587,
            username: None,
            password: None,
            from: None,
            to: to.into_iter().map(str::to_string).collect(),
            encryption: MailEncryption::StartTls,
            triggers: NotificationTriggers {
                on_success: true,
                on_failure: true,
            },
        }
    }
}
