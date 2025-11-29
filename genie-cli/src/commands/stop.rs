//! `genie stop` command - Stop the running daemon

use anyhow::Result;
use genie_core::Config;

pub async fn run(config: Config) -> Result<()> {
    let url = format!("{}/health", config.server_url());

    println!("Attempting to stop Genie daemon...");

    // Check if daemon is running
    match reqwest::get(&url).await {
        Ok(response) => {
            if response.status().is_success() {
                // In a real implementation, we'd send a shutdown signal
                // For now, we just inform the user
                println!("\n⚠️  Genie daemon is running at {}", config.server_url());
                println!("   To stop it:");
                println!("   - If running with TUI: press 'q' in the terminal");
                println!("   - If running in background: use Ctrl+C in that terminal");
                println!("   - Or find the process: ps aux | grep genie");
            } else {
                println!("\n❌ Genie daemon is not running");
            }
        }
        Err(_) => {
            println!("\n❌ Genie daemon is not running");
        }
    }

    Ok(())
}
