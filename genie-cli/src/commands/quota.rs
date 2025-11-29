//! `genie quota` commands - View and manage usage quotas

use anyhow::Result;
use genie_core::{Config, QuotaManager, QuotaStatus};
use tracing::info;

/// Show quota status
pub async fn status(config: Config, json_output: bool) -> Result<()> {
    let db_path = Config::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;

    let quota_manager = QuotaManager::new(&db_path, config.quota.clone()).await?;
    let stats = quota_manager.get_stats().await?;

    let quota_status = QuotaStatus {
        requests_today: stats.requests_today,
        requests_per_day_limit: config.quota.per_day,
        requests_last_minute: stats.requests_last_minute,
        requests_per_minute_limit: config.quota.per_minute,
        approx_input_tokens_today: stats.input_tokens_today,
        approx_output_tokens_today: stats.output_tokens_today,
        last_error: stats.last_error,
        reset_time: config.quota.reset_time.clone(),
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&quota_status)?);
    } else {
        println!("╭─────────────────────────────────────────╮");
        println!("│           Genie Quota Status            │");
        println!("├─────────────────────────────────────────┤");
        println!(
            "│  Requests today:    {:>6} / {:<6}     │",
            quota_status.requests_today, quota_status.requests_per_day_limit
        );
        println!(
            "│  Requests/minute:   {:>6} / {:<6}     │",
            quota_status.requests_last_minute, quota_status.requests_per_minute_limit
        );
        println!("├─────────────────────────────────────────┤");
        println!(
            "│  Input tokens:      {:>10}          │",
            quota_status.approx_input_tokens_today
        );
        println!(
            "│  Output tokens:     {:>10}          │",
            quota_status.approx_output_tokens_today
        );
        println!(
            "│  Total tokens:      {:>10}          │",
            quota_status.approx_input_tokens_today + quota_status.approx_output_tokens_today
        );
        println!("├─────────────────────────────────────────┤");
        println!(
            "│  Reset time:        {:>10}          │",
            quota_status.reset_time
        );
        if let Some(error) = &quota_status.last_error {
            println!("├─────────────────────────────────────────┤");
            println!("│  Last error: {:<26} │", truncate(error, 26));
        }
        println!("╰─────────────────────────────────────────╯");

        // Show progress bars
        let day_pct = (quota_status.requests_today as f64
            / quota_status.requests_per_day_limit as f64
            * 100.0)
            .min(100.0);
        let min_pct = (quota_status.requests_last_minute as f64
            / quota_status.requests_per_minute_limit as f64
            * 100.0)
            .min(100.0);

        println!("\nDaily:  {}", progress_bar(day_pct, 30));
        println!("Minute: {}", progress_bar(min_pct, 30));
    }

    Ok(())
}

/// Show recent usage log
pub async fn log(config: Config, last: u32) -> Result<()> {
    let db_path = Config::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;

    let quota_manager = QuotaManager::new(&db_path, config.quota.clone()).await?;
    let events = quota_manager.get_recent_events(last).await?;

    if events.is_empty() {
        println!("No usage events recorded yet.");
        return Ok(());
    }

    println!("╭──────────────────────────────────────────────────────────────────────────────╮");
    println!("│                              Recent Usage Log                                │");
    println!("├────────────────────┬────────────────┬──────────┬────────┬───────────────────┤");
    println!("│ Time               │ Model          │ Kind     │ Status │ Tokens (in/out)   │");
    println!("├────────────────────┼────────────────┼──────────┼────────┼───────────────────┤");

    for event in &events {
        let time = chrono::DateTime::parse_from_rfc3339(&event.timestamp)
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|_| "???".to_string());

        let status = if event.success { "✓" } else { "✗" };
        // status_color could be used for ANSI coloring in future
        let _status_color = if event.success { "green" } else { "red" };

        println!(
            "│ {:<18} │ {:<14} │ {:<8} │   {}    │ {:>6} / {:<6}   │",
            time,
            truncate(&event.model, 14),
            truncate(&event.kind, 8),
            status,
            event.approx_input_tokens,
            event.approx_output_tokens
        );
    }

    println!("╰────────────────────┴────────────────┴──────────┴────────┴───────────────────╯");

    info!("Displayed {} recent events", events.len());
    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}

fn progress_bar(percentage: f64, width: usize) -> String {
    let filled = ((percentage / 100.0) * width as f64) as usize;
    let empty = width.saturating_sub(filled);

    let (color, _label) = if percentage >= 90.0 {
        ("🔴", "CRITICAL")
    } else if percentage >= 70.0 {
        ("🟡", "WARNING")
    } else {
        ("🟢", "OK")
    };

    format!(
        "[{}{}] {:>5.1}% {}",
        "█".repeat(filled),
        "░".repeat(empty),
        percentage,
        color
    )
}
