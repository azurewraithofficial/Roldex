use std::io::{self, Stdout};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc as std_mpsc};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::cursor;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use roldex_core::{Agent, Config, ProjectSummary, StudioBroker, WorkspaceFs, project_tree};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

use crate::intent;

const MAX_LOG_ENTRIES: usize = 160;
const INPUT_PLACEHOLDER: &str = "Ask Roldex to build, fix, test, or explain…";

pub struct UiContext {
    pub root: PathBuf,
    pub project: ProjectSummary,
    pub config: Config,
    pub agent: Arc<Mutex<Agent>>,
    pub fs: Arc<WorkspaceFs>,
    pub studio_broker: StudioBroker,
    pub bridge_available: bool,
    pub studio_port: u16,
}

#[derive(Debug, Clone, Copy)]
enum LogKind {
    User,
    Assistant,
    System,
    Error,
}

#[derive(Debug, Clone)]
struct LogEntry {
    kind: LogKind,
    text: String,
}

#[derive(Debug)]
enum WorkerUpdate {
    Progress { id: u64, text: String },
    Done { id: u64, result: Result<String, String> },
}

struct AppState {
    input: String,
    log: Vec<LogEntry>,
    busy: bool,
    status: String,
    started_at: Option<Instant>,
    active_id: Option<u64>,
    next_id: u64,
    spinner_tick: usize,
}

impl AppState {
    fn new(context: &UiContext) -> Self {
        let mut log = Vec::new();
        log.push(LogEntry {
            kind: LogKind::System,
            text: format!(
                "Roldex v{} • {} • {}\nType what you want finished. Roldex will inspect, build, test, repair, and verify it.",
                env!("CARGO_PKG_VERSION"),
                context.project.kind,
                context.config.permissions.mode
            ),
        });
        Self {
            input: String::new(),
            log,
            busy: false,
            status: "Ready".into(),
            started_at: None,
            active_id: None,
            next_id: 1,
            spinner_tick: 0,
        }
    }

    fn push(&mut self, kind: LogKind, text: impl Into<String>) {
        self.log.push(LogEntry {
            kind,
            text: text.into(),
        });
        if self.log.len() > MAX_LOG_ENTRIES {
            let excess = self.log.len() - MAX_LOG_ENTRIES;
            self.log.drain(0..excess);
        }
    }
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    stop_events: Arc<AtomicBool>,
}

impl TerminalSession {
    fn new() -> Result<(Self, std_mpsc::Receiver<Event>)> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableBracketedPaste, cursor::Hide)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let (event_tx, event_rx) = std_mpsc::channel();
        let stop_events = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop_events);
        thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match event::poll(Duration::from_millis(80)) {
                    Ok(true) => match event::read() {
                        Ok(event) => {
                            if event_tx.send(event).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    },
                    Ok(false) => {}
                    Err(_) => break,
                }
            }
        });

        Ok((
            Self {
                terminal,
                stop_events,
            },
            event_rx,
        ))
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.stop_events.store(true, Ordering::Relaxed);
        let _ = self.terminal.show_cursor();
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            DisableBracketedPaste,
            LeaveAlternateScreen,
            cursor::Show
        );
    }
}

pub async fn run(context: UiContext) -> Result<()> {
    let (mut session, event_rx) = TerminalSession::new()?;
    let (event_tx, mut async_events) = mpsc::unbounded_channel::<Event>();
    thread::spawn(move || {
        while let Ok(event) = event_rx.recv() {
            if event_tx.send(event).is_err() {
                break;
            }
        }
    });

    let (worker_tx, mut worker_rx) = mpsc::unbounded_channel::<WorkerUpdate>();
    let mut worker: Option<JoinHandle<()>> = None;
    let mut state = AppState::new(&context);
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    let mut quit = false;

    while !quit {
        draw(&mut session.terminal, &state, &context)?;

        tokio::select! {
            _ = ticker.tick() => {
                state.spinner_tick = state.spinner_tick.wrapping_add(1);
            }
            Some(update) = worker_rx.recv() => {
                match update {
                    WorkerUpdate::Progress { id, text } if state.active_id == Some(id) => {
                        state.status = compact_status(&text);
                    }
                    WorkerUpdate::Done { id, result } if state.active_id == Some(id) => {
                        state.busy = false;
                        state.active_id = None;
                        state.started_at = None;
                        worker = None;
                        match result {
                            Ok(answer) => {
                                state.push(LogKind::Assistant, answer);
                                state.status = "Finished".into();
                            }
                            Err(error) => {
                                state.push(LogKind::Error, friendly_error(&error));
                                state.status = "Request failed".into();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some(event) = async_events.recv() => {
                match event {
                    Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
                            if let Some(handle) = worker.take() {
                                handle.abort();
                            }
                            let cancelled = context.studio_broker.cancel_pending().await;
                            if state.busy {
                                state.push(LogKind::System, format!("Stopped by user. Cancelled {cancelled} queued/pending Studio action(s)."));
                            }
                            quit = true;
                            continue;
                        }

                        match key.code {
                            KeyCode::Esc => {
                                if state.busy {
                                    if let Some(handle) = worker.take() {
                                        handle.abort();
                                    }
                                    let cancelled = context.studio_broker.cancel_pending().await;
                                    state.busy = false;
                                    state.active_id = None;
                                    state.started_at = None;
                                    state.status = "Stopped".into();
                                    state.push(LogKind::System, if cancelled == 0 {
                                        "Stopped by user.".to_string()
                                    } else {
                                        format!("Stopped by user. Cancelled {cancelled} queued/pending Studio action(s).")
                                    });
                                } else if !state.input.is_empty() {
                                    state.input.clear();
                                    state.status = "Input cleared".into();
                                }
                            }
                            KeyCode::Enter if !state.busy => {
                                let input = state.input.trim().to_string();
                                state.input.clear();
                                if input.is_empty() {
                                    continue;
                                }
                                if handle_local_command(&input, &context, &mut state)? {
                                    quit = true;
                                    continue;
                                }
                                state.push(LogKind::User, input.clone());
                                state.busy = true;
                                state.status = "Thinking…".into();
                                state.started_at = Some(Instant::now());
                                let id = state.next_id;
                                state.next_id = state.next_id.wrapping_add(1);
                                state.active_id = Some(id);
                                worker = Some(spawn_agent_request(id, input, &context, worker_tx.clone()));
                            }
                            KeyCode::Backspace if !state.busy => {
                                state.input.pop();
                            }
                            KeyCode::Char(ch) if !state.busy && !key.modifiers.contains(KeyModifiers::CONTROL) => {
                                state.input.push(ch);
                            }
                            _ => {}
                        }
                    }
                    Event::Paste(text) if !state.busy => {
                        state.input.push_str(&text.replace(['\r', '\n'], " "));
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn spawn_agent_request(
    id: u64,
    input: String,
    context: &UiContext,
    updates: mpsc::UnboundedSender<WorkerUpdate>,
) -> JoinHandle<()> {
    let agent = Arc::clone(&context.agent);
    let fs = Arc::clone(&context.fs);
    tokio::spawn(async move {
        let image_paths = intent::detect_image_paths(&input, fs.as_ref());
        let progress_tx = updates.clone();
        let result = {
            let mut agent = agent.lock().await;
            if let Some(arguments) = input.strip_prefix("/image ") {
                let (path, prompt) = match arguments.split_once("::") {
                    Some((path, prompt)) => (path.trim(), prompt.trim()),
                    None => (
                        arguments.trim(),
                        "Analyze this Roblox Studio screenshot or image and fix or improve the relevant project problem.",
                    ),
                };
                agent
                    .chat_with_image(prompt, path, fs.as_ref(), move |event| {
                        let _ = progress_tx.send(WorkerUpdate::Progress {
                            id,
                            text: event.to_string(),
                        });
                    })
                    .await
            } else if image_paths.is_empty() {
                agent
                    .chat_with_tools(&input, fs.as_ref(), move |event| {
                        let _ = progress_tx.send(WorkerUpdate::Progress {
                            id,
                            text: event.to_string(),
                        });
                    })
                    .await
            } else {
                agent
                    .chat_with_images(&input, &image_paths, fs.as_ref(), move |event| {
                        let _ = progress_tx.send(WorkerUpdate::Progress {
                            id,
                            text: event.to_string(),
                        });
                    })
                    .await
            }
        };
        let _ = updates.send(WorkerUpdate::Done {
            id,
            result: result.map_err(|error| format!("{error:#}")),
        });
    })
}

fn handle_local_command(input: &str, context: &UiContext, state: &mut AppState) -> Result<bool> {
    match input {
        "/quit" | "/exit" => return Ok(true),
        "/clear" => {
            state.log.clear();
            state.status = "Cleared".into();
        }
        "/help" => state.push(LogKind::System, help_text()),
        "/status" => state.push(
            LogKind::System,
            format!(
                "{}\nStudio connected: {}",
                context.project.describe(),
                context.studio_broker.is_connected()
            ),
        ),
        "/doctor" => state.push(LogKind::System, doctor_text(context)),
        "/tree" => match project_tree(&context.root, 3, 120) {
            Ok(tree) => state.push(LogKind::System, tree),
            Err(error) => state.push(LogKind::Error, format!("Tree failed: {error:#}")),
        },
        _ if input.starts_with("/read ") => {
            let path = input.trim_start_matches("/read ").trim();
            match context.fs.read_text(path) {
                Ok(text) => state.push(LogKind::System, text),
                Err(error) => state.push(LogKind::Error, format!("Read failed: {error:#}")),
            }
        }
        _ => return Ok(false),
    }
    Ok(false)
}

fn doctor_text(context: &UiContext) -> String {
    let ai_key = std::env::var_os(&context.config.ai.api_key_env).is_some();
    let media_key = std::env::var_os("POLLINATIONS_API_KEY").is_some();
    let git_available = Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    let mut lines = vec![
        "Roldex doctor".to_string(),
        format!("OS/arch: {}/{}", std::env::consts::OS, std::env::consts::ARCH),
        format!("Project: {} ({})", context.project.root.display(), context.project.kind),
        format!("Permissions: {}", context.config.permissions.mode),
        format!(
            "AI key {}: {}",
            context.config.ai.api_key_env,
            if ai_key { "configured" } else { "missing" }
        ),
        format!(
            "Media key POLLINATIONS_API_KEY: {}",
            if media_key { "configured" } else { "optional / missing" }
        ),
        format!("Git: {}", if git_available { "available" } else { "not found" }),
        format!(
            "Studio bridge: {} on 127.0.0.1:{}",
            if context.bridge_available { "running" } else { "not running" },
            context.studio_port
        ),
        format!(
            "Studio plugin: {}",
            if context.studio_broker.is_connected() { "connected" } else { "not connected" }
        ),
    ];

    #[cfg(windows)]
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let base = PathBuf::from(local_app_data).join("Roblox").join("Plugins");
        for name in ["RoldexStudio.plugin.lua", "RoldexStudioRuntime.plugin.lua"] {
            let path = base.join(name);
            lines.push(format!(
                "{}: {}",
                name,
                if path.exists() { "installed" } else { "not found" }
            ));
        }
    }

    lines.join("\n")
}

fn help_text() -> &'static str {
    "Just type what you want in normal language. Roldex chooses tools itself.\n\nExamples:\n• build a polished lobby in the open Studio place and test it\n• fix my datastore system and keep testing until it passes\n• make this inventory panel work on desktop and phone\n• inspect this screenshot and fix what is wrong\n\nShortcuts:\n/help   show this help\n/doctor check setup\n/status project + Studio status\n/tree   show project tree\n/read <path> read a file\n/clear  clear transcript\n/quit   exit\n\nEsc stops the active response/task. Ctrl+C exits Roldex."
}

fn compact_status(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return "Working…".into();
    }
    let mut text = raw.to_string();
    if text.len() > 70 {
        text.truncate(67);
        text.push('…');
    }
    if !text.ends_with('…') && !text.ends_with('.') {
        text.push('…');
    }
    text
}

fn friendly_error(error: &str) -> String {
    if error.contains("OPENROUTER_API_KEY") && error.contains("missing") {
        return "AI key is missing. Set OPENROUTER_API_KEY in Windows, then restart Roldex.".into();
    }
    if error.to_ascii_lowercase().contains("timed out") {
        return format!("The request timed out instead of hanging forever. You can retry; Roldex kept the session usable.\n\n{error}");
    }
    format!("Request failed: {error}")
}

fn draw(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &AppState,
    context: &UiContext,
) -> Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(4),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(area);

        let studio = if context.studio_broker.is_connected() {
            Span::styled("Studio connected", Style::default().fg(Color::Green))
        } else {
            Span::styled("Studio offline", Style::default().fg(Color::DarkGray))
        };
        let header = Line::from(vec![
            Span::styled(
                "roldex",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                context.project.kind.to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  •  "),
            Span::styled(
                context.config.permissions.mode.to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  •  "),
            studio,
        ]);
        frame.render_widget(Paragraph::new(header), chunks[0]);

        let body_lines = render_log(&state.log);
        let body_height = chunks[1].height.saturating_sub(1) as usize;
        let estimated = body_lines.len();
        let scroll = estimated.saturating_sub(body_height) as u16;
        let body = Paragraph::new(body_lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0));
        frame.render_widget(body, chunks[1]);

        let (input_title, input_text, border_style) = if state.busy {
            let spinner = ["·", "•", "●", "•"][state.spinner_tick % 4];
            (
                format!(" {spinner} {} ", state.status),
                "Esc to stop the active task".to_string(),
                Style::default().fg(Color::Magenta),
            )
        } else {
            let shown = if state.input.is_empty() {
                Span::styled(INPUT_PLACEHOLDER, Style::default().fg(Color::DarkGray))
            } else {
                Span::raw(format!("{}▏", visible_input_tail(&state.input, chunks[2].width)))
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Send a message ");
            frame.render_widget(Paragraph::new(Line::from(shown)).block(block), chunks[2]);
            let footer = Line::from(vec![
                Span::styled("enter", Style::default().fg(Color::DarkGray)),
                Span::raw(" send  •  "),
                Span::styled("esc", Style::default().fg(Color::DarkGray)),
                Span::raw(" clear  •  "),
                Span::styled("ctrl+c", Style::default().fg(Color::DarkGray)),
                Span::raw(" exit  •  "),
                Span::styled("/help", Style::default().fg(Color::DarkGray)),
            ]);
            frame.render_widget(Paragraph::new(footer), chunks[3]);
            return;
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .title(input_title);
        frame.render_widget(
            Paragraph::new(Span::styled(input_text, Style::default().fg(Color::DarkGray)))
                .block(block),
            chunks[2],
        );
        let elapsed = state
            .started_at
            .map(|started| format!("  •  {}s", started.elapsed().as_secs()))
            .unwrap_or_default();
        let footer = Line::from(vec![
            Span::styled("esc", Style::default().fg(Color::Magenta)),
            Span::raw(" stop  •  "),
            Span::styled(&state.status, Style::default().fg(Color::DarkGray)),
            Span::styled(elapsed, Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(footer), chunks[3]);
    })?;
    Ok(())
}

fn render_log(log: &[LogEntry]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for entry in log {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        let (label, label_style) = match entry.kind {
            LogKind::User => (
                "you",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            LogKind::Assistant => (
                "roldex",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            LogKind::System => ("", Style::default().fg(Color::DarkGray)),
            LogKind::Error => (
                "error",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        };
        if !label.is_empty() {
            lines.push(Line::from(Span::styled(label.to_string(), label_style)));
        }
        for raw in entry.text.lines() {
            let style = match entry.kind {
                LogKind::System => Style::default().fg(Color::DarkGray),
                LogKind::Error => Style::default().fg(Color::Red),
                _ if raw.trim_start().starts_with('✓') => Style::default().fg(Color::Green),
                _ if raw.trim_start().starts_with('•') => Style::default().fg(Color::Cyan),
                _ if raw.trim_start().starts_with("```") => Style::default().fg(Color::DarkGray),
                _ => Style::default(),
            };
            lines.push(Line::from(Span::styled(raw.to_string(), style)));
        }
    }
    lines
}

fn visible_input_tail(input: &str, area_width: u16) -> String {
    let max = area_width.saturating_sub(4) as usize;
    if input.chars().count() <= max {
        return input.to_string();
    }
    input
        .chars()
        .rev()
        .take(max.saturating_sub(1))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>()
        .prepend_ellipsis()
}

trait PrependEllipsis {
    fn prepend_ellipsis(self) -> String;
}

impl PrependEllipsis for String {
    fn prepend_ellipsis(self) -> String {
        format!("…{self}")
    }
}
