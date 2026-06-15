use time::macros::format_description;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::LocalTime;

pub fn init() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let timer = LocalTime::new(format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]"
    ));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_timer(timer)
        .with_writer(std::io::stderr)
        .init();
}
