//! `genie service` commands - Manage Genie as a macOS LaunchAgent
//!
//! This module provides commands to install, start, stop, and check status
//! of Genie as a background service on macOS.

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const LAUNCH_AGENT_LABEL: &str = "com.genie.server";

/// Get the path to the LaunchAgents directory
fn launch_agents_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    Ok(home.join("Library").join("LaunchAgents"))
}

/// Get the path to the plist file
fn plist_path() -> Result<PathBuf> {
    Ok(launch_agents_dir()?.join(format!("{}.plist", LAUNCH_AGENT_LABEL)))
}

/// Get the path to the genie binary
fn genie_binary_path() -> Result<PathBuf> {
    // Try to find the current executable
    let current_exe = std::env::current_exe().context("Could not determine current executable")?;

    // If running from cargo, suggest installing
    if current_exe
        .to_string_lossy()
        .contains("target/debug")
        || current_exe.to_string_lossy().contains("target/release")
    {
        // Return the expected installed path
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
        let cargo_bin = home.join(".cargo").join("bin").join("genie");
        if cargo_bin.exists() {
            return Ok(cargo_bin);
        }
        // Fall back to current exe path for development
        return Ok(current_exe);
    }

    Ok(current_exe)
}

/// Get log file paths
fn log_paths() -> Result<(PathBuf, PathBuf)> {
    let genie_dir =
        genie_core::Config::genie_dir().ok_or_else(|| anyhow!("Could not determine genie directory"))?;
    fs::create_dir_all(&genie_dir)?;
    Ok((
        genie_dir.join("server.out.log"),
        genie_dir.join("server.err.log"),
    ))
}

/// Generate the plist content
fn generate_plist(binary_path: &std::path::Path, port: u16) -> Result<String> {
    let (stdout_log, stderr_log) = log_paths()?;

    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>up</string>
        <string>--daemon</string>
        <string>--port</string>
        <string>{port}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>StandardOutPath</key>
    <string>{stdout}</string>
    <key>StandardErrorPath</key>
    <string>{stderr}</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin</string>
    </dict>
    <key>ProcessType</key>
    <string>Background</string>
    <key>ThrottleInterval</key>
    <integer>5</integer>
</dict>
</plist>
"#,
        label = LAUNCH_AGENT_LABEL,
        binary = binary_path.display(),
        port = port,
        stdout = stdout_log.display(),
        stderr = stderr_log.display(),
    ))
}

/// Install the LaunchAgent
pub fn install(port: u16, force: bool) -> Result<()> {
    // Check if we're on macOS
    if !cfg!(target_os = "macos") {
        return Err(anyhow!(
            "LaunchAgent installation is only supported on macOS.\n\
             On Linux, consider using systemd or a similar init system."
        ));
    }

    let plist = plist_path()?;

    // Check if already installed
    if plist.exists() && !force {
        return Err(anyhow!(
            "LaunchAgent already installed at {}\n\
             Use --force to overwrite, or run 'genie service uninstall' first.",
            plist.display()
        ));
    }

    // Get the binary path
    let binary = genie_binary_path()?;
    if !binary.exists() {
        return Err(anyhow!(
            "Genie binary not found at {}\n\
             Please install genie first: cargo install --path .",
            binary.display()
        ));
    }

    // Ensure LaunchAgents directory exists
    let agents_dir = launch_agents_dir()?;
    fs::create_dir_all(&agents_dir).context("Failed to create LaunchAgents directory")?;

    // Generate and write plist
    let plist_content = generate_plist(&binary, port)?;
    fs::write(&plist, &plist_content).context("Failed to write plist file")?;

    println!("✅ LaunchAgent installed successfully!");
    println!("   Plist: {}", plist.display());
    println!("   Binary: {}", binary.display());
    println!("   Port: {}", port);
    println!();
    println!("To start the service now, run:");
    println!("   genie service start");
    println!();
    println!("The service will automatically start on login.");

    Ok(())
}

/// Uninstall the LaunchAgent
pub fn uninstall() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(anyhow!("LaunchAgent uninstallation is only supported on macOS."));
    }

    let plist = plist_path()?;

    // Stop first if running
    let _ = stop();

    if plist.exists() {
        fs::remove_file(&plist).context("Failed to remove plist file")?;
        println!("✅ LaunchAgent uninstalled successfully!");
    } else {
        println!("ℹ️  LaunchAgent was not installed.");
    }

    Ok(())
}

/// Start the LaunchAgent
pub fn start() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(anyhow!("LaunchAgent is only supported on macOS."));
    }

    let plist = plist_path()?;

    if !plist.exists() {
        return Err(anyhow!(
            "LaunchAgent not installed. Run 'genie service install' first."
        ));
    }

    // Use launchctl to load the agent
    let output = Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&plist)
        .output()
        .context("Failed to run launchctl")?;

    if output.status.success() {
        println!("✅ Genie service started!");
        println!("   The server is now running in the background.");
        println!();
        println!("   API endpoint: http://127.0.0.1:11435/v1/chat/completions");
        println!("   Check status: genie service status");
        println!("   View logs:    genie service logs");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already loaded") || stderr.contains("Operation already in progress") {
            println!("ℹ️  Service is already running.");
        } else {
            return Err(anyhow!("Failed to start service: {}", stderr));
        }
    }

    Ok(())
}

/// Stop the LaunchAgent
pub fn stop() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(anyhow!("LaunchAgent is only supported on macOS."));
    }

    let plist = plist_path()?;

    if !plist.exists() {
        println!("ℹ️  LaunchAgent not installed.");
        return Ok(());
    }

    // Use launchctl to unload the agent
    let output = Command::new("launchctl")
        .args(["unload", "-w"])
        .arg(&plist)
        .output()
        .context("Failed to run launchctl")?;

    if output.status.success() {
        println!("✅ Genie service stopped.");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Could not find specified service") {
            println!("ℹ️  Service was not running.");
        } else {
            return Err(anyhow!("Failed to stop service: {}", stderr));
        }
    }

    Ok(())
}

/// Restart the LaunchAgent
pub fn restart() -> Result<()> {
    stop()?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    start()?;
    Ok(())
}

/// Check if the service is running
fn is_service_running() -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }

    let output = Command::new("launchctl")
        .args(["list", LAUNCH_AGENT_LABEL])
        .output();

    match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// Get the PID of the running service
fn get_service_pid() -> Option<u32> {
    if !cfg!(target_os = "macos") {
        return None;
    }

    let output = Command::new("launchctl")
        .args(["list", LAUNCH_AGENT_LABEL])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse output format: "PID	Status	Label"
    // First field is PID (or "-" if not running)
    stdout
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .and_then(|pid_str| pid_str.parse::<u32>().ok())
}

/// Check the status of the LaunchAgent
pub async fn status(config: genie_core::Config) -> Result<()> {
    let plist = plist_path()?;

    println!("🔍 Genie Service Status");
    println!("─────────────────────────────────────────");

    // Check installation
    if plist.exists() {
        println!("   Installed:  ✅ Yes");
        println!("   Plist:      {}", plist.display());
    } else {
        println!("   Installed:  ❌ No");
        println!();
        println!("   Run 'genie service install' to set up the service.");
        return Ok(());
    }

    // Check if running via launchctl
    let running = is_service_running();
    if running {
        println!("   Running:    ✅ Yes");
        if let Some(pid) = get_service_pid() {
            println!("   PID:        {}", pid);
        }
    } else {
        println!("   Running:    ❌ No");
    }

    // Try to connect to the server
    let server_url = config.server_url();
    let health_url = format!("{}/health", server_url);

    println!("   Server URL: {}", server_url);

    match reqwest::Client::new()
        .get(&health_url)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                println!("   API Status: ✅ Responding");
                if let Ok(health) = response.json::<serde_json::Value>().await {
                    if let Some(version) = health.get("version").and_then(|v| v.as_str()) {
                        println!("   Version:    {}", version);
                    }
                    if let Some(gemini) = health.get("gemini_available").and_then(|v| v.as_bool()) {
                        println!(
                            "   Gemini:     {}",
                            if gemini { "✅ Available" } else { "⚠️  Not available" }
                        );
                    }
                }
            } else {
                println!("   API Status: ⚠️  Error ({})", response.status());
            }
        }
        Err(_) => {
            println!("   API Status: ❌ Not responding");
        }
    }

    // Show log paths
    let (stdout_log, stderr_log) = log_paths()?;
    println!();
    println!("📋 Log Files:");
    println!("   stdout: {}", stdout_log.display());
    println!("   stderr: {}", stderr_log.display());

    Ok(())
}

/// View recent logs
pub fn logs(lines: u32, follow: bool) -> Result<()> {
    let (stdout_log, stderr_log) = log_paths()?;

    if follow {
        // Use tail -f to follow logs
        println!("Following logs (Ctrl+C to stop)...\n");

        let mut child = Command::new("tail")
            .args(["-f", "-n"])
            .arg(lines.to_string())
            .arg(&stdout_log)
            .arg(&stderr_log)
            .spawn()
            .context("Failed to start tail")?;

        child.wait()?;
    } else {
        // Show recent lines from both logs
        println!("📋 Recent Logs (last {} lines)\n", lines);

        if stdout_log.exists() {
            println!("─── stdout ───");
            let output = Command::new("tail")
                .args(["-n"])
                .arg(lines.to_string())
                .arg(&stdout_log)
                .output()?;
            print!("{}", String::from_utf8_lossy(&output.stdout));
            println!();
        }

        if stderr_log.exists() {
            println!("─── stderr ───");
            let output = Command::new("tail")
                .args(["-n"])
                .arg(lines.to_string())
                .arg(&stderr_log)
                .output()?;
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
    }

    Ok(())
}
