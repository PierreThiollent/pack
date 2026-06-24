use std::fmt;
use std::io::IsTerminal;
use time::macros::format_description;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::fmt::time::{FormatTime, LocalTime};
use tracing_subscriber::registry::LookupSpan;

const TAG_FIELD_NAME: &str = "pack_tag";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogTag<'a> {
    Config,
    Run,
    Cleanup,
    Cycler,
    Archive,
    Compressor(Option<&'a str>),
    Model(&'a str),
    Storage(&'a str),
    Database {
        database_type: &'a str,
        database_name: &'a str,
    },
    Local,
    Ftp,
    Sftp,
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

pub fn init() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let timer = LocalTime::new(format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]"
    ));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .event_format(CliFormatter::new(timer, detect_colors_enabled()))
        .with_writer(std::io::stderr)
        .init();
}

pub fn tag(log_tag: LogTag<'_>) -> TagDisplay<'_> {
    TagDisplay { log_tag }
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
        LogTag::Storage(storage_name) => format!("[Storage: {storage_name}]"),
        LogTag::Database {
            database_type,
            database_name,
        } => format!("[{database_type}: {database_name}]"),
        LogTag::Local => "[Local]".to_string(),
        LogTag::Ftp => "[FTP]".to_string(),
        LogTag::Sftp => "[SFTP]".to_string(),
    }
}

fn tag_color_from_text(text: &str) -> LogColor {
    // The formatter receives `pack_tag` after it has been formatted as text,
    // so color detection is based on the rendered tag string.
    if text.starts_with("[Config]")
        || text.starts_with("[Storage:")
        || text.starts_with("[FTP]")
        || text.starts_with("[SFTP]")
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
}
