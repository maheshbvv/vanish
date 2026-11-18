use std::collections::VecDeque;
use std::io;
use std::process::Stdio;
use std::time::Duration;

use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::sync::mpsc;

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(PartialEq)]
enum Phase {
    Login,
    Key,
    Chat,
}

enum MsgKind {
    Chat,
    Me,
    System,
    /// Host-mode invite line (tailcat address guests should paste).
    Invite,
}

struct ChatMessage {
    time: String,
    sender: String,
    content: String,
    kind: MsgKind,
}

struct App {
    address: String,
    phase: Phase,
    input: String,
    username: String,
    messages: Vec<ChatMessage>,
    list_state: ListState,
    connected: bool,
    /// Messages we rendered locally and are still waiting to see echoed by the server.
    pending_self: VecDeque<String>,
    /// Join key injected by the server when it launches the local client (host mode).
    auto_key: Option<String>,
    /// Guest command, when tailcat is running (host mode).
    invite_cmd: Option<String>,
    /// Join key shown in the host's invite line.
    invite_key: Option<String>,
    invite_copied: bool,
    /// Host-mode authority token; enables `/kick <name>`.
    host_token: Option<String>,
    host_claimed: bool,
    auth_error: Option<String>,
    should_quit: bool,
}

enum Action {
    None,
    Send(String),
    Quit,
}

impl App {
    fn new(address: String) -> Self {
        let mut app = Self {
            address,
            phase: Phase::Login,
            input: String::new(),
            username: String::new(),
            messages: Vec::new(),
            list_state: ListState::default(),
            connected: true,
            pending_self: VecDeque::new(),
            auto_key: std::env::args()
                .nth(2)
                .or_else(|| std::env::var("VANISH_KEY").ok())
                .filter(|k| !k.trim().is_empty()),
            invite_cmd: std::env::var("VANISH_INVITE")
                .ok()
                .filter(|s| !s.is_empty()),
            invite_key: std::env::var("VANISH_INVITE_KEY")
                .ok()
                .filter(|s| !s.is_empty()),
            invite_copied: std::env::var("VANISH_INVITE_COPIED").is_ok(),
            host_token: std::env::var("VANISH_HOST_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
            host_claimed: false,
            auth_error: None,
            should_quit: false,
        };
        app.refresh_invite();
        app
    }

    /// Keep the top-of-chat invite line in sync with the current key and
    /// clipboard state (the server sends a fresh key after a kick).
    fn refresh_invite(&mut self) {
        let Some(key) = self.invite_key.clone() else {
            return;
        };
        let content = match (self.invite_copied, self.invite_cmd.clone()) {
            (true, _) => format!("join key {key} · invite copied to clipboard"),
            (false, Some(cmd)) => format!("join key {key} · guests run: {cmd}"),
            (false, None) => format!("join key {key}"),
        };
        if let Some(msg) = self
            .messages
            .iter_mut()
            .find(|msg| matches!(msg.kind, MsgKind::Invite))
        {
            msg.content = content;
        } else {
            self.messages.insert(
                0,
                ChatMessage {
                    time: String::new(),
                    sender: String::new(),
                    content,
                    kind: MsgKind::Invite,
                },
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Quit;
        }
        match key.code {
            KeyCode::Char(c) => {
                self.input.push(c);
                Action::None
            }
            KeyCode::Backspace => {
                self.input.pop();
                Action::None
            }
            KeyCode::Enter => {
                let text = self.input.trim().to_string();
                if text.is_empty() {
                    return Action::None;
                }
                self.input.clear();
                match self.phase {
                    Phase::Login => {
                        self.username = text.clone();
                        self.phase = Phase::Key;
                        match &self.auto_key {
                            // Host mode: the server already handed us the key.
                            Some(key) => Action::Send(format!("{text}\n{key}\n")),
                            None => Action::Send(format!("{text}\n")),
                        }
                    }
                    Phase::Key if self.auto_key.is_some() => Action::None,
                    Phase::Key => Action::Send(format!("{text}\n")),
                    Phase::Chat => {
                        // Host-only command; never echoed as a chat message.
                        if self.host_token.is_some()
                            && let Some(name) = text.strip_prefix("/kick ")
                        {
                            let name = name.trim();
                            if !name.is_empty() {
                                return Action::Send(format!("KICK {name}\n"));
                            }
                        }
                        self.push_local_message(text.clone());
                        Action::Send(format!("{text}\n"))
                    }
                }
            }
            _ => Action::None,
        }
    }

    /// Render our own message immediately so it is visible even before (or
    /// without) the server echoing it back.
    fn push_local_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            time: Local::now().format("%H:%M").to_string(),
            sender: self.username.clone(),
            content: content.clone(),
            kind: MsgKind::Me,
        });
        self.pending_self.push_back(content);
        self.select_last();
    }

    fn select_last(&mut self) {
        self.list_state
            .select(Some(self.messages.len().saturating_sub(1)));
    }

    /// Handle a line from the server. Returns an optional line to send back
    /// (used once to claim the host role after authentication).
    fn push_server_line(&mut self, raw: &str) -> Option<String> {
        let line = raw.trim_end_matches(['\r', '\n']).trim();
        if line.is_empty() {
            return None;
        }

        if let Some(rest) = line.strip_prefix("VANISH_KEY ") {
            let mut parts = rest.split_whitespace();
            if let Some(key) = parts.next() {
                self.invite_key = Some(key.to_string());
            }
            self.invite_copied = parts.next() == Some("1");
            self.refresh_invite();
            return None;
        }
        if let Some(rest) = line.strip_prefix("KICKED") {
            let reason = rest.trim();
            self.auth_error = Some(if reason.is_empty() {
                "removed from the chat".to_string()
            } else {
                reason.to_string()
            });
            self.should_quit = true;
            return None;
        }
        if line.starts_with("AUTH_OK") {
            self.phase = Phase::Chat;
            self.connected = true;
            // Prove host authority once, after the server has registered us.
            if let Some(token) = self.host_token.as_ref().filter(|_| !self.host_claimed) {
                self.host_claimed = true;
                return Some(format!("HOST {token}\n"));
            }
            return None;
        }
        if let Some(rest) = line.strip_prefix("AUTH_FAIL") {
            let reason = rest.trim();
            self.auth_error = Some(if reason.is_empty() {
                "join rejected".to_string()
            } else {
                reason.to_string()
            });
            self.should_quit = true;
            return None;
        }

        // Ignore handshake prompts until we are fully in chat mode.
        if self.phase != Phase::Chat {
            return None;
        }

        let msg = parse_line(line, &self.username);

        // If this is the server echoing a message we already showed locally,
        // drop the duplicate.
        if matches!(msg.kind, MsgKind::Me)
            && let Some(front) = self.pending_self.front()
            && *front == msg.content
        {
            self.pending_self.pop_front();
            return None;
        }

        self.messages.push(msg);
        self.select_last();
        None
    }
}

fn parse_line(line: &str, me: &str) -> ChatMessage {
    let time = Local::now().format("%H:%M").to_string();
    if let Some(rest) = line.strip_prefix('[')
        && let Some(idx) = rest.find("] ")
    {
        let sender = &rest[..idx];
        let content = &rest[idx + 2..];
        let kind = if sender == me {
            MsgKind::Me
        } else {
            MsgKind::Chat
        };
        return ChatMessage {
            time,
            sender: sender.to_string(),
            content: content.to_string(),
            kind,
        };
    }
    ChatMessage {
        time,
        sender: String::new(),
        content: line.to_string(),
        kind: MsgKind::System,
    }
}

fn sender_color(name: &str) -> Color {
    const PALETTE: [Color; 6] = [
        Color::Green,
        Color::Magenta,
        Color::Yellow,
        Color::Blue,
        Color::LightRed,
        Color::LightCyan,
    ];
    let sum: usize = name.bytes().map(|b| b as usize).sum();
    PALETTE[sum % PALETTE.len()]
}

fn message_line(msg: &ChatMessage) -> Line<'static> {
    match msg.kind {
        MsgKind::System => Line::from(Span::styled(
            format!("  {}", msg.content.trim_end()),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )),
        MsgKind::Invite => Line::from(vec![
            Span::styled("  > ", Style::default().fg(Color::Green)),
            Span::styled(
                msg.content.clone(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        MsgKind::Me => Line::from(vec![
            Span::styled(
                format!("{} ", msg.time),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{:<8}", msg.sender),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(msg.content.clone(), Style::default().fg(Color::White)),
        ]),
        MsgKind::Chat => Line::from(vec![
            Span::styled(
                format!("{} ", msg.time),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{:<8}", msg.sender),
                Style::default()
                    .fg(sender_color(&msg.sender))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(msg.content.clone(), Style::default().fg(Color::White)),
        ]),
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.size());

    // Header
    let header_text = match app.phase {
        Phase::Login => Line::from(vec![
            Span::styled("○ connecting  ", Style::default().fg(Color::Yellow)),
            Span::styled(app.address.clone(), Style::default().fg(Color::Gray)),
        ]),
        Phase::Key if app.auto_key.is_some() => Line::from(vec![
            Span::styled("> signing in  ", Style::default().fg(Color::Yellow)),
            Span::styled(app.address.clone(), Style::default().fg(Color::Gray)),
            Span::styled("  ·  user: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.username.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Phase::Key => Line::from(vec![
            Span::styled("> join key required  ", Style::default().fg(Color::Yellow)),
            Span::styled(app.address.clone(), Style::default().fg(Color::Gray)),
            Span::styled("  ·  user: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.username.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Phase::Chat if app.connected => Line::from(vec![
            Span::styled("● connected  ", Style::default().fg(Color::Green)),
            Span::styled(app.address.clone(), Style::default().fg(Color::Gray)),
            Span::styled("  ·  you: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.username.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Phase::Chat => Line::from(Span::styled(
            "○ disconnected",
            Style::default().fg(Color::Red),
        )),
    };
    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                " Vanish ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
    );
    f.render_widget(header, chunks[0]);

    // Messages
    let items: Vec<ListItem> = app
        .messages
        .iter()
        .map(|m| ListItem::new(message_line(m)))
        .collect();
    let list = List::new(items);
    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // Input
    let (title, placeholder) = match app.phase {
        Phase::Login => (" Username ", "type a username and press Enter"),
        Phase::Key if app.auto_key.is_some() => {
            (" Signing in ", "authenticating with the host's key…")
        }
        Phase::Key => (
            " Join key ",
            "paste the join key printed by the server host",
        ),
        Phase::Chat => (" Message ", "type a message and press Enter"),
    };
    let show_placeholder = app.input.is_empty() && app.phase != Phase::Chat;
    let body = if show_placeholder {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                placeholder,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::styled(app.input.clone(), Style::default().fg(Color::White)),
        ])
    };
    let input = Paragraph::new(body).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(title, Style::default().fg(Color::Cyan))),
    );
    f.render_widget(input, chunks[2]);

    // Cursor inside the input box.
    let area = chunks[2];
    let cursor_x = (area.x + 3).saturating_add(app.input.chars().count() as u16);
    let cursor_x = cursor_x.min(area.x + area.width.saturating_sub(2));
    f.set_cursor(cursor_x, area.y + 1);
}

fn init_terminal() -> AppResult<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> AppResult<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn spawn_input_thread() -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false)
                && let Ok(ev) = event::read()
                && tx.send(ev).is_err()
            {
                break;
            }
        }
    });
    rx
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut writer: tokio::net::tcp::OwnedWriteHalf,
    mut server_rx: mpsc::UnboundedReceiver<String>,
    mut key_rx: mpsc::UnboundedReceiver<Event>,
) -> AppResult<()> {
    let mut ticker = tokio::time::interval(Duration::from_millis(200));
    loop {
        terminal.draw(|f| ui(f, app))?;

        tokio::select! {
            maybe_line = server_rx.recv() => {
                match maybe_line {
                    Some(line) => {
                        if let Some(reply) = app.push_server_line(&line) {
                            writer.write_all(reply.as_bytes()).await?;
                        }
                        if app.should_quit {
                            break;
                        }
                    }
                    None => {
                        app.connected = false;
                        break;
                    }
                }
            }
            maybe_event = key_rx.recv() => {
                match maybe_event {
                    Some(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                        match app.handle_key(key) {
                            Action::Quit => break,
                            Action::Send(data) => {
                                writer.write_all(data.as_bytes()).await?;
                            }
                            Action::None => {}
                        }
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            _ = ticker.tick() => {}
        }
    }
    Ok(())
}

/// Split `<tc-address>[:remote_port]`. A bare tc address defaults to port 8080.
fn split_tc(s: &str) -> (String, u16) {
    if let Some(idx) = s.rfind(':')
        && let Ok(port) = s[idx + 1..].parse::<u16>()
    {
        let tc = &s[..idx];
        if tc.starts_with("tc") {
            return (tc.to_string(), port);
        }
    }
    (s.to_string(), 8080)
}

/// Find a free localhost port to expose the tailcat forward on.
fn pick_local_port() -> u16 {
    for port in 18080..18100u16 {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    18080
}

/// Spawn `tailcat forward` in the background. It is killed when the guard drops.
fn start_forward(tc: &str, local: u16, remote: u16) -> AppResult<tokio::process::Child> {
    let mut child = Command::new("tailcat")
        .arg("forward")
        .arg(tc)
        .arg(format!("{local}:{remote}"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            format!("failed to start tailcat forward: {e} — install with `brew install tailcat`")
        })?;

    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(_)) = lines.next_line().await {}
        });
    }
    Ok(child)
}

/// Poll the local forward until it accepts connections.
async fn wait_for_forward(port: u16) -> AppResult<()> {
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err("timed out waiting for tailcat forward to become ready".into())
}

#[tokio::main]
async fn main() -> AppResult<()> {
    // `--version` / `-V` before the address is parsed, so it prints plainly.
    if std::env::args()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-V")
    {
        println!("vanish-client {}", env!("CARGO_PKG_VERSION"));
        if let Some(sha) = option_env!("VANISH_GIT_SHA") {
            println!("commit {sha}");
        }
        return Ok(());
    }

    let self_destruct = std::env::var("VANISH_SELF_DESTRUCT").ok().as_deref() == Some("1")
        || std::env::args().any(|arg| arg == "--vanish" || arg == "--self-destruct");

    // First CLI arg (or CHAT_SERVER):
    //   - `tc...` tailcat address (optionally `:remote_port`) -> auto-run tailcat forward
    //   - `host:port` -> connect directly
    //   - nothing -> 127.0.0.1:8080
    let raw = std::env::args()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .or_else(|| std::env::var("CHAT_SERVER").ok())
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    // Keep the tailcat child alive for the lifetime of the client.
    let (address, _tailcat_guard) = if raw.starts_with("tc") {
        let (tc, remote_port) = split_tc(&raw);
        if tc.len() >= 24 {
            let local_port = pick_local_port();
            eprintln!(
                "tailcat: forwarding to 127.0.0.1:{local_port} (remote port {remote_port})..."
            );
            let guard = start_forward(&tc, local_port, remote_port)?;
            wait_for_forward(local_port).await?;
            (format!("127.0.0.1:{local_port}"), Some(guard))
        } else {
            (raw, None)
        }
    } else {
        (raw, None)
    };

    let stream = match TcpStream::connect(&address).await {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("✗ Could not connect to {address} ({e}).");
            eprintln!("  • Same machine: start `vanish` first.");
            eprintln!(
                "  • Another machine: use the server's LAN IP, e.g. `vanish-client 192.168.1.20:8080`"
            );
            eprintln!("  • tailcat: use the printed address, e.g. `vanish-client tc...:8080`");
            std::process::exit(1);
        }
    };
    let (reader, writer) = stream.into_split();

    // Read incoming server lines into a channel consumed by the UI loop.
    let (line_tx, line_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut buffer = String::new();
        loop {
            buffer.clear();
            match reader.read_line(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if line_tx.send(buffer.clone()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let key_rx = spawn_input_thread();

    // Restore the terminal even if a panic unwinds out of the UI loop.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        default_hook(info);
    }));

    let mut terminal = init_terminal()?;
    let mut app = App::new(address);

    let result = run_app(&mut terminal, &mut app, writer, line_rx, key_rx).await;

    restore_terminal(&mut terminal)?;

    if let Err(e) = result {
        eprintln!("Error: {e}");
    }
    if let Some(reason) = &app.auth_error {
        eprintln!("Join failed: {reason}");
    } else if !app.connected {
        println!("Disconnected from server.");
    } else {
        println!("Goodbye.");
    }

    if self_destruct && let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(&exe);
        println!("vanish: client binary deleted from disk (zero trace).");
    }

    Ok(())
}
