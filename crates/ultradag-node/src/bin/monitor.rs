//! ultradag-monitor — Remote TUI dashboard for any UltraDAG node.
//!
//! Connects to a node's HTTP RPC endpoint and displays a live dashboard
//! without running a validator. Reuses the same visual style as the node TUI.

use std::{
    collections::VecDeque,
    io::{self, stdout},
    time::{Duration, Instant},
};

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline},
    Frame, Terminal,
};
use serde::Deserialize;

// Brand colours (matching node TUI)
const ACCENT: Color = Color::Rgb(0, 224, 196);
const BLUE: Color = Color::Rgb(0, 102, 255);
const WARN: Color = Color::Rgb(255, 184, 0);
const ERR: Color = Color::Rgb(239, 68, 68);
const DIM: Color = Color::DarkGray;
const LABEL: Color = Color::DarkGray;

const COIN: u64 = 100_000_000;
const SPARKLINE_WIDTH: usize = 60;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "ultradag-monitor", about = "Remote TUI dashboard for UltraDAG nodes")]
struct Args {
    /// Node RPC endpoint URL
    #[arg(default_value = "http://localhost:10333")]
    url: String,
}

// ---------------------------------------------------------------------------
// JSON response types (loosely typed — we only parse what we need)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Default)]
struct StatusResponse {
    #[serde(default)]
    last_finalized_round: Option<u64>,
    #[serde(default)]
    peer_count: usize,
    #[serde(default)]
    mempool_size: usize,
    #[serde(default)]
    total_supply: u64,
    #[serde(default)]
    account_count: usize,
    #[serde(default)]
    dag_round: u64,
    #[serde(default)]
    dag_vertices: usize,
    #[serde(default)]
    dag_tips: usize,
    #[serde(default)]
    finalized_count: usize,
    #[serde(default)]
    validator_count: usize,
    #[serde(default)]
    total_staked: u64,
    #[serde(default)]
    active_stakers: usize,
    #[serde(default)]
    treasury_balance: u64,
    #[serde(default)]
    memory_usage_bytes: Option<u64>,
    #[serde(default)]
    uptime_seconds: Option<u64>,
}

#[derive(Deserialize, Clone, Default)]
struct HealthResponse {
    #[serde(default)]
    status: String,
    #[serde(default)]
    warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Monitor state
// ---------------------------------------------------------------------------

struct MonitorState {
    url: String,
    status: Option<StatusResponse>,
    health: Option<HealthResponse>,
    sparkline_buf: VecDeque<u64>,
    last_fetch: Instant,
    fetch_latency_ms: u64,
    error: Option<String>,
    start_time: Instant,
    fetch_count: u64,
}

impl MonitorState {
    fn new(url: String) -> Self {
        Self {
            url,
            status: None,
            health: None,
            sparkline_buf: VecDeque::with_capacity(SPARKLINE_WIDTH + 1),
            last_fetch: Instant::now(),
            fetch_latency_ms: 0,
            error: None,
            start_time: Instant::now(),
            fetch_count: 0,
        }
    }

    fn finality_lag(&self) -> u64 {
        self.status
            .as_ref()
            .map(|s| s.dag_round.saturating_sub(s.last_finalized_round.unwrap_or(0)))
            .unwrap_or(0)
    }

    fn tick_sparkline(&mut self) {
        self.sparkline_buf.push_back(self.finality_lag());
        while self.sparkline_buf.len() > SPARKLINE_WIDTH {
            self.sparkline_buf.pop_front();
        }
    }
}

// ---------------------------------------------------------------------------
// Data fetching
// ---------------------------------------------------------------------------

async fn fetch_data(client: &reqwest::Client, state: &mut MonitorState) {
    let t0 = Instant::now();

    let status_url = format!("{}/status", state.url);
    let health_url = format!("{}/health/detailed", state.url);

    let (status_res, health_res) = tokio::join!(client.get(&status_url).send(), client.get(&health_url).send());

    match status_res {
        Ok(resp) => match resp.json::<StatusResponse>().await {
            Ok(s) => {
                state.status = Some(s);
                state.error = None;
            }
            Err(e) => state.error = Some(format!("Parse error: {}", e)),
        },
        Err(e) => state.error = Some(format!("Connection failed: {}", e)),
    }

    if let Ok(resp) = health_res {
        if let Ok(h) = resp.json::<HealthResponse>().await {
            state.health = Some(h);
        }
    }

    state.fetch_latency_ms = t0.elapsed().as_millis() as u64;
    state.last_fetch = Instant::now();
    state.fetch_count += 1;
    state.tick_sparkline();
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    let url = args.url.trim_end_matches('/').to_string();

    // Panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let mut state = MonitorState::new(url);
    let mut last_fetch_time = Instant::now() - Duration::from_secs(10); // fetch immediately

    loop {
        // Fetch every 1 second
        if last_fetch_time.elapsed() >= Duration::from_secs(1) {
            fetch_data(&client, &mut state).await;
            last_fetch_time = Instant::now();
        }

        // Draw
        terminal.draw(|f| render(f, &state))?;

        // Handle input (poll 200ms)
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break,
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            last_fetch_time = Instant::now() - Duration::from_secs(10);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(f: &mut Frame, state: &MonitorState) {
    let area = f.area();

    let chunks = Layout::vertical([
        Constraint::Length(1), // Header
        Constraint::Length(1), // Separator
        Constraint::Length(5), // Stats dashboard
        Constraint::Length(6), // Sparkline
        Constraint::Length(5), // Connection info
        Constraint::Min(3),   // Health status
        Constraint::Length(1), // Sync bar
        Constraint::Length(1), // Footer
    ])
    .split(area);

    render_header(f, chunks[0], state);
    render_separator(f, chunks[1]);
    render_stats(f, chunks[2], state);
    render_sparkline(f, chunks[3], state);
    render_connection(f, chunks[4], state);
    render_health(f, chunks[5], state);
    render_sync_bar(f, chunks[6], state);
    render_footer(f, chunks[7]);
}

fn render_header(f: &mut Frame, area: Rect, state: &MonitorState) {
    let uptime = state.start_time.elapsed();
    let uptime_str = format!(
        "{:02}:{:02}:{:02}",
        uptime.as_secs() / 3600,
        (uptime.as_secs() % 3600) / 60,
        uptime.as_secs() % 60
    );

    let status_dot = if state.error.is_some() {
        Span::styled(" \u{25CF} ", Style::default().fg(ERR))
    } else {
        Span::styled(" \u{25CF} ", Style::default().fg(ACCENT))
    };

    let header = Line::from(vec![
        Span::styled(
            " \u{25C8} ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "ULTRADAG MONITOR",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  \u{2502}  ", Style::default().fg(DIM)),
        status_dot,
        Span::styled(&state.url, Style::default().fg(Color::White)),
        Span::styled("  \u{2502}  ", Style::default().fg(DIM)),
        Span::styled(
            format!("\u{23F1} {}", uptime_str),
            Style::default().fg(Color::White),
        ),
        Span::styled("  \u{2502}  ", Style::default().fg(DIM)),
        Span::styled(
            format!("{}ms", state.fetch_latency_ms),
            Style::default().fg(if state.fetch_latency_ms > 500 {
                WARN
            } else {
                DIM
            }),
        ),
    ]);

    f.render_widget(Paragraph::new(header), area);
}

fn render_separator(f: &mut Frame, area: Rect) {
    let sep = "\u{2500}".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(sep, Style::default().fg(DIM)))),
        area,
    );
}

fn render_stats(f: &mut Frame, area: Rect, state: &MonitorState) {
    let snap = state.status.as_ref().cloned().unwrap_or_default();
    let lag = state.finality_lag();
    let lag_color = match lag {
        0..=2 => ACCENT,
        3..=10 => WARN,
        _ => ERR,
    };

    let bold_white = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let label = Style::default().fg(LABEL);

    let finalized = snap.last_finalized_round.unwrap_or(0);

    let row1 = Line::from(vec![
        Span::styled(" ROUND ", label),
        Span::styled(format!("{:<12}", format_num(snap.dag_round)), bold_white),
        Span::styled("FINALIZED ", label),
        Span::styled(format!("{:<12}", format_num(finalized)), bold_white),
        Span::styled("LAG ", label),
        Span::styled(
            format!("{:<6}", lag),
            Style::default()
                .fg(lag_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("PEERS ", label),
        Span::styled(format!("{:<8}", snap.peer_count), bold_white),
        Span::styled("MEMPOOL ", label),
        Span::styled(format!("{:<8}", format!("{} tx", snap.mempool_size)), bold_white),
        Span::styled("VALIDATORS ", label),
        Span::styled(
            format!("{}/{}", snap.active_stakers, snap.validator_count),
            bold_white,
        ),
    ]);

    let row2 = Line::from(vec![
        Span::styled(" SUPPLY ", label),
        Span::styled(
            format!("{:<20}", format!("{} UDAG", fmt_udag(snap.total_supply))),
            bold_white,
        ),
        Span::styled("STAKED ", label),
        Span::styled(
            format!("{:<18}", format!("{} UDAG", fmt_udag(snap.total_staked))),
            bold_white,
        ),
        Span::styled("TREASURY ", label),
        Span::styled(
            format!("{:<18}", format!("{} UDAG", fmt_udag(snap.treasury_balance))),
            bold_white,
        ),
        Span::styled("ACCOUNTS ", label),
        Span::styled(format!("{}", format_num(snap.account_count as u64)), bold_white),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            " Dashboard ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(vec![row1, row2]).block(block);
    f.render_widget(paragraph, area);
}

fn render_sparkline(f: &mut Frame, area: Rect, state: &MonitorState) {
    let data: Vec<u64> = state.sparkline_buf.iter().copied().collect();
    let max_lag = data.iter().copied().max().unwrap_or(1).max(1);
    let current_lag = data.last().copied().unwrap_or(0);
    let lag_color = match current_lag {
        0..=2 => ACCENT,
        3..=10 => WARN,
        _ => ERR,
    };

    let title = format!(
        " Finality Lag ({}s)  current: {}  max: {} ",
        data.len(),
        current_lag,
        max_lag
    );

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(DIM))
                .title(Span::styled(title, Style::default().fg(lag_color))),
        )
        .data(&data)
        .max(max_lag.max(5))
        .style(Style::default().fg(lag_color));

    f.render_widget(sparkline, area);
}

fn render_connection(f: &mut Frame, area: Rect, state: &MonitorState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            " Connection ",
            Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
        ));

    let age = state.last_fetch.elapsed().as_secs();
    let age_color = if age > 5 { ERR } else { ACCENT };

    let label = Style::default().fg(LABEL);
    let val = Style::default().fg(Color::White);

    let snap = state.status.as_ref().cloned().unwrap_or_default();

    let mem_str = snap
        .memory_usage_bytes
        .map(|b| format!("{:.1} MB", b as f64 / 1_048_576.0))
        .unwrap_or_else(|| "-".into());

    let uptime_str = snap
        .uptime_seconds
        .map(|s| {
            let h = s / 3600;
            let m = (s % 3600) / 60;
            format!("{}h {:02}m", h, m)
        })
        .unwrap_or_else(|| "-".into());

    let mut lines = vec![Line::from(vec![
        Span::styled(" ENDPOINT ", label),
        Span::styled(format!("{:<30}", &state.url), val),
        Span::styled("LATENCY ", label),
        Span::styled(format!("{:<8}", format!("{}ms", state.fetch_latency_ms)), val),
        Span::styled("UPDATED ", label),
        Span::styled(format!("{}s ago", age), Style::default().fg(age_color)),
    ])];

    lines.push(Line::from(vec![
        Span::styled(" VERTICES ", label),
        Span::styled(format!("{:<12}", format_num(snap.dag_vertices as u64)), val),
        Span::styled("TIPS ", label),
        Span::styled(format!("{:<8}", snap.dag_tips), val),
        Span::styled("FINALIZED ", label),
        Span::styled(format!("{:<12}", format_num(snap.finalized_count as u64)), val),
        Span::styled("MEMORY ", label),
        Span::styled(format!("{:<12}", mem_str), val),
        Span::styled("NODE UP ", label),
        Span::styled(uptime_str, val),
    ]));

    if let Some(ref err) = state.error {
        lines.push(Line::from(vec![
            Span::styled(" ERROR ", Style::default().fg(ERR).add_modifier(Modifier::BOLD)),
            Span::styled(err.as_str(), Style::default().fg(ERR)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn render_health(f: &mut Frame, area: Rect, state: &MonitorState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            " Health ",
            Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
        ));

    let health = state.health.as_ref();
    let (status_str, status_color) = match health.map(|h| h.status.as_str()) {
        Some("healthy") => ("\u{25CF} HEALTHY", ACCENT),
        Some("warning") => ("\u{25CF} WARNING", WARN),
        Some("unhealthy") => ("\u{25CF} UNHEALTHY", ERR),
        Some("degraded") => ("\u{25CF} DEGRADED", ERR),
        _ if state.error.is_some() => ("\u{25CF} UNREACHABLE", ERR),
        _ => ("\u{25CF} CONNECTING...", DIM),
    };

    let mut lines = vec![Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            status_str,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("    Fetches: {}", state.fetch_count),
            Style::default().fg(DIM),
        ),
    ])];

    if let Some(h) = health {
        for w in &h.warnings {
            lines.push(Line::from(vec![
                Span::styled("  \u{26A0} ", Style::default().fg(WARN)),
                Span::styled(w.as_str(), Style::default().fg(WARN)),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn render_sync_bar(f: &mut Frame, area: Rect, state: &MonitorState) {
    let snap = state.status.as_ref();
    let finalized = snap
        .and_then(|s| s.last_finalized_round)
        .unwrap_or(0);
    let dag_round = snap.map(|s| s.dag_round).unwrap_or(0);
    let lag = dag_round.saturating_sub(finalized);

    if state.error.is_some() {
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(ERR))
            .percent(0)
            .label(Span::styled(
                " DISCONNECTED",
                Style::default().fg(ERR).add_modifier(Modifier::BOLD),
            ));
        f.render_widget(gauge, area);
    } else if lag <= 3 && dag_round > 0 {
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(ACCENT))
            .percent(100)
            .label(Span::styled(
                " SYNCED \u{2713}",
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));
        f.render_widget(gauge, area);
    } else if dag_round > 0 {
        let target = dag_round.max(1);
        let pct = ((finalized as f64 / target as f64) * 100.0).min(99.0).max(0.0) as u16;
        let label = format!(
            " {}% (finalized {}/{})",
            pct, format_num(finalized), format_num(dag_round)
        );
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(WARN))
            .percent(pct)
            .label(Span::styled(label, Style::default().fg(Color::White)));
        f.render_widget(gauge, area);
    } else {
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(DIM))
            .percent(0)
            .label(Span::styled(
                " Waiting for data...",
                Style::default().fg(DIM),
            ));
        f.render_widget(gauge, area);
    }
}

fn render_footer(f: &mut Frame, area: Rect) {
    let footer = Line::from(vec![
        Span::styled(
            " q",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Quit  ", Style::default().fg(DIM)),
        Span::styled(
            "r",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Refresh  ", Style::default().fg(DIM)),
        Span::styled(
            "Esc",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Exit", Style::default().fg(DIM)),
    ]);
    f.render_widget(Paragraph::new(footer), area);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_num(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn fmt_udag(sats: u64) -> String {
    let whole = sats / COIN;
    let frac = (sats % COIN) / (COIN / 100);
    if frac > 0 {
        format!("{}.{:02}", format_num(whole), frac)
    } else {
        format_num(whole)
    }
}
