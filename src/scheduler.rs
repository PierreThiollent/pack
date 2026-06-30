use crate::config::Config;
use crate::logging::{LogTag, tag};
use crate::model;
use serde::Deserialize;
use std::sync::Arc;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::Semaphore;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info};

/// Automatic execution schedule for a model.
#[derive(Debug, Deserialize)]
pub struct ScheduleConfig {
    /// Cron expression, for example `5 4 * * sun`.
    pub cron: Option<String>,

    /// Interval expression, for example `1day`.
    pub every: Option<String>,

    /// Optional time used with `every`, for example `04:05`.
    pub at: Option<String>,
}

pub async fn run_foreground(config: Config) -> Result<(), String> {
    info!(pack_tag = %tag(LogTag::Run), "Starting scheduler...");

    log_scheduled_models(&config);

    let mut scheduler = JobScheduler::new()
        .await
        .map_err(|error| format!("Failed to create scheduler: {error}"))?;

    register_cron_jobs(&scheduler, Arc::new(config)).await?;

    scheduler
        .start()
        .await
        .map_err(|error| format!("Failed to start scheduler: {error}"))?;

    info!(pack_tag = %tag(LogTag::Run), "Scheduler started");

    wait_for_shutdown_signal().await?;

    info!(
        pack_tag = %tag(LogTag::Run),
        "Received shutdown signal, stopping scheduler..."
    );

    scheduler
        .shutdown()
        .await
        .map_err(|error| format!("Failed to stop scheduler: {error}"))?;

    Ok(())
}

async fn register_cron_jobs(scheduler: &JobScheduler, config: Arc<Config>) -> Result<(), String> {
    let backup_semaphore = Arc::new(Semaphore::new(1));

    for (model_name, cron) in scheduled_cron_models(&config) {
        let normalized_cron = normalize_cron_expression(&cron)?;
        let job_model_name = model_name.clone();
        let job_config = Arc::clone(&config);
        let job_backup_semaphore = Arc::clone(&backup_semaphore);
        let job = Job::new_async(
            normalized_cron.clone(),
            move |_job_identifier, _scheduler_lock| {
                let model_name = job_model_name.clone();
                let config = Arc::clone(&job_config);
                let backup_semaphore = Arc::clone(&job_backup_semaphore);

                Box::pin(async move {
                    let permit = match backup_semaphore.try_acquire_owned() {
                        Ok(permit) => permit,
                        Err(_) => {
                            info!(
                                pack_tag = %tag(LogTag::Model(&model_name)),
                                "Scheduled backup skipped because another backup is already running"
                            );
                            return;
                        }
                    };

                    info!(
                        pack_tag = %tag(LogTag::Model(&model_name)),
                        "Starting scheduled backup"
                    );

                    let model_name_for_run = model_name.clone();
                    match tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        model::run_one(&config, &model_name_for_run)
                    })
                    .await
                    {
                        Ok(Ok(())) => info!(
                            pack_tag = %tag(LogTag::Model(&model_name)),
                            "Scheduled backup completed"
                        ),
                        Ok(Err(run_error)) => error!(
                            pack_tag = %tag(LogTag::Model(&model_name)),
                            "Scheduled backup failed: {run_error}"
                        ),
                        Err(join_error) => error!(
                            pack_tag = %tag(LogTag::Model(&model_name)),
                            "Scheduled backup task failed: {join_error}"
                        ),
                    }
                })
            },
        )
        .map_err(|error| format!("Failed to create cron job for model {model_name}: {error}"))?;

        scheduler.add(job).await.map_err(|error| {
            format!("Failed to register cron job for model {model_name}: {error}")
        })?;

        info!(
            pack_tag = %tag(LogTag::Run),
            "Registered cron job for model {model_name}: {normalized_cron}"
        );
    }

    Ok(())
}

async fn wait_for_shutdown_signal() -> Result<(), String> {
    let mut terminate_signal = signal(SignalKind::terminate())
        .map_err(|error| format!("Failed to listen for SIGTERM: {error}"))?;

    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result.map_err(|error| format!("Failed to listen for Ctrl+C: {error}"))?;
        }
        _ = terminate_signal.recv() => {}
    }

    Ok(())
}

fn log_scheduled_models(config: &Config) {
    let mut scheduled_model_count = 0;

    for (model_name, model) in &config.models {
        let Some(schedule) = &model.schedule else {
            continue;
        };

        scheduled_model_count += 1;
        info!(
            pack_tag = %tag(LogTag::Run),
            "Registered model {model_name} with schedule {}",
            schedule_description(schedule)
        );
    }

    if scheduled_model_count == 0 {
        info!(
            pack_tag = %tag(LogTag::Run),
            "No scheduled model found. Add a schedule block to a model to run it automatically."
        );
    }
}

fn normalize_cron_expression(cron: &str) -> Result<String, String> {
    let fields: Vec<&str> = cron.split_whitespace().collect();

    match fields.len() {
        5 => Ok(format!("0 {}", fields.join(" "))),
        6 => Ok(fields.join(" ")),
        field_count => Err(format!(
            "Invalid cron expression `{cron}`: expected 5 fields like GoBackup (`min hour day-of-month month day-of-week`) or 6 fields with seconds (`sec min hour day-of-month month day-of-week`), got {field_count}"
        )),
    }
}

fn scheduled_cron_models(config: &Config) -> Vec<(String, String)> {
    config
        .models
        .iter()
        .filter_map(|(model_name, model)| {
            model
                .schedule
                .as_ref()
                .and_then(|schedule| schedule.cron.as_ref())
                .map(|cron| (model_name.clone(), cron.clone()))
        })
        .collect()
}

fn schedule_description(schedule: &ScheduleConfig) -> String {
    if let Some(cron) = &schedule.cron {
        return format!("cron {cron}");
    }

    if let Some(every) = &schedule.every {
        if let Some(at) = &schedule.at {
            return format!("every {every} at {at}");
        }

        return format!("every {every}");
    }

    "empty schedule".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_description_formats_cron_schedule() {
        let schedule = ScheduleConfig {
            cron: Some("5 4 * * sun".to_string()),
            every: None,
            at: None,
        };

        assert_eq!(schedule_description(&schedule), "cron 5 4 * * sun");
    }

    #[test]
    fn normalize_cron_expression_prefixes_gobackup_five_field_cron() {
        let normalized_cron = normalize_cron_expression("5 4 * * sun").unwrap();

        assert_eq!(normalized_cron, "0 5 4 * * sun");
    }

    #[test]
    fn normalize_cron_expression_keeps_six_field_cron() {
        let normalized_cron = normalize_cron_expression("0 5 4 * * sun").unwrap();

        assert_eq!(normalized_cron, "0 5 4 * * sun");
    }

    #[test]
    fn normalize_cron_expression_returns_error_for_invalid_field_count() {
        let result = normalize_cron_expression("5 4 * *");

        assert_eq!(
            result,
            Err("Invalid cron expression `5 4 * *`: expected 5 fields like GoBackup (`min hour day-of-month month day-of-week`) or 6 fields with seconds (`sec min hour day-of-month month day-of-week`), got 4".to_string())
        );
    }

    #[test]
    fn schedule_description_formats_every_schedule() {
        let schedule = ScheduleConfig {
            cron: None,
            every: Some("1day".to_string()),
            at: None,
        };

        assert_eq!(schedule_description(&schedule), "every 1day");
    }

    #[test]
    fn schedule_description_formats_every_schedule_with_at() {
        let schedule = ScheduleConfig {
            cron: None,
            every: Some("1day".to_string()),
            at: Some("04:05".to_string()),
        };

        assert_eq!(schedule_description(&schedule), "every 1day at 04:05");
    }
}
