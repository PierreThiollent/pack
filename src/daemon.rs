use std::path::Path;

use daemonize::{Daemonize, Outcome, Parent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DaemonProcess {
    Parent,
    Child,
}

/// Fork and detach the current process.
///
/// The parent should print the final user-facing startup message and exit.
/// The child should initialize file logging, load the config, and run the scheduler.
pub(crate) fn start(pid_file_path: &Path) -> Result<DaemonProcess, String> {
    // Keep the launch directory as the daemon working directory, like GoBackup,
    // so relative paths behave the same with `pack run` and `pack start`.
    let current_directory = std::env::current_dir()
        .map_err(|error| format!("Failed to get current directory: {error}"))?;
    let daemonize = Daemonize::new()
        .pid_file(pid_file_path)
        .working_directory(current_directory);

    match daemonize.execute() {
        Outcome::Parent(Ok(parent)) => parent_result(parent),
        Outcome::Parent(Err(error)) => Err(format!("Failed to daemonize parent process: {error}")),
        Outcome::Child(Ok(_child)) => Ok(DaemonProcess::Child),
        Outcome::Child(Err(error)) => Err(format!("Failed to daemonize child process: {error}")),
    }
}

fn parent_result(parent: Parent) -> Result<DaemonProcess, String> {
    if parent.first_child_exit_code == 0 {
        Ok(DaemonProcess::Parent)
    } else {
        Err(format!(
            "Failed to daemonize: first child exited with status {}",
            parent.first_child_exit_code
        ))
    }
}
