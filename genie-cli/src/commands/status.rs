//! `genie status` command - Check daemon status

use anyhow::Result;
use genie_core::Config;

pub async fn run(config: Config) -> Result<()> {
    let url = format!("{}/health", config.server_url());

    println!("Checking Genie daemon status...");
    println!("URL: {}", url);

    // Try to connect to the server
    match reqwest::get(&url).await {
        Ok(response) => {
            if response.status().is_success() {
                let health: serde_json::Value = response.json().await?;

                println!("\n✅ Genie daemon is running");
                println!(
                    "   Status:          {}",
                    health
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                );
                println!(
                    "   Version:         {}",
                    health
                        .get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                );
                println!(
                    "   Gemini CLI:      {}",
                    if health
                        .get("gemini_available")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        "available ✓"
                    } else {
                        "not available ⚠"
                    }
                );

                // Get quota info too
                let quota_url = format!("{}/v1/quota", config.server_url());
                if let Ok(quota_response) = reqwest::get(&quota_url).await {
                    if let Ok(quota) = quota_response.json::<serde_json::Value>().await {
                        println!("\n📊 Quota:");
                        println!(
                            "   Today:    {}/{}",
                            quota
                                .get("requests_today")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            quota
                                .get("requests_per_day_limit")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                        );
                        println!(
                            "   Minute:   {}/{}",
                            quota
                                .get("requests_last_minute")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            quota
                                .get("requests_per_minute_limit")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                        );
                    }
                }
            } else {
                println!(
                    "\n⚠️  Genie daemon responded with status: {}",
                    response.status()
                );
            }
        }
        Err(_) => {
            println!("\n❌ Genie daemon is not running");
            println!("   Start it with: genie up");
        }
    }

    Ok(())
}
