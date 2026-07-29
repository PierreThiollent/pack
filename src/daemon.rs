use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use daemonize::{Daemonize, Outcome, Parent};
use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

const STOP_TIMEOUT: Duration = Duration::from_secs(3);
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DaemonProcess {
    Parent,
    Child,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StopOutcome {
    NotRunning,
    InvalidPidFileRemoved,
    StalePidFileRemoved { pid: i32 },
    Stopped { pid: i32 },
    StopRequested { pid: i32 },
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

pub(crate) fn stop(pid_file_path: &Path) -> Result<StopOutcome, String> {
    let pid_file_content = match fs::read_to_string(pid_file_path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(StopOutcome::NotRunning);
        }
        Err(error) => {
            return Err(format!(
                "Failed to read PID file {}: {error}",
                pid_file_path.display()
            ));
        }
    };

    let pid = match parse_pid(&pid_file_content) {
        Some(pid) => pid,
        None => {
            remove_pid_file(pid_file_path)?;
            return Ok(StopOutcome::InvalidPidFileRemoved);
        }
    };

    if !process_exists(pid)? {
        remove_pid_file(pid_file_path)?;
        return Ok(StopOutcome::StalePidFileRemoved { pid });
    }

    terminate_process(pid)?;

    if wait_until_stopped(pid)? {
        remove_pid_file(pid_file_path)?;
        Ok(StopOutcome::Stopped { pid })
    } else {
        Ok(StopOutcome::StopRequested { pid })
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

fn parse_pid(content: &str) -> Option<i32> {
    let pid = content.trim().parse::<i32>().ok()?;
    (pid > 0).then_some(pid)
}

fn process_exists(pid: i32) -> Result<bool, String> {
    match kill(Pid::from_raw(pid), None) {
        Ok(()) => Ok(true),
        Err(Errno::ESRCH) => Ok(false),
        Err(Errno::EPERM) => Ok(true),
        Err(error) => Err(format!("Failed to check daemon process {pid}: {error}")),
    }
}

fn terminate_process(pid: i32) -> Result<(), String> {
    match kill(Pid::from_raw(pid), Signal::SIGTERM) {
        Ok(()) => Ok(()),
        Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(format!("Failed to stop daemon process {pid}: {error}")),
    }
}

fn remove_pid_file(pid_file_path: &Path) -> Result<(), String> {
    match fs::remove_file(pid_file_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Failed to remove PID file {}: {error}",
            pid_file_path.display()
        )),
    }
}

fn wait_until_stopped(pid: i32) -> Result<bool, String> {
    let deadline = Instant::now() + STOP_TIMEOUT;
    while Instant::now() < deadline {
        if !process_exists(pid)? {
            return Ok(true);
        }
        thread::sleep(STOP_POLL_INTERVAL);
    }

    Ok(!process_exists(pid)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pid_accepts_positive_pid() {
        assert_eq!(parse_pid("1234\n"), Some(1234));
    }

    #[test]
    fn parse_pid_rejects_invalid_or_non_positive_pid() {
        assert_eq!(parse_pid(""), None);
        assert_eq!(parse_pid("abc"), None);
        assert_eq!(parse_pid("0"), None);
        assert_eq!(parse_pid("-1"), None);
    }

    #[test]
    fn stop_returns_not_running_when_pid_file_is_missing() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let pid_file_path = temporary_directory.path().join("pack.pid");

        let outcome = stop(&pid_file_path).unwrap();

        assert_eq!(outcome, StopOutcome::NotRunning);
    }

    #[test]
    fn stop_removes_invalid_pid_file() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let pid_file_path = temporary_directory.path().join("pack.pid");
        fs::write(&pid_file_path, "not-a-pid").unwrap();

        let outcome = stop(&pid_file_path).unwrap();

        assert_eq!(outcome, StopOutcome::InvalidPidFileRemoved);
        assert!(!pid_file_path.exists());
    }
}
