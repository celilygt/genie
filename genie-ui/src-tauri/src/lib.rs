//! Genie Desktop UI - Tauri backend
//!
//! This module provides Tauri commands that bridge to genie-core functionality.
//! Also starts an HTTP server for OpenAI-compatible API access.

mod commands;
mod state;

use state::AppState;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tokio::sync::RwLock;
use tracing::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize app state
            let mut state = AppState::new().expect("Failed to initialize app state");

            // Start the HTTP server in the background
            let server_addr = state.config.server_addr();
            if let Err(e) = state.start_server() {
                tracing::error!("Failed to start HTTP server: {}", e);
            } else {
                info!("Genie API available at http://{}/v1/chat/completions", server_addr);
            }

            app.manage(Arc::new(RwLock::new(state)));

            // Create tray menu
            let show_item = MenuItem::with_id(app, "show", "Show Genie", true, None::<&str>)?;
            let hide_item = MenuItem::with_id(app, "hide", "Hide Genie", true, None::<&str>)?;
            let api_info = MenuItem::with_id(
                app,
                "api_info",
                format!("API: http://{}", server_addr),
                false,
                None::<&str>,
            )?;
            let separator = MenuItem::with_id(app, "sep", "---", false, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show_item, &hide_item, &api_info, &separator, &quit_item])?;

            // Build tray icon
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Genie - Local AI Service")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide window instead of closing when user clicks close button
            if let WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Chat commands
            commands::chat::send_message,
            // Docs commands
            commands::docs::summarize_pdf,
            commands::docs::summarize_book,
            // Repo commands
            commands::repo::summarize_repo,
            // Template commands
            commands::templates::list_templates,
            commands::templates::get_template,
            commands::templates::run_template,
            // Quota commands
            commands::quota::get_quota_status,
            commands::quota::get_usage_log,
            // Config commands
            commands::config::get_config,
            commands::config::health_check,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
