use std::collections::HashMap;
use std::io::{IsTerminal, Read};
use std::net::{IpAddr, SocketAddr};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};

/// Join-key guessing policy: per source IP, across connections.
const JOIN_WINDOW: Duration = Duration::from_secs(60);
const MAX_JOIN_FAILURES: u32 = 5;
const JOIN_BLOCK: Duration = Duration::from_secs(60);

enum Out {
    Text(String),
    Close,
}

struct ClientEntry {
    /// Unique per connection, so a kicked user's cleanup cannot remove a
    /// reconnected client that reused the username.
    id: u64,
    tx: mpsc::UnboundedSender<Out>,
    kick: Option<oneshot::Sender<()>>,
}

struct Hub {
    clients: Mutex<HashMap<String, ClientEntry>>,
    key: RwLock<String>,
    /// Secret handed to the host's local client; required to kick.
    host_token: String,
    /// Public tailcat address, if one is running.
    share: Option<String>,
    limiter: Mutex<JoinLimiter>,
    quiet: bool,
}

#[derive(Default)]
struct JoinLimiter {
    failures: HashMap<IpAddr, (u32, Instant)>,
    blocked: HashMap<IpAddr, Instant>,
}

enum Admission {
    Allowed,
    Blocked { retry_after: Duration },
}

impl JoinLimiter {
    fn admit(&mut self, ip: IpAddr, now: Instant) -> Admission {
        if let Some(&until) = self.blocked.get(&ip) {
            if now < until {
                return Admission::Blocked {
                    retry_after: until - now,
                };
            }
            self.blocked.remove(&ip);
        }
        Admission::Allowed
    }

    /// Record a wrong key, returning the number of failures in the window.
    fn record_failure(&mut self, ip: IpAddr, now: Instant) -> u32 {
        let entry = self.failures.entry(ip).or_insert((0, now));
        if now.saturating_duration_since(entry.1) > JOIN_WINDOW {
            *entry = (0, now);
        }
        entry.0 += 1;
        let count = entry.0;
        if count >= MAX_JOIN_FAILURES {
            self.blocked.insert(ip, now + JOIN_BLOCK);
            self.failures.remove(&ip);
        }
        count
    }

    fn record_success(&mut self, ip: IpAddr) {
        self.failures.remove(&ip);
        self.blocked.remove(&ip);
    }
}

/// Slow guessing down even before the block kicks in.
fn failure_delay(failures: u32) -> Duration {
    Duration::from_millis(250 * u64::from(failures.min(4)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `--version` / `-V` before anything else, so it prints plainly.
    if std::env::args()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-V")
    {
        println!("vanish-server {}", env!("CARGO_PKG_VERSION"));
        if let Some(sha) = option_env!("VANISH_GIT_SHA") {
            println!("commit {sha}");
        }
        return Ok(());
    }

    // Bind address: first CLI arg, CHAT_BIND env var, or localhost default.
    // Keep this on 127.0.0.1 when tailcat fronts the server.
    let address = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("CHAT_BIND").ok())
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let port = port_of(&address).unwrap_or(8080);

    // Join key: CHAT_KEY env var, otherwise a fresh random key each boot.
    let key = std::env::var("CHAT_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .map(|k| normalize_key(&k))
        .unwrap_or_else(generate_key);

    // Only the host's local client gets this, and it is required to kick.
    let host_token = format!("{}{}", generate_key(), generate_key());

    let listener = TcpListener::bind(&address).await?;
    println!("🚀 Vanish server listening on {address}");
    println!(
        "🔑 Join key (this run only): {}   expires when this server stops",
        display_key(&key)
    );

    // Start tailcat in the background unless disabled with CHAT_TAILCAT=0.
    let mut tailcat: Option<Child> = None;
    let mut share: Option<String> = None;
    if std::env::var("CHAT_TAILCAT").ok().as_deref() != Some("0") {
        println!("⏳ starting tailcat…");
        match start_tailcat(port).await {
            Ok((child, tc_addr)) => {
                println!("🌐 tailcat address: {tc_addr}");
                println!("👉 Guests connect with:  cargo run -p vanish-client -- {tc_addr}:{port}");
                println!("   (tailcat runs in the background; it stops with this server)");
                share = Some(format!("{tc_addr}:{port}"));
                tailcat = Some(child);
            }
            Err(e) => {
                println!("⚠ tailcat not started ({e}). Running local-only.");
                println!(
                    "   install with: brew install tailcat   (or set CHAT_TAILCAT=0 to silence)"
                );
            }
        }
    }

    // Host mode: open a local client in this terminal. When it exits, the
    // server shuts down too, so the chat ends with the host.
    // Disable with CHAT_HOST=0; skipped automatically when stdin isn't a TTY.
    let host_mode =
        std::env::var("CHAT_HOST").ok().as_deref() != Some("0") && std::io::stdin().is_terminal();

    // The TUI clears the screen, so hand the join key (plus the guest command
    // when tailcat is active) to the client. Copy the full invite to the
    // clipboard when a clipboard tool is available.
    let invite_key = display_key(&key);
    let invite = share
        .as_ref()
        .map(|addr| format!("cargo run -p vanish-client -- {addr}"));
    let invite_copied = invite
        .as_ref()
        .map(|cmd| {
            copy_to_clipboard(&format!(
                "Join my Vanish chat:\n{cmd}\nJoin key: {invite_key}"
            ))
        })
        .unwrap_or(false);
    if invite_copied {
        println!("📋 invite + join key copied to your clipboard");
    }

    let (host_tx, mut host_rx) = tokio::sync::mpsc::channel::<()>(1);
    let mut hosting = false;
    if host_mode {
        match spawn_local_client(
            port,
            &key,
            &host_token,
            invite.as_deref(),
            &invite_key,
            invite_copied,
        ) {
            Ok(mut child) => {
                hosting = true;
                let tx = host_tx.clone();
                tokio::spawn(async move {
                    let _ = child.wait().await;
                    let _ = tx.send(()).await;
                });
            }
            Err(e) => {
                eprintln!("⚠ could not start local client ({e}). Running headless.");
            }
        }
    }
    drop(host_tx);

    let hub = Arc::new(Hub {
        clients: Mutex::new(HashMap::new()),
        key: RwLock::new(key),
        host_token,
        share,
        limiter: Mutex::new(JoinLimiter::default()),
        quiet: hosting,
    });
    let next_id = AtomicU64::new(1);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                if let Some(mut child) = tailcat.take() {
                    let _ = child.kill().await;
                }
                println!("👋 shutting down");
                break;
            }
            _ = host_rx.recv(), if hosting => {
                println!("👋 host left the chat — shutting down");
                if let Some(mut child) = tailcat.take() {
                    let _ = child.kill().await;
                }
                break;
            }
            accepted = listener.accept() => {
                let (stream, addr) = accepted?;
                let hub = Arc::clone(&hub);
                let id = next_id.fetch_add(1, Ordering::Relaxed);
                tokio::spawn(async move {
                    handle_client(stream, addr, id, hub).await;
                });
            }
        }
    }

    Ok(())
}

fn port_of(address: &str) -> Option<u16> {
    address.rsplit(':').next()?.parse().ok()
}

/// Launch the local client attached to this terminal. Prefers the sibling
/// `vanish-client` binary, building it quietly on first use if needed.
fn spawn_local_client(
    port: u16,
    key: &str,
    host_token: &str,
    invite: Option<&str>,
    invite_key: &str,
    invite_copied: bool,
) -> Result<Child, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "cannot locate executable directory".to_string())?
        .to_path_buf();
    let bin = if cfg!(windows) {
        dir.join("vanish-client.exe")
    } else {
        dir.join("vanish-client")
    };

    if !bin.exists() {
        let root = dir.parent().and_then(|target| target.parent());
        if let Some(root) = root {
            eprintln!("… building vanish-client (first run)");
            let status = std::process::Command::new("cargo")
                .args(["build", "-q", "-p", "vanish-client"])
                .current_dir(root)
                .status()
                .map_err(|e| format!("could not run cargo: {e}"))?;
            if !status.success() {
                return Err("building vanish-client failed".to_string());
            }
        }
    }
    if !bin.exists() {
        return Err("vanish-client binary not found".to_string());
    }

    let mut cmd = Command::new(&bin);
    cmd.arg(format!("127.0.0.1:{port}"))
        .env("VANISH_KEY", key)
        .env("VANISH_HOST_TOKEN", host_token)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    if let Some(invite) = invite {
        cmd.env("VANISH_INVITE", invite);
    }
    cmd.env("VANISH_INVITE_KEY", invite_key);
    if invite_copied {
        cmd.env("VANISH_INVITE_COPIED", "1");
    }
    cmd.spawn()
        .map_err(|e| format!("could not launch client: {e}"))
}

/// Best-effort clipboard copy. Linux needs wl-copy, xclip, or xsel installed.
fn copy_to_clipboard(text: &str) -> bool {
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ]
    };
    for (program, args) in candidates {
        let mut child = match std::process::Command::new(program)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => continue,
        };
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().map(|status| status.success()).unwrap_or(false) {
            return true;
        }
    }
    false
}

fn extract_tc(line: &str) -> Option<String> {
    line.split_whitespace()
        .map(|t| t.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')))
        .find(|t| t.starts_with("tc") && t.len() >= 24)
        .map(|t| t.to_string())
}

async fn start_tailcat(port: u16) -> Result<(Child, String), String> {
    let mut child = Command::new("tailcat")
        .arg("serve")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("failed to launch tailcat: {e}"))?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "tailcat produced no output stream".to_string())?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    loop {
        let next = tokio::time::timeout(Duration::from_secs(20), rx.recv()).await;
        let line = match next {
            Ok(Some(line)) => line,
            Ok(None) => return Err("tailcat exited before printing an address".to_string()),
            Err(_) => return Err("timed out waiting for a tailcat address".to_string()),
        };
        if let Some(addr) = extract_tc(&line) {
            return Ok((child, addr));
        }
    }
}

async fn handle_client(stream: TcpStream, addr: SocketAddr, id: u64, hub: Arc<Hub>) {
    let ip = addr.ip();
    let (raw_reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(raw_reader);
    let mut line = String::new();

    // 0. Rate limit: refuse blocked sources before prompting for anything.
    {
        let mut limiter = hub.limiter.lock().await;
        if let Admission::Blocked { retry_after } = limiter.admit(ip, Instant::now()) {
            let secs = retry_after.as_secs().max(1);
            let _ = writer
                .write_all(
                    format!("AUTH_FAIL too many wrong join keys — try again in {secs}s\n")
                        .as_bytes(),
                )
                .await;
            return;
        }
    }

    // 1. Username
    if writer.write_all(b"Enter your username: \n").await.is_err() {
        return;
    }
    let username = match read_line_trim(&mut reader, &mut line).await {
        Some(u) if !u.is_empty() && u.len() <= 32 => u,
        _ => return,
    };

    // 2. Join key, up to 3 attempts. Failures accumulate per source IP across
    //    connections, so reconnecting does not reset the budget.
    let mut authed = false;
    for attempt in 0..3 {
        if writer.write_all(b"Enter join key: \n").await.is_err() {
            return;
        }
        match read_line_trim(&mut reader, &mut line).await {
            Some(k) => {
                let expected = hub.key.read().await.clone();
                if constant_time_eq(&k, &expected) {
                    hub.limiter.lock().await.record_success(ip);
                    authed = true;
                    break;
                }
                let failures = hub.limiter.lock().await.record_failure(ip, Instant::now());
                tokio::time::sleep(failure_delay(failures)).await;
                if attempt == 2 {
                    let _ = writer.write_all(b"AUTH_FAIL wrong join key\n").await;
                }
            }
            None => return,
        }
    }
    if !authed {
        return;
    }

    // 3. Username uniqueness is checked only after the key is proven, so
    //    unauthenticated peers cannot squat on names.
    {
        let clients = hub.clients.lock().await;
        if clients.contains_key(&username) {
            let _ = writer
                .write_all(b"AUTH_FAIL username already in use\n")
                .await;
            return;
        }
    }

    // 4. Accepted — tell the client, register it, announce it
    if writer.write_all(b"AUTH_OK\n").await.is_err() {
        return;
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<Out>();
    let (kick_tx, mut kick_rx) = oneshot::channel::<()>();
    {
        let mut clients = hub.clients.lock().await;
        clients.insert(
            username.clone(),
            ClientEntry {
                id,
                tx: tx.clone(),
                kick: Some(kick_tx),
            },
        );
    }

    if !hub.quiet {
        println!("🟢 {} connected.", username);
    }
    broadcast(&hub, &format!("📢 *** {} joined the chat ***\n", username)).await;

    // 5. Writer task: channel -> TCP stream. `Close` ends the connection.
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                Out::Text(text) => {
                    if writer.write_all(text.as_bytes()).await.is_err() {
                        break;
                    }
                }
                Out::Close => break,
            }
        }
    });

    // 6. Read incoming lines. The host can claim its role and kick users.
    let mut is_host = false;
    let mut kicked = false;
    loop {
        line.clear();
        tokio::select! {
            _ = &mut kick_rx => {
                kicked = true;
                break;
            }
            read = reader.read_line(&mut line) => {
                match read {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Some(token) = trimmed.strip_prefix("HOST ") {
                            if constant_time_eq(token.trim(), &hub.host_token) {
                                is_host = true;
                            }
                            continue;
                        }
                        if let Some(target) = trimmed.strip_prefix("KICK ") {
                            if is_host {
                                handle_kick(&hub, &username, target.trim(), &tx).await;
                            }
                            continue;
                        }
                        let formatted = format!("[{}] {}\n", username, trimmed);
                        broadcast(&hub, &formatted).await;
                    }
                }
            }
        }
    }

    // 7. Cleanup. Only remove our own entry: a kicked user may have reconnected
    //    under the same name before this task noticed.
    {
        let mut clients = hub.clients.lock().await;
        if clients.get(&username).map(|entry| entry.id) == Some(id) {
            clients.remove(&username);
        }
    }
    write_task.abort();
    if !hub.quiet {
        println!("🔴 {} disconnected.", username);
    }
    if kicked {
        broadcast(
            &hub,
            &format!("📢 *** {} was removed by the host ***\n", username),
        )
        .await;
    } else {
        broadcast(&hub, &format!("📢 *** {} left the chat ***\n", username)).await;
    }
}

/// Disconnect a guest and rotate the join key so the old invite cannot be
/// reused. Only reachable from a connection that proved the host token.
async fn handle_kick(hub: &Hub, host: &str, target: &str, host_tx: &mpsc::UnboundedSender<Out>) {
    if target.is_empty() {
        let _ = host_tx.send(Out::Text("usage: /kick <username>\n".to_string()));
        return;
    }
    if target == host {
        let _ = host_tx.send(Out::Text("you cannot remove yourself\n".to_string()));
        return;
    }

    let entry = hub.clients.lock().await.remove(target);
    let Some(entry) = entry else {
        let _ = host_tx.send(Out::Text(format!("no connected user named {target}\n")));
        return;
    };

    let _ = entry
        .tx
        .send(Out::Text("KICKED removed by the host\n".to_string()));
    let _ = entry.tx.send(Out::Close);
    if let Some(kick) = entry.kick {
        let _ = kick.send(());
    }

    // Rotate the key: the removed person knows the old one.
    let new_key = generate_key();
    let display = display_key(&new_key);
    *hub.key.write().await = new_key;
    let copied = match hub.share.as_ref() {
        Some(addr) => {
            let cmd = format!("cargo run -p vanish-client -- {addr}");
            copy_to_clipboard(&format!("Join my Vanish chat:\n{cmd}\nJoin key: {display}"))
        }
        None => copy_to_clipboard(&format!("Join key: {display}")),
    };
    let _ = host_tx.send(Out::Text(format!(
        "VANISH_KEY {display} {}\n",
        u8::from(copied)
    )));

    if !hub.quiet {
        println!("🔑 removed {target}; new join key: {display}");
    }
}

async fn read_line_trim(
    reader: &mut BufReader<OwnedReadHalf>,
    line: &mut String,
) -> Option<String> {
    line.clear();
    match reader.read_line(line).await {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}

async fn broadcast(hub: &Hub, message: &str) {
    let clients = hub.clients.lock().await;
    for entry in clients.values() {
        let _ = entry.tx.send(Out::Text(message.to_string()));
    }
}

/// Strip separators and uppercase, so "abcd-efgh" == "ABCDEFGH".
fn normalize_key(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Group into blocks of 4 for readability: "ABCD-EFGH".
fn display_key(key: &str) -> String {
    let chunks: Vec<String> = key
        .chars()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|c| c.iter().collect())
        .collect();
    chunks.join("-")
}

/// 8 random Crockford-base32 chars (~40 bits). Reads the OS RNG.
fn generate_key() -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut bytes = [0u8; 8];
    let ok = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_ok();
    if !ok {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mixed = (nanos as u64) ^ ((std::process::id() as u64) << 32);
        bytes = mixed.to_le_bytes();
    }
    bytes
        .iter()
        .map(|b| ALPHABET[(*b % 32) as usize] as char)
        .collect()
}

/// Compare without short-circuiting on the first differing byte.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = normalize_key(a);
    let b = normalize_key(b);
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tc_address_from_log_line() {
        let line = "# 🐈 Server listening with new address: tcomFwWCCcjS5nKNqAod034nWoJZW0LZq";
        assert_eq!(
            extract_tc(line).as_deref(),
            Some("tcomFwWCCcjS5nKNqAod034nWoJZW0LZq")
        );
    }

    #[test]
    fn extracts_tc_address_from_saved_key_line() {
        let line = "🐈 Server listening with saved key \"default\": tcZZZZZZZZZZZZZZZZZZZZZZZZ";
        assert_eq!(
            extract_tc(line).as_deref(),
            Some("tcZZZZZZZZZZZZZZZZZZZZZZZZ")
        );
    }

    #[test]
    fn ignores_lines_without_address() {
        assert_eq!(
            extract_tc("# Selected bootstrap relay region 302, San Francisco"),
            None
        );
        assert_eq!(extract_tc("tcSHORT"), None);
    }

    #[test]
    fn parses_port_from_bind_address() {
        assert_eq!(port_of("127.0.0.1:8080"), Some(8080));
        assert_eq!(port_of("0.0.0.0:9099"), Some(9099));
        assert_eq!(port_of("nonsense"), None);
    }

    #[test]
    fn key_normalization_and_compare() {
        assert_eq!(normalize_key("abcd-efgh"), "ABCDEFGH");
        assert!(constant_time_eq("test-key", "TESTKEY"));
        assert!(!constant_time_eq("TESTKEY", "WRONG"));
    }

    #[test]
    fn limiter_blocks_after_repeated_failures_and_expires() {
        let mut limiter = JoinLimiter::default();
        let ip: IpAddr = "203.0.113.7".parse().unwrap();
        let t0 = Instant::now();

        assert!(matches!(limiter.admit(ip, t0), Admission::Allowed));
        for _ in 0..MAX_JOIN_FAILURES {
            limiter.record_failure(ip, t0);
        }
        assert!(matches!(
            limiter.admit(ip, t0),
            Admission::Blocked { retry_after } if retry_after > Duration::ZERO
        ));
        assert!(matches!(
            limiter.admit(ip, t0 + JOIN_BLOCK + Duration::from_secs(1)),
            Admission::Allowed
        ));
    }

    #[test]
    fn limiter_resets_after_window_and_on_success() {
        let mut limiter = JoinLimiter::default();
        let ip: IpAddr = "203.0.113.8".parse().unwrap();
        let t0 = Instant::now();

        for _ in 0..MAX_JOIN_FAILURES - 1 {
            limiter.record_failure(ip, t0);
        }
        // Old failures outside the window no longer count.
        let later = t0 + JOIN_WINDOW + Duration::from_secs(1);
        let count = limiter.record_failure(ip, later);
        assert_eq!(count, 1);
        assert!(matches!(limiter.admit(ip, later), Admission::Allowed));

        // A successful join clears that IP's history.
        limiter.record_success(ip);
        assert!(matches!(limiter.admit(ip, later), Admission::Allowed));
        assert_eq!(limiter.record_failure(ip, later), 1);
    }

    #[test]
    fn failure_delay_grows_but_is_capped() {
        assert_eq!(failure_delay(1), Duration::from_millis(250));
        assert_eq!(failure_delay(2), Duration::from_millis(500));
        assert_eq!(failure_delay(9), Duration::from_millis(1000));
    }
}
