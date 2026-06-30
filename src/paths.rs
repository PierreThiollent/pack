use std::path::PathBuf;

/// Get the user's home directory.
pub(crate) fn home_dir() -> String {
    std::env::var("HOME").expect("Could not find HOME environment variable")
}

pub(crate) fn pack_home_dir() -> PathBuf {
    PathBuf::from(home_dir()).join(".pack")
}

pub(crate) fn pack_log_file_path() -> PathBuf {
    pack_home_dir().join("pack.log")
}

pub(crate) fn pack_pid_file_path() -> PathBuf {
    pack_home_dir().join("pack.pid")
}

/// Expand a leading tilde in a path.
///
/// Example: `~/foo` becomes `$HOME/foo`.
pub(crate) fn expand_tilde(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => format!("{}/{rest}", home_dir()),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_returns_home_environment_variable() {
        let home = std::env::var("HOME").unwrap();

        assert_eq!(home_dir(), home);
    }

    #[test]
    fn expand_tilde_expands_home_prefix() {
        let home = std::env::var("HOME").unwrap();

        assert_eq!(expand_tilde("~/Desktop"), format!("{home}/Desktop"));
    }

    #[test]
    fn expand_tilde_keeps_paths_without_home_prefix() {
        assert_eq!(expand_tilde("/tmp/pack"), "/tmp/pack");
        assert_eq!(expand_tilde("./local"), "./local");
        assert_eq!(expand_tilde("~"), "~");
    }

    #[test]
    fn pack_runtime_paths_live_in_pack_home() {
        let home = std::env::var("HOME").unwrap();

        assert_eq!(pack_home_dir(), PathBuf::from(&home).join(".pack"));
        assert_eq!(
            pack_log_file_path(),
            PathBuf::from(&home).join(".pack/pack.log")
        );
        assert_eq!(
            pack_pid_file_path(),
            PathBuf::from(&home).join(".pack/pack.pid")
        );
    }
}
