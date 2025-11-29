//! `genie config` commands - View and manage configuration

use anyhow::Result;
use genie_core::Config;

/// Show current configuration
pub fn show(config: Config) -> Result<()> {
    println!("╭─────────────────────────────────────────╮");
    println!("│         Genie Configuration             │");
    println!("├─────────────────────────────────────────┤");
    println!("│ Gemini                                  │");
    println!("│   Binary:       {:<23} │", config.gemini.binary);
    println!("│   Model:        {:<23} │", config.gemini.default_model);
    if let Some(sys) = &config.gemini.system_prompt {
        println!("│   System:       {:<23} │", truncate(sys, 23));
    }
    println!("├─────────────────────────────────────────┤");
    println!("│ Server                                  │");
    println!("│   Host:         {:<23} │", config.server.host);
    println!("│   Port:         {:<23} │", config.server.port);
    println!("│   URL:          {:<23} │", config.server_url());
    println!("├─────────────────────────────────────────┤");
    println!("│ Quota                                   │");
    println!("│   Per minute:   {:<23} │", config.quota.per_minute);
    println!("│   Per day:      {:<23} │", config.quota.per_day);
    println!("│   Reset time:   {:<23} │", config.quota.reset_time);
    println!("├─────────────────────────────────────────┤");
    println!("│ Logging                                 │");
    println!("│   Level:        {:<23} │", config.logging.level);
    println!("╰─────────────────────────────────────────╯");

    // Show paths
    println!("\n📁 Paths:");
    if let Some(path) = Config::default_config_path() {
        let exists = path.exists();
        println!(
            "   Config:   {} {}",
            path.display(),
            if exists { "✓" } else { "(not created)" }
        );
    }
    if let Some(path) = Config::default_db_path() {
        let exists = path.exists();
        println!(
            "   Database: {} {}",
            path.display(),
            if exists { "✓" } else { "(not created)" }
        );
    }
    if let Some(path) = Config::prompts_dir() {
        let exists = path.exists();
        println!(
            "   Prompts:  {} {}",
            path.display(),
            if exists { "✓" } else { "(not created)" }
        );
    }

    Ok(())
}

/// Initialize default configuration
pub fn init(force: bool) -> Result<()> {
    let path = Config::default_config_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config path"))?;

    if path.exists() && !force {
        println!(
            "⚠️  Configuration file already exists at: {}",
            path.display()
        );
        println!("   Use --force to overwrite.");
        return Ok(());
    }

    // Ensure directory exists
    Config::ensure_dirs()?;

    // Create default config
    let config = Config::default();
    config.save_to_file(&path)?;

    println!("✅ Created configuration file at: {}", path.display());
    println!("\n📝 Default configuration:");
    println!("{}", toml::to_string_pretty(&config)?);

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}
