//! Terminal User Interface for Genie daemon
//!
//! Provides a Tilt-like TUI showing:
//! - Quota status (requests/minute, requests/day)
//! - Recent request log
//! - Server status

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use genie_core::{server::AppState, Config, QuotaConfig, UsageEvent, UsageStats};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;

/// TUI view mode
#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    Compact,
    Expanded,
}

/// Cached data for rendering (avoids blocking in render functions)
struct CachedData {
    stats: Option<UsageStats>,
    events: Vec<UsageEvent>,
    config: QuotaConfig,
}

impl Default for CachedData {
    fn default() -> Self {
        Self {
            stats: None,
            events: Vec::new(),
            config: QuotaConfig::default(),
        }
    }
}

/// TUI application state
struct App {
    state: Arc<AppState>,
    config: Config,
    view_mode: ViewMode,
    should_quit: bool,
    cached: CachedData,
}

impl App {
    fn new(state: Arc<AppState>, config: Config) -> Self {
        Self {
            state,
            config,
            view_mode: ViewMode::Expanded,
            should_quit: false,
            cached: CachedData::default(),
        }
    }

    fn toggle_view(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Compact => ViewMode::Expanded,
            ViewMode::Expanded => ViewMode::Compact,
        };
    }

    /// Refresh cached data from the state (async, non-blocking)
    async fn refresh_data(&mut self) {
        // Get stats
        if let Ok(stats) = self.state.quota.get_stats().await {
            self.cached.stats = Some(stats);
        }

        // Get recent events
        if let Ok(events) = self.state.quota.get_recent_events(20).await {
            self.cached.events = events;
        }

        // Get config
        self.cached.config = self.state.config.read().await.quota.clone();
    }
}

/// Run the TUI
pub async fn run(
    state: Arc<AppState>,
    config: Config,
    mut _shutdown_rx: mpsc::Receiver<()>,
) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(state, config);

    // Initial data load
    app.refresh_data().await;

    // Main loop
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    let mut event_stream = EventStream::new();
    let mut refresh_interval = interval(Duration::from_millis(500));

    loop {
        // Draw UI
        terminal.draw(|f| ui(f, app))?;

        // Handle events with timeout for periodic refresh
        tokio::select! {
            // Handle keyboard events
            maybe_event = event_stream.next() => {
                if let Some(Ok(event)) = maybe_event {
                    if let Event::Key(key) = event {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    app.should_quit = true;
                                }
                                KeyCode::Char(' ') => {
                                    app.toggle_view();
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            // Periodic data refresh
            _ = refresh_interval.tick() => {
                app.refresh_data().await;
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(if app.view_mode == ViewMode::Expanded {
                10
            } else {
                5
            }), // Status
            Constraint::Min(5),    // Logs
            Constraint::Length(3), // Help
        ])
        .split(f.area());

    render_header(f, chunks[0], app);
    render_status(f, chunks[1], app);
    render_logs(f, chunks[2], app);
    render_help(f, chunks[3], app);
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            " 🧞 GENIE ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(" │ "),
        Span::styled(app.config.server_url(), Style::default().fg(Color::Green)),
    ])])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(header, area);
}

fn render_status(f: &mut Frame, area: Rect, app: &App) {
    let config = &app.cached.config;

    let block = Block::default()
        .title(" 📊 Quota Status ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    if app.view_mode == ViewMode::Expanded {
        // Expanded view with gauges
        let inner = block.inner(area);
        f.render_widget(block, area);

        let status_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(inner);

        if let Some(stats) = &app.cached.stats {
            // Daily quota gauge
            let daily_pct =
                (stats.requests_today as f64 / config.per_day as f64 * 100.0).min(100.0);
            let daily_gauge = Gauge::default()
                .block(Block::default().title(format!(
                    "Daily: {}/{} requests",
                    stats.requests_today, config.per_day
                )))
                .gauge_style(Style::default().fg(if daily_pct > 90.0 {
                    Color::Red
                } else if daily_pct > 70.0 {
                    Color::Yellow
                } else {
                    Color::Green
                }))
                .percent(daily_pct as u16);
            f.render_widget(daily_gauge, status_chunks[0]);

            // Minute quota gauge
            let minute_pct =
                (stats.requests_last_minute as f64 / config.per_minute as f64 * 100.0).min(100.0);
            let minute_gauge = Gauge::default()
                .block(Block::default().title(format!(
                    "Per minute: {}/{} requests",
                    stats.requests_last_minute, config.per_minute
                )))
                .gauge_style(Style::default().fg(if minute_pct > 90.0 {
                    Color::Red
                } else if minute_pct > 70.0 {
                    Color::Yellow
                } else {
                    Color::Green
                }))
                .percent(minute_pct as u16);
            f.render_widget(minute_gauge, status_chunks[1]);

            // Token stats
            let token_info = Paragraph::new(format!(
                "Tokens today: {} in / {} out",
                stats.input_tokens_today, stats.output_tokens_today
            ))
            .style(Style::default().fg(Color::DarkGray));
            f.render_widget(token_info, status_chunks[2]);
        } else {
            let loading =
                Paragraph::new("Loading stats...").style(Style::default().fg(Color::DarkGray));
            f.render_widget(loading, inner);
        }
    } else {
        // Compact view
        let inner = block.inner(area);
        f.render_widget(block, area);

        if let Some(stats) = &app.cached.stats {
            let status_text = Paragraph::new(format!(
                "Day: {}/{} │ Min: {}/{} │ Tokens: {} in / {} out",
                stats.requests_today,
                config.per_day,
                stats.requests_last_minute,
                config.per_minute,
                stats.input_tokens_today,
                stats.output_tokens_today
            ))
            .style(Style::default().fg(Color::White));
            f.render_widget(status_text, inner);
        }
    }
}

fn render_logs(f: &mut Frame, area: Rect, app: &App) {
    let events = &app.cached.events;

    let items: Vec<ListItem> = if events.is_empty() {
        vec![ListItem::new("No requests yet...")]
    } else {
        events
            .iter()
            .map(|e| {
                let time = chrono::DateTime::parse_from_rfc3339(&e.timestamp)
                    .map(|t| t.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|_| "???".to_string());

                let status_color = if e.success { Color::Green } else { Color::Red };
                let status_symbol = if e.success { "✓" } else { "✗" };

                // Build the main line
                let mut spans = vec![
                    Span::styled(format!("{} ", time), Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} ", status_symbol),
                        Style::default().fg(status_color),
                    ),
                    Span::styled(format!("{:<8} ", e.kind), Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("{:<14} ", truncate(&e.model, 14)),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::raw(format!(
                        "{}→{} tokens",
                        e.approx_input_tokens, e.approx_output_tokens
                    )),
                ];

                // Add error info if present
                if !e.success {
                    if let Some(ref error_code) = e.error_code {
                        // Truncate error to fit on screen but show meaningful info
                        let error_display = format_error_message(error_code);
                        spans.push(Span::styled(
                            format!(" │ {}", error_display),
                            Style::default()
                                .fg(Color::Red)
                                .add_modifier(Modifier::ITALIC),
                        ));
                    }
                }

                ListItem::new(Line::from(spans))
            })
            .collect()
    };

    let logs = List::new(items)
        .block(
            Block::default()
                .title(" 📝 Recent Requests ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(logs, area);
}

fn render_help(f: &mut Frame, area: Rect, _app: &App) {
    let help = Paragraph::new(Line::from(vec![
        Span::styled(" q ", Style::default().fg(Color::Black).bg(Color::White)),
        Span::raw(" Quit  "),
        Span::styled(
            " Space ",
            Style::default().fg(Color::Black).bg(Color::White),
        ),
        Span::raw(" Toggle View  "),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(help, area);
}

/// Truncate a string to max_len characters
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}

/// Format error message for display - extracts key info from error strings
fn format_error_message(error: &str) -> String {
    // Try to extract HTTP status codes and meaningful error info
    let error_lower = error.to_lowercase();

    // Check for common HTTP error patterns
    if error_lower.contains("500") {
        if error_lower.contains("internal server error") {
            return "500 Internal Server Error - Gemini API failed".to_string();
        }
        return "500 Server Error".to_string();
    }
    if error_lower.contains("429") || error_lower.contains("rate limit") {
        return "429 Rate Limited".to_string();
    }
    if error_lower.contains("401") || error_lower.contains("unauthorized") {
        return "401 Unauthorized - Check API key".to_string();
    }
    if error_lower.contains("403") || error_lower.contains("forbidden") {
        return "403 Forbidden".to_string();
    }
    if error_lower.contains("404") {
        return "404 Not Found".to_string();
    }
    if error_lower.contains("timeout") {
        return "Request Timeout".to_string();
    }
    if error_lower.contains("connection") {
        return "Connection Error".to_string();
    }

    // For other errors, truncate to reasonable length
    truncate(error, 40)
}
