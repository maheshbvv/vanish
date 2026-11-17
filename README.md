# Vanish

A terminal-based secret chat. Self-hosted, gated by a one-session join key, with an
optional WireGuard-encrypted tunnel ([tailcat](https://github.com/tailscale/tailcat)) —
no accounts, no database, no inbound ports.

```
┌ Vanish ──────────────────────────────────────┐
│● connected  127.0.0.1:8080  ·  you: alice     │
└──────────────────────────────────────────────┘
  📢  *** alice joined the chat ***
00:45 alice   hello everyone
00:45 bob     hi alice
┌ Message ─────────────────────────────────────┐
│> ▊                                            │
└──────────────────────────────────────────────┘
```

## Features

- **Join key per boot.** The server prints a random key at startup. Guests cannot
  join without it. Restart the server and the old key is dead — nothing is persisted.
- **Polished TUI.** Fixed input box, timestamps, per-user colors, own messages
  highlighted, join/leave events dimmed, and instant local echo of what you send.
- **tailcat on autopilot.** `cargo run` starts tailcat in the background and prints a
  single paste-able command for guests. The client auto-establishes the tunnel.
- **Tiny and dependency-light.** Tokio TCP server; ratatui/crossterm client.

## Requirements

- Rust 1.85+ (both crates use edition 2024).
- Optional: [tailcat](https://github.com/tailscale/tailcat) for chatting across machines
  (`brew install tailcat`). Without it Vanish still runs on localhost.

## Build

```bash
cargo build --release
```

This is a Cargo workspace and builds both binaries:
`target/release/vanish-server` and `target/release/vanish-client`.

## Quick start (same machine)

Terminal 1 — server:

```bash
cargo run -p vanish-server
# 🚀 Vanish server listening on 127.0.0.1:8080
# 🔑 Join key (this run only): V9FB-6NCH   expires when this server stops
```

Terminal 2 and 3 — clients:

```bash
cargo run -p vanish-client
```

Each client asks for a **username** and the **join key**, then drops you into the chat.
Open more terminals for more people; usernames must be unique while connected.

## Chatting across machines (tailcat, automatic)

The server starts tailcat for you and prints a ready-to-share line:

```bash
cargo run -p vanish-server
# 🌐 tailcat address: tcomFwWCCcjS5nKNqAod034nWoJZW0LZq
# 👉 Guests connect with:  cargo run -- tcomFwWCCcjS5nKNqAod034nWoJZW0LZq:8080
```

A guest pastes that command:

```bash
cargo run -p vanish-client -- tcomFwWCCcjS5nKNqAod034nWoJZW0LZq:8080
```

The client detects the `tc...` address, runs `tailcat forward` in the background, and
connects through the encrypted tunnel. tailcat must be installed on both machines.

If tailcat is not installed the server falls back to local-only and tells you how to
install it. Disable auto-tailcat explicitly with `CHAT_TAILCAT=0`.

## LAN without tailcat

```bash
# server (listen on all interfaces)
cargo run -p vanish-server -- 0.0.0.0:8080

# client
cargo run -p vanish-client -- 192.168.1.20:8080
```

## Configuration

Server:

| Setting | How | Default |
|---|---|---|
| Bind address | CLI arg, or `CHAT_BIND` | `127.0.0.1:8080` |
| Join key | `CHAT_KEY` env (pins it — dev only) | random 8 chars per boot |
| tailcat | `CHAT_TAILCAT=0` to disable | auto-start |

Client:

| Setting | How | Default |
|---|---|---|
| Target | CLI arg, or `CHAT_SERVER`; accepts `host:port`, `tc...`, `tc...:port` | `127.0.0.1:8080` |

The join key is normalized: `abcd-efgh`, `ABCD EFGH`, and `ABCDEFGH` are equivalent.

## Security model

- **Join key.** 8 random characters (~40 bits) from the OS RNG, regenerated every boot,
  never written to disk, compared in constant time, with 3 attempts per connection.
  It is a session bearer token, not a user identity.
- **Transport.** Over localhost the traffic is plain TCP. Across machines, run it through
  tailcat (or Tailscale): the tunnel is WireGuard-encrypted, the tailcat address is
  itself a secret capability, and no inbound port is exposed.
- **Be honest about limits:** plain TCP carries the join key and messages in cleartext.
  Do not expose a bare `0.0.0.0` socket to untrusted networks — put tailcat in front.
  There is no per-user authentication, no message persistence, and no app-layer
  encryption beyond whatever transport you provide.

## How it works

- **Server** (`server/`): a Tokio TCP listener. Per connection it asks for a username,
  then a join key (3 tries), then sends `AUTH_OK` and registers the client. Every chat
  line is broadcast to all connected clients as `[username] message`; system events are
  sent as `📢 ...` lines. Duplicate usernames are rejected.
- **Client** (`client/`): a ratatui TUI driven by a Tokio event loop. It renders its own
  message immediately on send and skips the server's echo of it, so the chat feels
  instant and never double-prints. A background thread feeds keyboard events; a reader
  task feeds server lines.
- **tailcat glue**: the server spawns `tailcat serve <port>`, parses the address from its
  output, and kills it on shutdown. The client spawns `tailcat forward` for `tc...`
  targets and connects to the resulting local port.

## Keyboard

| Key | Action |
|---|---|
| Enter | submit username / key / message |
| Backspace | edit input |
| Ctrl+C | quit (terminal is restored) |

## Project layout

```
Cargo.toml        workspace root
server/           vanish-server: Tokio TCP chat server
client/           vanish-client: ratatui terminal client
```

## Tests

```bash
cargo test -p vanish-server
```

Covers tailcat address parsing, bind-port parsing, and join-key normalization/comparison.

## Limitations

- No message history or persistence — restart and it is gone ("vanish").
- No file transfer, typing indicators, or rooms.
- A readable browser URL is not possible with tailcat alone; use a DNS `TXT` record or a
  different tunnel if you need one.
