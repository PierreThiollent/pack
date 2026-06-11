/// Get the user's home directory.
pub(crate) fn home_dir() -> String {
    std::env::var("HOME").expect("Could not find HOME environment variable")
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
        assert_eq!(expand_tilde("/tmp/rbak"), "/tmp/rbak");
        assert_eq!(expand_tilde("./local"), "./local");
        assert_eq!(expand_tilde("~"), "~");
    }
}
