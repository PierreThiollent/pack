use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use time::macros::format_description;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::fmt::time::{FormatTime, LocalTime};
use tracing_subscriber::fmt::writer::MakeWriter;
use tracing_subscriber::registry::LookupSpan;

const TAG_FIELD_NAME: &str = "pack_tag";
const DEFAULT_LOG_FILTER: &str = "info,tokio_cron_scheduler=warn";
static LOGGING_INITIALIZED: AtomicBool = AtomicBool::new(false);
const LOG_FILE_MAX_SIZE_BYTES: u64 = 10 * 1024 * 1024;
const LOG_FILE_MAX_BACKUPS: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogTag<'a> {
    Config,
    Run,
    Cleanup,
    Cycler,
    Archive,
    Compressor(Option<&'a str>),
    Model(&'a str),
    Notifier(&'a str),
    Storage(&'a str),
    Database {
        database_type: &'a str,
        database_name: &'a str,
    },
    Local,
    Ftp,
    Sftp,
    Scp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagDisplay<'a> {
    log_tag: LogTag<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogColor {
    Blue,
    Cyan,
    Dimmed,
    Green,
    Magenta,
    Red,
    Yellow,
}

/// Custom CLI formatter.
///
/// We keep a custom formatter because `tracing_subscriber` can color timestamps
/// and levels by default, but it does not provide a simple way to color only one
/// application-specific field (`pack_tag`). The output intentionally stays close
/// to tracing's compact text output while adding colored pack tags.
#[derive(Debug)]
struct CliFormatter<T> {
    timer: T,
    colors_enabled: bool,
}

#[derive(Default, Debug, PartialEq, Eq)]
struct EventFields {
    tag: Option<String>,
    message: Option<String>,
    extra_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogDestination {
    ConsoleOnly,
    ConsoleAndFile(PathBuf),
    FileOnly(PathBuf),
}

/// Factory required by `tracing_subscriber`.
///
/// `tracing_subscriber` does not write directly to a single `Write` value. Instead,
/// it asks a `MakeWriter` to create a concrete writer whenever it emits a log event.
/// This factory keeps the shared logging destination and the optional opened file.
#[derive(Clone)]
struct LogWriterFactory {
    destination: LogDestination,
    file: Option<Arc<Mutex<LogFile>>>,
}

/// Concrete writer used by `tracing_subscriber` for one log event.
///
/// It writes the already formatted log line to stderr, the log file, or both,
/// depending on the selected `LogDestination`.
struct LogWriter {
    write_to_console: bool,
    file: Option<Arc<Mutex<LogFile>>>,
}

pub fn init(destination: LogDestination) -> Result<(), String> {
    let colors_enabled = destination.writes_to_console() && detect_colors_enabled();
    let writer_factory = LogWriterFactory::new(destination)?;
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    let timer = LocalTime::new(format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]"
    ));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .event_format(CliFormatter::new(timer, colors_enabled))
        .with_writer(writer_factory)
        .init();
    LOGGING_INITIALIZED.store(true, Ordering::SeqCst);

    Ok(())
}

pub fn log_final_cli_error(error: &str) {
    if is_initialized() {
        tracing::error!(pack_tag = %tag(LogTag::Run), "{error}");
    } else {
        eprintln!("{error}");
    }
}

fn is_initialized() -> bool {
    LOGGING_INITIALIZED.load(Ordering::SeqCst)
}

pub fn tag(log_tag: LogTag<'_>) -> TagDisplay<'_> {
    TagDisplay { log_tag }
}

impl LogDestination {
    fn writes_to_console(&self) -> bool {
        matches!(
            self,
            LogDestination::ConsoleOnly | LogDestination::ConsoleAndFile(_)
        )
    }
}

impl LogWriterFactory {
    fn new(destination: LogDestination) -> Result<Self, String> {
        let file_path = match &destination {
            LogDestination::ConsoleOnly => None,
            LogDestination::ConsoleAndFile(path) | LogDestination::FileOnly(path) => Some(path),
        };
        let file = file_path
            .map(|path| open_log_file(path).map(|file| Arc::new(Mutex::new(file))))
            .transpose()?;

        Ok(Self { destination, file })
    }
}

/// Create the concrete writer that `tracing_subscriber` will use for an event.
impl<'writer> MakeWriter<'writer> for LogWriterFactory {
    type Writer = LogWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        LogWriter {
            write_to_console: self.destination.writes_to_console(),
            file: self.file.clone(),
        }
    }
}

/// Implement the actual write/flush behavior for console and file outputs.
impl Write for LogWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.write_to_console {
            io::stderr().write_all(buffer)?;
        }

        if let Some(file) = &self.file {
            let mut file = file
                .lock()
                .map_err(|_| io::Error::other("log file lock poisoned"))?;
            let plain_buffer = strip_ansi_codes(buffer);
            file.write_all(&plain_buffer)?;
        }

        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.write_to_console {
            io::stderr().flush()?;
        }

        if let Some(file) = &self.file {
            let mut file = file
                .lock()
                .map_err(|_| io::Error::other("log file lock poisoned"))?;
            file.flush()?;
        }

        Ok(())
    }
}

fn strip_ansi_codes(buffer: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(buffer.len());
    let mut index = 0;

    while index < buffer.len() {
        if buffer[index] == 0x1b && buffer.get(index + 1) == Some(&b'[') {
            index += 2;
            while index < buffer.len() && !buffer[index].is_ascii_alphabetic() {
                index += 1;
            }
            if index < buffer.len() {
                index += 1;
            }
        } else {
            result.push(buffer[index]);
            index += 1;
        }
    }

    result
}

fn open_log_file(path: &Path) -> Result<LogFile, String> {
    LogFile::open(path.to_path_buf())
}

struct LogFile {
    path: PathBuf,
    file: File,
    max_size_bytes: u64,
    max_backups: u8,
}

impl LogFile {
    fn open(path: PathBuf) -> Result<Self, String> {
        Self::open_internal(path, LOG_FILE_MAX_SIZE_BYTES, LOG_FILE_MAX_BACKUPS)
    }

    #[cfg(test)]
    fn open_with_limits(
        path: PathBuf,
        max_size_bytes: u64,
        max_backups: u8,
    ) -> Result<Self, String> {
        Self::open_internal(path, max_size_bytes, max_backups)
    }

    fn open_internal(path: PathBuf, max_size_bytes: u64, max_backups: u8) -> Result<Self, String> {
        if let Some(parent_directory) = path.parent() {
            fs::create_dir_all(parent_directory).map_err(|error| {
                format!("Failed to create log directory {parent_directory:?}: {error}")
            })?;
        }

        let file = open_log_file_append(&path)?;

        Ok(Self {
            path,
            file,
            max_size_bytes,
            max_backups,
        })
    }

    fn write_all(&mut self, buffer: &[u8]) -> io::Result<()> {
        self.rotate_if_needed(buffer.len() as u64)?;
        self.file.write_all(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }

    fn rotate_if_needed(&mut self, incoming_size: u64) -> io::Result<()> {
        let current_size = self.file.metadata()?.len();
        if current_size + incoming_size <= self.max_size_bytes {
            return Ok(());
        }

        self.file.flush()?;

        for index in (1..=self.max_backups).rev() {
            let rotated_path = rotated_log_path(&self.path, index);
            if !rotated_path.exists() {
                continue;
            }

            if index == self.max_backups {
                fs::remove_file(&rotated_path)?;
            } else {
                fs::rename(rotated_path, rotated_log_path(&self.path, index + 1))?;
            }
        }

        if self.path.exists() {
            fs::rename(&self.path, rotated_log_path(&self.path, 1))?;
        }

        self.file = open_log_file_append(&self.path).map_err(io::Error::other)?;
        Ok(())
    }
}

fn open_log_file_append(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("Failed to open log file {path:?}: {error}"))
}

fn rotated_log_path(path: &Path, index: u8) -> PathBuf {
    PathBuf::from(format!("{}.{}", path.display(), index))
}

impl<T> CliFormatter<T>
where
    T: FormatTime,
{
    fn new(timer: T, colors_enabled: bool) -> Self {
        Self {
            timer,
            colors_enabled,
        }
    }

    fn write_timestamp(&self, writer: &mut Writer<'_>) -> fmt::Result {
        if self.colors_enabled {
            writer.write_str(LogColor::Dimmed.ansi_code())?;
            self.timer.format_time(writer)?;
            writer.write_str("\u{1b}[0m")
        } else {
            self.timer.format_time(writer)
        }
    }
}

impl<S, N, T> FormatEvent<S, N> for CliFormatter<T>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
    T: FormatTime,
{
    fn format_event(
        &self,
        _context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        self.write_timestamp(&mut writer)?;
        writer.write_char(' ')?;
        writer.write_str(&format_level(event.metadata().level(), self.colors_enabled))?;
        writer.write_char(' ')?;

        write_event_fields(
            &mut writer,
            collect_event_fields(event),
            self.colors_enabled,
        )?;
        writer.write_char('\n')
    }
}

impl fmt::Display for TagDisplay<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&tag_text(self.log_tag))
    }
}

impl Visit for EventFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message = Some(value.to_string()),
            TAG_FIELD_NAME => self.tag = Some(value.to_string()),
            field_name => self.extra_fields.push(format!("{field_name}={value:?}")),
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let value = format!("{value:?}");

        match field.name() {
            "message" => self.message = Some(value),
            TAG_FIELD_NAME => self.tag = Some(value),
            field_name => self.extra_fields.push(format!("{field_name}={value}")),
        }
    }
}

fn collect_event_fields(event: &Event<'_>) -> EventFields {
    let mut fields = EventFields::default();
    event.record(&mut fields);
    fields
}

fn write_event_fields(
    writer: &mut Writer<'_>,
    fields: EventFields,
    colors_enabled: bool,
) -> fmt::Result {
    writer.write_str(&format_event_fields(fields, colors_enabled))
}

fn format_event_fields(fields: EventFields, colors_enabled: bool) -> String {
    let mut parts = Vec::new();

    if let Some(tag) = fields.tag {
        parts.push(format_tag_text(&tag, colors_enabled));
    }

    if let Some(message) = fields.message {
        parts.push(message);
    }

    parts.extend(fields.extra_fields);
    parts.join(" ")
}

fn detect_colors_enabled() -> bool {
    let no_color = std::env::var("NO_COLOR").ok();
    let force_color = std::env::var("FORCE_COLOR").ok();

    should_enable_colors(
        std::io::stderr().is_terminal(),
        no_color.as_deref(),
        force_color.as_deref(),
    )
}

fn should_enable_colors(
    stderr_is_terminal: bool,
    no_color: Option<&str>,
    force_color: Option<&str>,
) -> bool {
    // FORCE_COLOR intentionally wins over NO_COLOR, so users can force colors
    // even when the output is piped or when NO_COLOR is set globally.
    if force_color.is_some_and(force_color_is_enabled) {
        return true;
    }

    if no_color.is_some_and(|value| !value.is_empty()) {
        return false;
    }

    stderr_is_terminal
}

fn force_color_is_enabled(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
}

#[cfg(test)]
fn format_tag(log_tag: LogTag<'_>, colors_enabled: bool) -> String {
    format_tag_text(&tag_text(log_tag), colors_enabled)
}

fn format_level(level: &Level, colors_enabled: bool) -> String {
    let text = format!("{level:>5}");
    format_with_ansi_color(&text, level_color(level), colors_enabled)
}

fn format_tag_text(text: &str, colors_enabled: bool) -> String {
    format_with_ansi_color(text, tag_color_from_text(text), colors_enabled)
}

fn format_with_ansi_color(text: &str, color: LogColor, colors_enabled: bool) -> String {
    if colors_enabled {
        format!("{}{}\u{1b}[0m", color.ansi_code(), text)
    } else {
        text.to_string()
    }
}

fn tag_text(log_tag: LogTag<'_>) -> String {
    match log_tag {
        LogTag::Config => "[Config]".to_string(),
        LogTag::Run => "[Run]".to_string(),
        LogTag::Cleanup => "[Cleanup]".to_string(),
        LogTag::Cycler => "[Cycler]".to_string(),
        LogTag::Archive => "[Archive]".to_string(),
        LogTag::Compressor(Some(compressor)) => format!("[Compressor: {compressor}]"),
        LogTag::Compressor(None) => "[Compressor]".to_string(),
        LogTag::Model(model_name) => format!("[Model: {model_name}]"),
        LogTag::Notifier(notifier_name) => format!("[Notifier: {notifier_name}]"),
        LogTag::Storage(storage_name) => format!("[Storage: {storage_name}]"),
        LogTag::Database {
            database_type,
            database_name,
        } => format!("[{database_type}: {database_name}]"),
        LogTag::Local => "[Local]".to_string(),
        LogTag::Ftp => "[FTP]".to_string(),
        LogTag::Sftp => "[SFTP]".to_string(),
        LogTag::Scp => "[SCP]".to_string(),
    }
}

fn tag_color_from_text(text: &str) -> LogColor {
    // The formatter receives `pack_tag` after it has been formatted as text,
    // so color detection is based on the rendered tag string.
    if text.starts_with("[Config]")
        || text.starts_with("[Storage:")
        || text.starts_with("[Notifier:")
        || text.starts_with("[FTP]")
        || text.starts_with("[SFTP]")
        || text.starts_with("[SCP]")
    {
        LogColor::Cyan
    } else if text.starts_with("[Run]") {
        LogColor::Blue
    } else if text.starts_with("[Cleanup]")
        || text.starts_with("[Cycler]")
        || text.starts_with("[MySQL:")
    {
        LogColor::Yellow
    } else if text.starts_with("[Archive]") || text.starts_with("[Local]") {
        LogColor::Green
    } else {
        LogColor::Magenta
    }
}

fn level_color(level: &Level) -> LogColor {
    match *level {
        Level::ERROR => LogColor::Red,
        Level::WARN => LogColor::Yellow,
        Level::INFO => LogColor::Green,
        Level::DEBUG => LogColor::Blue,
        Level::TRACE => LogColor::Magenta,
    }
}

impl LogColor {
    fn ansi_code(self) -> &'static str {
        match self {
            LogColor::Blue => "\u{1b}[34m",
            LogColor::Cyan => "\u{1b}[36m",
            LogColor::Green => "\u{1b}[32m",
            LogColor::Magenta => "\u{1b}[35m",
            LogColor::Red => "\u{1b}[31m",
            LogColor::Yellow => "\u{1b}[33m",
            LogColor::Dimmed => "\u{1b}[2m",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_tag_formats_static_tag() {
        assert_eq!(format_tag(LogTag::Archive, false), "[Archive]");
    }

    #[test]
    fn plain_tag_formats_dynamic_tag() {
        assert_eq!(
            format_tag(LogTag::Model("my_site"), false),
            "[Model: my_site]"
        );
    }

    #[test]
    fn tag_display_is_plain_text_for_structured_fields() {
        assert_eq!(tag(LogTag::Archive).to_string(), "[Archive]");
    }

    #[test]
    fn colored_tag_wraps_only_the_tag_with_ansi_codes() {
        let result = format_tag(LogTag::Archive, true);

        assert!(result.starts_with("\u{1b}["));
        assert!(result.contains("[Archive]"));
        assert!(result.ends_with("\u{1b}[0m"));
    }

    #[test]
    fn plain_level_keeps_padding() {
        assert_eq!(format_level(&Level::INFO, false), " INFO");
    }

    #[test]
    fn colored_error_level_is_red() {
        assert_eq!(
            format_level(&Level::ERROR, true),
            "\u{1b}[31mERROR\u{1b}[0m"
        );
    }

    #[test]
    fn event_fields_format_tag_message_and_extra_fields_in_order() {
        let fields = EventFields {
            tag: Some(tag(LogTag::Archive).to_string()),
            message: Some("Archive created".to_string()),
            extra_fields: vec!["path=\"backup.tar\"".to_string()],
        };

        assert_eq!(
            format_event_fields(fields, false),
            "[Archive] Archive created path=\"backup.tar\""
        );
    }

    #[test]
    fn colors_are_enabled_for_terminal_output_by_default() {
        assert!(should_enable_colors(true, None, None));
    }

    #[test]
    fn no_color_disables_colors() {
        assert!(!should_enable_colors(true, Some("1"), None));
    }

    #[test]
    fn empty_no_color_does_not_disable_colors() {
        assert!(should_enable_colors(true, Some(""), None));
    }

    #[test]
    fn force_color_enables_colors_even_without_terminal() {
        assert!(should_enable_colors(false, None, Some("1")));
    }

    #[test]
    fn force_color_wins_over_no_color() {
        assert!(should_enable_colors(false, Some("1"), Some("1")));
    }

    #[test]
    fn console_and_file_writes_to_console() {
        assert!(LogDestination::ConsoleAndFile(PathBuf::from("pack.log")).writes_to_console());
    }

    #[test]
    fn file_only_does_not_write_to_console() {
        assert!(!LogDestination::FileOnly(PathBuf::from("pack.log")).writes_to_console());
    }

    #[test]
    fn strip_ansi_codes_removes_color_sequences() {
        let buffer = b"\x1b[32m INFO\x1b[0m [Run] hello\n";

        assert_eq!(strip_ansi_codes(buffer), b" INFO [Run] hello\n");
    }

    #[test]
    fn default_filter_hides_tokio_cron_scheduler_info_logs() {
        assert_eq!(DEFAULT_LOG_FILTER, "info,tokio_cron_scheduler=warn");
    }

    #[test]
    fn logging_starts_uninitialized() {
        assert!(!is_initialized());
    }

    #[test]
    fn log_rotation_uses_expected_defaults() {
        assert_eq!(LOG_FILE_MAX_SIZE_BYTES, 10 * 1024 * 1024);
        assert_eq!(LOG_FILE_MAX_BACKUPS, 5);
    }

    #[test]
    fn rotating_log_file_rotates_when_size_limit_is_reached() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let log_path = temporary_directory.path().join("pack.log");
        let mut log_file = LogFile::open_with_limits(log_path.clone(), 10, 5).unwrap();

        log_file.write_all(b"12345").unwrap();
        log_file.write_all(b"67890").unwrap();
        log_file.write_all(b"abc").unwrap();
        log_file.flush().unwrap();

        assert_eq!(fs::read_to_string(&log_path).unwrap(), "abc");
        assert_eq!(
            fs::read_to_string(rotated_log_path(&log_path, 1)).unwrap(),
            "1234567890"
        );
    }

    #[test]
    fn rotating_log_file_keeps_configured_backup_count() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let log_path = temporary_directory.path().join("pack.log");
        let mut log_file = LogFile::open_with_limits(log_path.clone(), 1, 2).unwrap();

        log_file.write_all(b"a").unwrap();
        log_file.write_all(b"b").unwrap();
        log_file.write_all(b"c").unwrap();
        log_file.write_all(b"d").unwrap();
        log_file.flush().unwrap();

        assert_eq!(fs::read_to_string(&log_path).unwrap(), "d");
        assert_eq!(
            fs::read_to_string(rotated_log_path(&log_path, 1)).unwrap(),
            "c"
        );
        assert_eq!(
            fs::read_to_string(rotated_log_path(&log_path, 2)).unwrap(),
            "b"
        );
        assert!(!rotated_log_path(&log_path, 3).exists());
    }
}
