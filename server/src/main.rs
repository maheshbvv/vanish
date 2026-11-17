use std::collections::HashMap;
use std::io::Read;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

type ClientMap = Arc<Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<String>>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bind address: first CLI arg, CHAT_BIND env var, or localhost default.
    // Keep this on 127.0.0.1 when tailcat fronts the server.
    let address = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("CHAT_BIND").ok())
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let port = port_of(&address).unwrap_or(8080);

    // Join key: CHAT_KEY env var, otherwise a fresh random key each boot.
    let key = Arc::new(
        std::env::var("CHAT_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .map(|k| normalize_key(&k))
            .unwrap_or_else(generate_key),
    );

    let listener = TcpListener::bind(&address).await?;
    println!("🚀 Vanish server listening on {address}");
    println!(
        "🔑 Join key (this run only): {}   expires when this server stops",
        display_key(&key)
    );

    // Start tailcat in the background unless disabled with CHAT_TAILCAT=0.
    let mut tailcat: Option<Child> = None;
    if std::env::var("CHAT_TAILCAT").ok().as_deref() != Some("0") {
        println!("⏳ starting tailcat…");
        match start_tailcat(port).await {
            Ok((child, tc_addr)) => {
                println!("🌐 tailcat address: {tc_addr}");
                println!("👉 Guests connect with:  cargo run -- {tc_addr}:{port}");
                println!("   (tailcat runs in the background; it stops with this server)");
                tailcat = Some(child);
            }
            Err(e) => {
                println!("⚠ tailcat not started ({e}). Running local-only.");
                println!("   install with: brew install tailcat   (or set CHAT_TAILCAT=0 to silence)");
            }
        }
    }

    let clients: ClientMap = Arc::new(Mutex::new(HashMap::new()));

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                if let Some(mut child) = tailcat.take() {
                    let _ = child.kill().await;
                }
                println!("👋 shutting down");
                break;
            }
            accepted = listener.accept() => {
                let (stream, _addr) = accepted?;
                let clients = Arc::clone(&clients);
                let key = Arc::clone(&key);
                tokio::spawn(async move {
                    handle_client(stream, clients, key).await;
                });
            }
        }
    }

    Ok(())
}

fn port_of(address: &str) -> Option<u16> {
    address.rsplit(':').next()?.parse().ok()
}

fn extract_tc(line: &str) -> Option<String> {
    line.split_whitespace()
        .map(|t| {
            t.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        })
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

async fn handle_client(stream: TcpStream, clients: ClientMap, key: Arc<String>) {
    let (raw_reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(raw_reader);
    let mut line = String::new();

    // 1. Username
    if writer.write_all(b"Enter your username: \n").await.is_err() {
        return;
    }
    let username = match read_line_trim(&mut reader, &mut line).await {
        Some(u) if !u.is_empty() => u,
        _ => return,
    };

    {
        let clients_lock = clients.lock().await;
        if clients_lock.contains_key(&username) {
            let _ = writer
                .write_all(b"AUTH_FAIL username already in use\n")
                .await;
            return;
        }
    }

    // 2. Join key, up to 3 attempts
    let mut authed = false;
    for attempt in 0..3 {
        if writer.write_all(b"Enter join key: \n").await.is_err() {
            return;
        }
        match read_line_trim(&mut reader, &mut line).await {
            Some(k) if constant_time_eq(&k, &key) => {
                authed = true;
                break;
            }
            Some(_) => {
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

    // 3. Accepted — tell the client, register it, announce it
    if writer.write_all(b"AUTH_OK\n").await.is_err() {
        return;
    }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    {
        let mut clients_lock = clients.lock().await;
        clients_lock.insert(username.clone(), tx);
    }

    println!("🟢 {} connected.", username);
    broadcast(
        &clients,
        &format!("📢 *** {} joined the chat ***\n", username),
    )
    .await;

    // 4. Writer task: broadcast channel -> TCP stream
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if writer.write_all(msg.as_bytes()).await.is_err() {
                break;
            }
        }
    });

    // 5. Read incoming lines from this user and broadcast them
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let formatted = format!("[{}] {}\n", username, trimmed);
                    broadcast(&clients, &formatted).await;
                }
            }
        }
    }

    // 6. Cleanup
    {
        let mut clients_lock = clients.lock().await;
        clients_lock.remove(&username);
    }
    write_task.abort();
    println!("🔴 {} disconnected.", username);
    broadcast(&clients, &format!("📢 *** {} left the chat ***\n", username)).await;
}

async fn read_line_trim(reader: &mut BufReader<OwnedReadHalf>, line: &mut String) -> Option<String> {
    line.clear();
    match reader.read_line(line).await {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}

async fn broadcast(clients: &ClientMap, message: &str) {
    let clients_lock = clients.lock().await;
    for tx in clients_lock.values() {
        let _ = tx.send(message.to_string());
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
        assert_eq!(extract_tc("# Selected bootstrap relay region 302, San Francisco"), None);
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
}
