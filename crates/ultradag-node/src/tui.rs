//! UltraDAG Node TUI - Terminal User Interface
//!
//! A beautiful dashboard for monitoring a running UltraDAG node.
//!
//! Features:
//! - Compact styled header with validator identity and network
//! - Real-time stats dashboard (round, finality, peers, supply, staking)
//! - Finality lag sparkline (60-second rolling window)
//! - Peer connection table
//! - Scrollable activity log with color-coded levels
//! - Sync progress bar
//! - Keyboard controls (quit, toggle logs, scroll, pause)

use std::{
    collections::VecDeque,
    io::{self, stdout},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

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
    widgets::{
        Block, Borders, Cell, Gauge, List, ListItem, ListState, Paragraph, Row, Sparkline, Table,
    },
    Frame, Terminal,
};
use tokio::sync::RwLock;
use ultradag_coin::constants::COIN;
use ultradag_network::NodeServer;

// ---------------------------------------------------------------------------
// Brand colours
// ---------------------------------------------------------------------------
const ACCENT: Color = Color::Rgb(0, 224, 196); // #00E0C4  cyan-green
const BLUE: Color = Color::Rgb(0, 102, 255); // #0066FF
const WARN: Color = Color::Rgb(255, 184, 0); // #FFB800
const ERR: Color = Color::Rgb(239, 68, 68); // #EF4444
const DIM: Color = Color::DarkGray;
const LABEL: Color = Color::DarkGray;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const TUI_REFRESH_MS: u64 = 200;
const MAX_LOG_LINES: usize = 200;
const SPARKLINE_WIDTH: usize = 60;

// ---------------------------------------------------------------------------
// Log types (re-exported for callers)
// ---------------------------------------------------------------------------

/// Log entry with timestamp and level.
#[derive(Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl LogLevel {
    fn color(&self) -> Color {
        match self {
            LogLevel::Info => ACCENT,
            LogLevel::Warn => WARN,
            LogLevel::Error => ERR,
            LogLevel::Debug => BLUE,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO ",
            LogLevel::Warn => "WARN ",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
        }
    }
}

// ---------------------------------------------------------------------------
// Data snapshot (filled from NodeServer, read by renderer)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PeerInfo {
    pub addr: String,
    pub validator_id: String,
    pub round: u64,
}

#[derive(Clone)]
pub struct NodeSnapshot {
    // DAG / finality
    pub current_round: u64,
    pub finalized_round: u64,
    // Peers
    pub peer_count: usize,
    pub peer_addrs: Vec<String>,
    // Mempool
    pub mempool_size: usize,
    // Sync
    pub sync_complete: bool,
    pub peer_max_round: u64,
    // Validators
    pub active_validator_count: usize,
    pub configured_validators: Option<u64>,
    // Economics
    pub total_supply: u64,
    pub total_staked: u64,
    pub total_delegated: u64,
    pub treasury_balance: u64,
    pub epoch: u64,
    // Identity
    pub validator_addr: Option<String>,
    pub is_testnet: bool,
}

// ---------------------------------------------------------------------------
// Shared TUI state
// ---------------------------------------------------------------------------

pub struct TuiState {
    pub logs: VecDeque<LogEntry>,
    pub log_scroll: ListState,
    pub show_logs: bool,
    pub paused: bool,
    pub node_snapshot: Option<NodeSnapshot>,

    // Sparkline ring buffer: finality lag sampled once per second
    sparkline_buf: VecDeque<u64>,
    last_sparkline_tick: Instant,
}

impl TuiState {
    pub fn new() -> Self {
        let mut log_scroll = ListState::default();
        log_scroll.select(Some(0));
        Self {
            logs: VecDeque::with_capacity(MAX_LOG_LINES + 1),
            log_scroll,
            show_logs: true,
            paused: false,
            node_snapshot: None,
            sparkline_buf: VecDeque::with_capacity(SPARKLINE_WIDTH + 1),
            last_sparkline_tick: Instant::now(),
        }
    }

    pub fn add_log(&mut self, level: LogLevel, message: String) {
        let now = chrono::Local::now();
        let timestamp = now.format("%H:%M:%S").to_string();
        self.logs.push_back(LogEntry {
            timestamp,
            level,
            message,
        });
        while self.logs.len() > MAX_LOG_LINES {
            self.logs.pop_front();
        }
        // Auto-scroll to bottom
        if self.show_logs {
            self.log_scroll
                .select(Some(self.logs.len().saturating_sub(1)));
        }
    }

    pub fn scroll_logs(&mut self, delta: i32) {
        if let Some(selected) = self.log_scroll.selected() {
            let new = (selected as i32 + delta)
                .clamp(0, self.logs.len().saturating_sub(1) as i32) as usize;
            self.log_scroll.select(Some(new));
        }
    }

    /// Sample the sparkline ring buffer (call once per second).
    fn tick_sparkline(&mut self) {
        if self.last_sparkline_tick.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.last_sparkline_tick = Instant::now();
        let lag = self
            .node_snapshot
            .as_ref()
            .map(|s| s.current_round.saturating_sub(s.finalized_round))
            .unwrap_or(0);
        self.sparkline_buf.push_back(lag);
        while self.sparkline_buf.len() > SPARKLINE_WIDTH {
            self.sparkline_buf.pop_front();
        }
    }

    /// Populate snapshot from NodeServer (non-blocking try_read).
    pub async fn update_snapshot(&mut self, server: &Arc<NodeServer>) {
        // Use try_read to avoid blocking the TUI render thread.
        let current_round = match server.dag.try_read() {
            Ok(dag) => dag.current_round(),
            Err(_) => {
                return;
            }
        };
        let finalized_round = match server.finality.try_read() {
            Ok(fin) => fin.last_finalized_round(),
            Err(_) => {
                return;
            }
        };
        let (total_supply, total_staked, total_delegated, treasury_balance, epoch, active_count, configured) =
            match server.state.try_read() {
                Ok(st) => (
                    st.total_supply(),
                    st.total_staked(),
                    st.total_delegated(),
                    st.treasury_balance(),
                    st.current_epoch(),
                    st.active_validators().len(),
                    st.configured_validator_count(),
                ),
                Err(_) => {
                    return;
                }
            };
        let mempool_size = match server.mempool.try_read() {
            Ok(mp) => mp.len(),
            Err(_) => 0,
        };
        let sync_complete = server.sync_complete.load(Ordering::Relaxed);
        let peer_max_round = server.peer_max_round.load(Ordering::Relaxed);
        let peer_addrs = server.peers.connected_listen_addrs().await;
        let peer_count = peer_addrs.len();

        let validator_addr = server.validator_sk.as_ref().map(|sk| {
            let pk = sk.verifying_key().to_bytes();
            let addr = ultradag_coin::Address::from_pubkey(&pk);
            addr.short()
        });

        let is_testnet = server.testnet_mode;

        self.node_snapshot = Some(NodeSnapshot {
            current_round,
            finalized_round,
            peer_count,
            peer_addrs,
            mempool_size,
            sync_complete,
            peer_max_round,
            active_validator_count: active_count,
            configured_validators: configured,
            total_supply,
            total_staked,
            total_delegated,
            treasury_balance,
            epoch,
            validator_addr,
            is_testnet,
        });
    }
}

impl Default for TuiState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

pub struct TuiApp {
    pub state: Arc<RwLock<TuiState>>,
    pub server: Arc<NodeServer>,
    pub should_quit: Arc<AtomicBool>,
    pub start_time: Instant,
}

impl TuiApp {
    pub fn new(server: Arc<NodeServer>) -> Self {
        Self {
            state: Arc::new(RwLock::new(TuiState::new())),
            server,
            should_quit: Arc::new(AtomicBool::new(false)),
            start_time: Instant::now(),
        }
    }

    pub async fn run(&mut self) -> io::Result<()> {
        // Install panic hook so terminal is restored on panic.
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

        let result = self.run_loop(&mut terminal).await;

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        result
    }

    async fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        let mut last_snapshot = Instant::now();

        loop {
            // Update snapshot (every 500ms)
            if last_snapshot.elapsed().as_millis() >= 500 {
                let mut st = self.state.write().await;
                if !st.paused {
                    st.update_snapshot(&self.server).await;
                    st.tick_sparkline();
                }
                drop(st);
                last_snapshot = Instant::now();
            }

            // Draw
            let start_time = self.start_time;
            let state_ref = self.state.clone();
            terminal.draw(|f| {
                ui(f, &state_ref, start_time);
            })?;

            // Handle keyboard input
            if event::poll(Duration::from_millis(TUI_REFRESH_MS))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => {
                                self.should_quit.store(true, Ordering::Relaxed);
                            }
                            KeyCode::Char('l') | KeyCode::Char('L') => {
                                let mut st = self.state.write().await;
                                st.show_logs = !st.show_logs;
                            }
                            KeyCode::Char(' ') => {
                                let mut st = self.state.write().await;
                                st.paused = !st.paused;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                let mut st = self.state.write().await;
                                st.scroll_logs(-1);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                let mut st = self.state.write().await;
                                st.scroll_logs(1);
                            }
                            KeyCode::PageUp => {
                                let mut st = self.state.write().await;
                                st.scroll_logs(-10);
                            }
                            KeyCode::PageDown => {
                                let mut st = self.state.write().await;
                                st.scroll_logs(10);
                            }
                            KeyCode::End => {
                                let mut st = self.state.write().await;
                                let len = st.logs.len();
                                st.log_scroll.select(Some(len.saturating_sub(1)));
                            }
                            KeyCode::Home => {
                                let mut st = self.state.write().await;
                                st.log_scroll.select(Some(0));
                            }
                            _ => {}
                        }
                    }
                }
            }

            if self.should_quit.load(Ordering::Relaxed) {
                break;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Rendering (fully synchronous, reads snapshot only)
// ---------------------------------------------------------------------------

fn ui(f: &mut Frame, state_arc: &Arc<RwLock<TuiState>>, start_time: Instant) {
    let state = match state_arc.try_read() {
        Ok(s) => s,
        Err(_) => return,
    };

    let snap = state.node_snapshot.clone().unwrap_or(NodeSnapshot {
        current_round: 0,
        finalized_round: 0,
        peer_count: 0,
        peer_addrs: vec![],
        mempool_size: 0,
        sync_complete: false,
        peer_max_round: 0,
        active_validator_count: 0,
        configured_validators: None,
        total_supply: 0,
        total_staked: 0,
        total_delegated: 0,
        treasury_balance: 0,
        epoch: 0,
        validator_addr: None,
        is_testnet: true,
    });

    let area = f.area();

    let chunks = Layout::vertical([
        Constraint::Length(1), // Header
        Constraint::Length(1), // Separator
        Constraint::Length(5), // Stats dashboard (2 rows + borders)
        Constraint::Length(6), // Sparkline
        Constraint::Length(8), // Peers
        Constraint::Min(6),   // Logs (expands)
        Constraint::Length(1), // Sync bar
        Constraint::Length(1), // Footer
    ])
    .split(area);

    render_header(f, chunks[0], &snap, start_time);
    render_separator(f, chunks[1]);
    render_stats(f, chunks[2], &snap);
    render_sparkline(f, chunks[3], &state);
    render_peers(f, chunks[4], &snap);
    render_logs(f, chunks[5], &state);
    render_sync_bar(f, chunks[6], &snap);
    render_footer(f, chunks[7], &state);
}

/// Compact one-line header with identity, network, and uptime.
fn render_header(f: &mut Frame, area: Rect, snap: &NodeSnapshot, start_time: Instant) {
    let uptime = start_time.elapsed();
    let uptime_str = format!(
        "{:02}:{:02}:{:02}",
        uptime.as_secs() / 3600,
        (uptime.as_secs() % 3600) / 60,
        uptime.as_secs() % 60
    );

    let addr_str = snap
        .validator_addr
        .clone()
        .unwrap_or_else(|| "observer".into());

    let (net_label, net_color) = if snap.is_testnet {
        ("TESTNET", WARN)
    } else {
        ("MAINNET", ACCENT)
    };

    let header = Line::from(vec![
        Span::styled(" \u{25C8} ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(
            "ULTRADAG",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  \u{2502}  ", Style::default().fg(DIM)),
        Span::styled(addr_str, Style::default().fg(Color::White)),
        Span::styled("  \u{2502}  ", Style::default().fg(DIM)),
        Span::styled(
            format!("\u{25C7} {}", net_label),
            Style::default().fg(net_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  \u{2502}  ", Style::default().fg(DIM)),
        Span::styled(
            format!("\u{23F1} {}", uptime_str),
            Style::default().fg(Color::White),
        ),
        Span::styled("  \u{2502}  ", Style::default().fg(DIM)),
        Span::styled("v0.1", Style::default().fg(DIM)),
    ]);

    f.render_widget(Paragraph::new(header), area);
}

/// Thin dim separator line.
fn render_separator(f: &mut Frame, area: Rect) {
    let sep = "\u{2500}".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(sep, Style::default().fg(DIM)))),
        area,
    );
}

/// Two-row stats dashboard.
fn render_stats(f: &mut Frame, area: Rect, snap: &NodeSnapshot) {
    let lag = snap.current_round.saturating_sub(snap.finalized_round);
    let lag_color = match lag {
        0..=2 => ACCENT,
        3..=10 => WARN,
        _ => ERR,
    };

    let val_total = snap
        .configured_validators
        .unwrap_or(snap.active_validator_count as u64);

    // Format supply values
    let fmt_udag = |sats: u64| -> String {
        let whole = sats / COIN;
        let frac = (sats % COIN) / (COIN / 100); // 2 decimal places
        if frac > 0 {
            format!("{},{:03}.{:02}", whole / 1000, whole % 1000, frac)
                .trim_start_matches("0,")
                .replace(",000", ",000")
                .to_string()
        } else {
            format_num(whole)
        }
    };

    let bold_white = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let label = Style::default().fg(LABEL);

    // Row 1 - operational metrics
    let row1 = Line::from(vec![
        Span::styled(" ROUND ", label),
        Span::styled(format!("{:<12}", format_num(snap.current_round)), bold_white),
        Span::styled("FINALIZED ", label),
        Span::styled(
            format!("{:<12}", format_num(snap.finalized_round)),
            bold_white,
        ),
        Span::styled("LAG ", label),
        Span::styled(
            format!("{:<6}", lag),
            Style::default()
                .fg(lag_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("PEERS ", label),
        Span::styled(
            format!("{:<8}", format!("{}/{}", snap.peer_count, val_total)),
            bold_white,
        ),
        Span::styled("MEMPOOL ", label),
        Span::styled(format!("{:<10}", format!("{} tx", snap.mempool_size)), bold_white),
        Span::styled("VALIDATORS ", label),
        Span::styled(
            format!(
                "{}/{}",
                snap.active_validator_count, val_total
            ),
            bold_white,
        ),
    ]);

    // Row 2 - economics
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
        Span::styled("EPOCH ", label),
        Span::styled(format!("{}", snap.epoch), bold_white),
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

/// Sparkline of finality lag over the last 60 seconds.
fn render_sparkline(f: &mut Frame, area: Rect, state: &TuiState) {
    let data: Vec<u64> = state.sparkline_buf.iter().copied().collect();

    let max_lag = data.iter().copied().max().unwrap_or(1).max(1);

    let current_lag = data.last().copied().unwrap_or(0);
    let lag_color = match current_lag {
        0..=2 => ACCENT,
        3..=10 => WARN,
        _ => ERR,
    };

    let title = format!(
        " Finality Lag (60s)  current: {}  max: {} ",
        current_lag, max_lag
    );

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(DIM))
                .title(Span::styled(
                    title,
                    Style::default().fg(lag_color),
                )),
        )
        .data(&data)
        .max(max_lag.max(5))
        .style(Style::default().fg(lag_color));

    f.render_widget(sparkline, area);
}

/// Peer list table.
fn render_peers(f: &mut Frame, area: Rect, snap: &NodeSnapshot) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            format!(" Peers ({}) ", snap.peer_count),
            Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
        ));

    if snap.peer_addrs.is_empty() {
        let msg = Paragraph::new(Line::from(Span::styled(
            "  Waiting for peer connections...",
            Style::default().fg(DIM),
        )))
        .block(block);
        f.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(Span::styled("", Style::default().fg(LABEL))),
        Cell::from(Span::styled("ADDRESS", Style::default().fg(LABEL))),
        Cell::from(Span::styled("STATUS", Style::default().fg(LABEL))),
    ])
    .height(1);

    let rows: Vec<Row> = snap
        .peer_addrs
        .iter()
        .take(6) // Show up to 6 peers within the border height
        .map(|addr| {
            Row::new(vec![
                Cell::from(Span::styled(
                    " \u{25CF}",
                    Style::default().fg(ACCENT),
                )),
                Cell::from(Span::styled(
                    addr.clone(),
                    Style::default().fg(Color::White),
                )),
                Cell::from(Span::styled(
                    "Connected",
                    Style::default().fg(ACCENT),
                )),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Min(30),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(block);

    f.render_widget(table, area);
}

/// Activity log (scrollable).
fn render_logs(f: &mut Frame, area: Rect, state: &TuiState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            if state.show_logs {
                " Activity Log "
            } else {
                " Activity Log (hidden - press L) "
            },
            Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
        ));

    if !state.show_logs {
        let msg = Paragraph::new(Line::from(Span::styled(
            "  Press L to show logs",
            Style::default().fg(DIM),
        )))
        .block(block);
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = state
        .logs
        .iter()
        .map(|entry| {
            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", entry.timestamp),
                    Style::default().fg(DIM),
                ),
                Span::styled(
                    format!("{} ", entry.level.label()),
                    Style::default()
                        .fg(entry.level.color())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&entry.message, Style::default().fg(Color::White)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut scroll_state = state.log_scroll.clone();
    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Rgb(30, 30, 40)),
        );

    f.render_stateful_widget(list, area, &mut scroll_state);
}

/// Sync progress bar.
fn render_sync_bar(f: &mut Frame, area: Rect, snap: &NodeSnapshot) {
    if snap.sync_complete {
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
    } else {
        let target = snap.peer_max_round.max(snap.current_round).max(1);
        let pct = ((snap.finalized_round as f64 / target as f64) * 100.0)
            .min(99.0)
            .max(0.0) as u16;
        let label = format!(
            " {}% (round {}/{})",
            pct, snap.finalized_round, target
        );
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(WARN))
            .percent(pct)
            .label(Span::styled(
                label,
                Style::default().fg(Color::White),
            ));
        f.render_widget(gauge, area);
    }
}

/// Footer with key hints.
fn render_footer(f: &mut Frame, area: Rect, state: &TuiState) {
    let paused_indicator = if state.paused {
        Span::styled(
            " PAUSED ",
            Style::default()
                .fg(Color::Black)
                .bg(WARN)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };

    let footer = Line::from(vec![
        Span::styled(" q", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(" Quit  ", Style::default().fg(DIM)),
        Span::styled("l", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(" Toggle Logs  ", Style::default().fg(DIM)),
        Span::styled("\u{2191}\u{2193}", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(" Scroll  ", Style::default().fg(DIM)),
        Span::styled("Space", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(" Pause  ", Style::default().fg(DIM)),
        Span::styled("PgUp/PgDn", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(" Page  ", Style::default().fg(DIM)),
        paused_indicator,
    ]);

    f.render_widget(Paragraph::new(footer), area);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format a u64 with thousands separators (1,234,567).
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

// ---------------------------------------------------------------------------
// Tracing bridge: captures tracing events and feeds them into TUI logs
// ---------------------------------------------------------------------------

use std::sync::mpsc;

/// A tracing Layer that sends formatted log entries to the TUI via a channel.
pub struct TuiTracingLayer {
    sender: mpsc::SyncSender<LogEntry>,
}

impl TuiTracingLayer {
    pub fn new(sender: mpsc::SyncSender<LogEntry>) -> Self {
        Self { sender }
    }
}

impl<S> tracing_subscriber::Layer<S> for TuiTracingLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let level = match *event.metadata().level() {
            tracing::Level::ERROR => LogLevel::Error,
            tracing::Level::WARN => LogLevel::Warn,
            tracing::Level::INFO => LogLevel::Info,
            tracing::Level::DEBUG | tracing::Level::TRACE => LogLevel::Debug,
        };

        // Only capture info and above to avoid flooding
        if matches!(level, LogLevel::Debug) {
            return;
        }

        // Format the message
        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);
        let message = if visitor.0.is_empty() {
            format!("{}", event.metadata().name())
        } else {
            visitor.0
        };

        let now = chrono::Local::now();
        let entry = LogEntry {
            timestamp: now.format("%H:%M:%S").to_string(),
            level,
            message,
        };

        // Non-blocking send — drop if channel is full
        let _ = self.sender.try_send(entry);
    }
}

struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{:?}", value);
        } else if !self.0.is_empty() {
            self.0.push_str(&format!(" {}={:?}", field.name(), value));
        } else {
            self.0 = format!("{}={:?}", field.name(), value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0 = value.to_string();
        } else if !self.0.is_empty() {
            self.0.push_str(&format!(" {}={}", field.name(), value));
        } else {
            self.0 = format!("{}={}", field.name(), value);
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Create the tracing layer and log receiver for the TUI.
/// Returns (layer, receiver). The layer should be added to the tracing subscriber.
/// The receiver should be polled by the TUI to drain log entries.
pub fn create_tui_tracing() -> (TuiTracingLayer, mpsc::Receiver<LogEntry>) {
    let (tx, rx) = mpsc::sync_channel(500);
    (TuiTracingLayer::new(tx), rx)
}

/// Initialize and run the TUI.
pub async fn run_tui(server: Arc<NodeServer>, log_rx: Option<mpsc::Receiver<LogEntry>>) -> io::Result<()> {
    let mut app = TuiApp::new(server);

    // Drain logs from the tracing bridge into TUI state
    if let Some(rx) = log_rx {
        let state = app.state.clone();
        tokio::spawn(async move {
            loop {
                match rx.try_recv() {
                    Ok(entry) => {
                        let mut st = state.write().await;
                        st.logs.push_back(entry);
                        while st.logs.len() > MAX_LOG_LINES {
                            st.logs.pop_front();
                        }
                        if st.show_logs {
                            let len = st.logs.len();
                            st.log_scroll.select(Some(len.saturating_sub(1)));
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
        });
    }

    app.run().await
}
