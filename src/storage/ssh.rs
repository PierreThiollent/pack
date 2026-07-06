use crate::logging::{LogTag, tag};
use crate::paths::expand_tilde;
use crate::storage::{remote_address, socket_address, timeout_duration};
use ssh2::Session;
use std::net::TcpStream;
use std::path::PathBuf;
use tracing::info;

/// SSH settings shared by SSH-based storages such as SFTP and SCP.
pub(crate) struct SshConnectionConfig<'a> {
    pub storage_name: &'static str,
    pub log_tag: LogTag<'a>,
    pub host: &'a str,
    pub port: u16,
    pub timeout: u64,
    pub username: &'a str,
    pub password: Option<&'a str>,
    pub private_key: Option<&'a str>,
    pub passphrase: Option<&'a str>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SshAuthMethod {
    Password,
    PrivateKey,
}

/// Validate common SSH settings before opening any network connection.
pub(crate) fn validate_config(config: &SshConnectionConfig<'_>) -> Result<(), String> {
    if config.host.trim().is_empty() || config.username.trim().is_empty() {
        return Err(format!(
            "{} host or username cannot be empty",
            config.storage_name
        ));
    }

    if config.password.is_none() && config.private_key.is_none() {
        return Err(format!(
            "{} password or private_key is required",
            config.storage_name
        ));
    }

    if config
        .private_key
        .is_some_and(|private_key| private_key.trim().is_empty())
    {
        return Err(format!(
            "{} private_key cannot be empty",
            config.storage_name
        ));
    }

    if config.passphrase.is_some() && config.private_key.is_none() {
        return Err(format!(
            "{} passphrase requires private_key authentication",
            config.storage_name
        ));
    }

    Ok(())
}

/// Open the TCP connection and perform the SSH handshake.
pub(crate) fn connect(config: &SshConnectionConfig<'_>) -> Result<Session, String> {
    let tcp_stream = open_tcp_connection(config)?;
    let mut session = Session::new().map_err(|error| {
        format!(
            "Failed to create {} SSH session: {error}",
            config.storage_name
        )
    })?;
    session.set_tcp_stream(tcp_stream);
    session.handshake().map_err(|error| {
        format!(
            "Failed to start {} SSH session: {error}",
            config.storage_name
        )
    })?;

    info!(pack_tag = %tag(config.log_tag), "SSH session established");
    Ok(session)
}

/// Authenticate the SSH session with the configured auth method.
pub(crate) fn authenticate(
    config: &SshConnectionConfig<'_>,
    session: &mut Session,
) -> Result<(), String> {
    match auth_method(config)? {
        SshAuthMethod::Password => authenticate_with_password(config, session),
        SshAuthMethod::PrivateKey => authenticate_with_private_key(config, session),
    }
}

/// Choose the authentication method from the parsed config.
pub(crate) fn auth_method(config: &SshConnectionConfig<'_>) -> Result<SshAuthMethod, String> {
    if config.password.is_some() {
        return Ok(SshAuthMethod::Password);
    }

    if config.private_key.is_some() {
        return Ok(SshAuthMethod::PrivateKey);
    }

    Err(format!(
        "{} password or private_key is required",
        config.storage_name
    ))
}

/// Return the configured private key path with a leading `~` expanded.
pub(crate) fn private_key_path(config: &SshConnectionConfig<'_>) -> Result<PathBuf, String> {
    let private_key = config.private_key.ok_or_else(|| {
        format!(
            "{} private_key is required for private_key authentication",
            config.storage_name
        )
    })?;

    Ok(PathBuf::from(expand_tilde(private_key)))
}

fn open_tcp_connection(config: &SshConnectionConfig<'_>) -> Result<TcpStream, String> {
    info!(
        pack_tag = %tag(config.log_tag),
        "Connecting to {}",
        remote_address(config.host, config.port)
    );

    let tcp_stream = TcpStream::connect_timeout(
        &socket_address(config.host, config.port, config.storage_name)?,
        timeout_duration(config.timeout),
    )
    .map_err(|error| {
        format!(
            "Failed to connect to {} server: {error}",
            config.storage_name
        )
    })?;
    tcp_stream.set_nodelay(true).map_err(|error| {
        format!(
            "Failed to configure {} TCP connection: {error}",
            config.storage_name
        )
    })?;
    Ok(tcp_stream)
}

fn authenticate_with_password(
    config: &SshConnectionConfig<'_>,
    session: &mut Session,
) -> Result<(), String> {
    let password = config.password.ok_or_else(|| {
        format!(
            "{} password is required for password authentication",
            config.storage_name
        )
    })?;

    session
        .userauth_password(config.username, password)
        .map_err(|error| {
            format!(
                "Failed to authenticate to {} server with password: {error}",
                config.storage_name
            )
        })?;

    if session.authenticated() {
        info!(pack_tag = %tag(config.log_tag), "Authenticated with password");
        return Ok(());
    }

    Err(format!(
        "Failed to authenticate to {} server with password",
        config.storage_name
    ))
}

fn authenticate_with_private_key(
    config: &SshConnectionConfig<'_>,
    session: &mut Session,
) -> Result<(), String> {
    let private_key_path = private_key_path(config)?;
    if !private_key_path.is_file() {
        return Err(format!(
            "{} private_key file does not exist or is not a file: {}",
            config.storage_name,
            private_key_path.display()
        ));
    }

    session
        .userauth_pubkey_file(config.username, None, &private_key_path, config.passphrase)
        .map_err(|error| {
            format!(
                "Failed to authenticate to {} server with private_key {}: {error}",
                config.storage_name,
                private_key_path.display()
            )
        })?;

    if session.authenticated() {
        info!(
            pack_tag = %tag(config.log_tag),
            "Authenticated with private_key"
        );
        return Ok(());
    }

    Err(format!(
        "Failed to authenticate to {} server with private_key {}",
        config.storage_name,
        private_key_path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> SshConnectionConfig<'static> {
        SshConnectionConfig {
            storage_name: "SFTP",
            log_tag: LogTag::Sftp,
            host: "sftp.example.com",
            port: 22,
            timeout: 300,
            username: "user",
            password: Some("secret"),
            private_key: None,
            passphrase: None,
        }
    }

    #[test]
    fn auth_method_uses_password_when_password_is_configured() {
        let config = valid_config();

        assert_eq!(auth_method(&config).unwrap(), SshAuthMethod::Password);
    }

    #[test]
    fn auth_method_uses_private_key_when_password_is_missing() {
        let mut config = valid_config();
        config.password = None;
        config.private_key = Some("~/.ssh/id_rsa");

        assert_eq!(auth_method(&config).unwrap(), SshAuthMethod::PrivateKey);
    }

    #[test]
    fn auth_method_prefers_password_when_both_are_configured() {
        let mut config = valid_config();
        config.private_key = Some("~/.ssh/id_rsa");

        assert_eq!(auth_method(&config).unwrap(), SshAuthMethod::Password);
    }

    #[test]
    fn validate_config_accepts_password_auth() {
        assert!(validate_config(&valid_config()).is_ok());
    }

    #[test]
    fn validate_config_accepts_private_key_auth() {
        let mut config = valid_config();
        config.password = None;
        config.private_key = Some("~/.ssh/id_rsa");

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn validate_config_rejects_empty_required_fields() {
        let mut config = valid_config();
        config.host = "";

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn validate_config_rejects_missing_auth_method() {
        let mut config = valid_config();
        config.password = None;
        config.private_key = None;

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn validate_config_rejects_empty_private_key() {
        let mut config = valid_config();
        config.password = None;
        config.private_key = Some("  ");

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn validate_config_rejects_passphrase_without_private_key() {
        let mut config = valid_config();
        config.passphrase = Some("secret");

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn private_key_path_expands_tilde() {
        let mut config = valid_config();
        config.password = None;
        config.private_key = Some("~/.ssh/id_rsa");
        let home = std::env::var("HOME").unwrap();

        assert_eq!(
            private_key_path(&config).unwrap(),
            PathBuf::from(format!("{home}/.ssh/id_rsa"))
        );
    }

    #[test]
    fn private_key_path_keeps_non_tilde_path() {
        let mut config = valid_config();
        config.password = None;
        config.private_key = Some("/tmp/id_rsa");

        assert_eq!(
            private_key_path(&config).unwrap(),
            PathBuf::from("/tmp/id_rsa")
        );
    }
}
