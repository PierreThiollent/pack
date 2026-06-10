use crate::config::MySQLConfig;
use std::process::Command;

/// MySQL dump using mysqldump subprocess.
///
/// Builds a command like:
///   mysqldump --host localhost --port 3306 -u root -psecret my_database --result-file=/tmp/dump.sql
pub struct MySQL<'a> {
    config: &'a MySQLConfig,
    dump_path: String,
}

impl<'a> MySQL<'a> {
    /// Create a new MySQL dump handler.
    ///
    /// * `config` — parsed MySQL config from the YAML file
    /// * `dump_path` — directory where the SQL dump file will be written
    pub fn new(config: &'a MySQLConfig, dump_path: &str) -> Self {
        Self {
            config,
            dump_path: dump_path.to_string(),
        }
    }

    /// Build the list of arguments for `mysqldump`.
    ///
    /// This is a separate function so we can test the argument construction
    /// without actually running mysqldump.
    pub fn build_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Host (default "localhost" from config.rs)
        args.push("--host".to_string());
        args.push(self.config.host.clone());

        // Port — only add if explicitly configured
        if let Some(port) = self.config.port {
            args.push("--port".to_string());
            args.push(port.to_string());
        }

        // Authentication
        if let Some(ref username) = self.config.username {
            args.push("-u".to_string());
            args.push(username.clone());
        }
        if let Some(ref password) = self.config.password {
            // mysqldump accepts -pPASSWORD without space
            args.push(format!("-p{password}"));
        }

        // Database name — if not set, mysqldump will complain
        args.push(self.config.database.clone());

        // Output file named after the database
        let output_file = format!("{}/{}.sql", self.dump_path, self.config.database);
        args.push("--result-file".to_string());
        args.push(output_file);

        args
    }

    /// Run `mysqldump` with the configured arguments.
    ///
    /// Returns `Ok(())` on success, or `Err(message)` if the subprocess fails.
    pub fn perform(&self) -> Result<(), String> {
        let args = self.build_args();

        let output = Command::new("mysqldump")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to run mysqldump: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("mysqldump failed:\n{stderr}"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MySQLConfig;

    fn make_config(overrides: impl FnOnce(&mut MySQLConfig)) -> MySQLConfig {
        let mut config = MySQLConfig {
            host: "localhost".to_string(),
            port: None,
            database: "testdb".to_string(),
            username: Some("root".to_string()),
            password: Some("secret".to_string()),
        };
        overrides(&mut config);
        config
    }

    #[test]
    fn build_args_uses_given_host_and_port() {
        let config = make_config(|c| {
            c.host = "db.example.com".to_string();
            c.port = Some(3307);
        });
        let mysql = MySQL::new(&config, "/tmp/dumps");

        let args = mysql.build_args();
        assert!(args.contains(&"--host".to_string()));
        assert!(args.contains(&"db.example.com".to_string()));
        assert!(args.contains(&"--port".to_string()));
        assert!(args.contains(&"3307".to_string()));
    }

    #[test]
    fn build_args_omits_port_when_not_set() {
        let config = make_config(|c| c.port = None);
        let mysql = MySQL::new(&config, "/tmp/dumps");

        let args = mysql.build_args();
        assert!(
            !args.contains(&"--port".to_string()),
            "Should not add --port when port is None"
        );
    }

    #[test]
    fn build_args_includes_auth() {
        let config = make_config(|_| {});
        let mysql = MySQL::new(&config, "/tmp/dumps");

        let args = mysql.build_args();
        // -u root
        let u_idx = args.iter().position(|a| a == "-u").unwrap();
        assert_eq!(args[u_idx + 1], "root");
        // -psecret
        assert!(args.contains(&"-psecret".to_string()));
    }

    #[test]
    fn build_args_omits_auth_when_not_configured() {
        let config = make_config(|c| {
            c.username = None;
            c.password = None;
        });
        let mysql = MySQL::new(&config, "/tmp/dumps");

        let args = mysql.build_args();
        assert!(!args.contains(&"-u".to_string()));
    }

    #[test]
    fn build_args_uses_database_name() {
        let config = make_config(|c| {
            c.database = "my_app_prod".to_string();
        });
        let mysql = MySQL::new(&config, "/tmp/dumps");

        let args = mysql.build_args();
        assert!(args.contains(&"my_app_prod".to_string()));
    }

    #[test]
    fn build_args_includes_result_file() {
        let config = make_config(|_| {});
        let mysql = MySQL::new(&config, "/tmp/dumps");

        let args = mysql.build_args();
        let rf_idx = args.iter().position(|a| a == "--result-file").unwrap();
        assert_eq!(args[rf_idx + 1], "/tmp/dumps/testdb.sql");
    }
}
