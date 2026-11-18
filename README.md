# Vanish

A terminal-based secret chat. Self-hosted, gated by a one-session join key, with an
optional WireGuard-encrypted tunnel ([tailcat](https://github.com/tailscale/tailcat)) —
no accounts, no database, no inbound ports.

**Explore the project at [vanish.pendura.in](https://vanish.pendura.in)**

```
┌ Vanish ──────────────────────────────────────┐
│● connected  127.0.0.1:8080  ·  you: alice     │
└──────────────────────────────────────────────┘
  *** alice joined the chat ***
00:45 alice   hello everyone
00:45 bob     hi alice
┌ Message ─────────────────────────────────────┐
│> ▊                                            │
└──────────────────────────────────────────────┘
```

## Install

macOS (arm64, x86_64) and Linux (x86_64, arm64), prebuilt:

```bash
curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/install.sh | sh
```

It downloads the release for your machine, checks its sha256, and puts
`vanish`, `vanish-server` and `vanish-client` in `~/.local/bin`
(override with `VANISH_INSTALL_DIR`; pin with `VANISH_VERSION=v0.1.0`).

```bash
vanish                       # start a room, as the host
vanish-client <address>      # join someone else's room
```

### Zero-install join

Guests don't need to install anything. The host shares a single command
(printed at startup and copied to the clipboard). The guest pastes it and
the chat opens:

```bash
sh -c "$(curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/join.sh)" -- <address>
```

It downloads the prebuilt client binary (cached for instant subsequent joins),
connects through the tunnel, and opens the chat right in the terminal.
The guest types their username, enters the join key shared by the host, and starts chatting.
No clone, no Rust toolchain, no prior setup.

Prefer the source? See [Build](#build).

## Features

- **One command to start.** Running the server opens a local client in the same
  terminal — type a username and you are chatting. When the host quits, the server
  (and the room) shuts down. The full invite (guest command + join key) is copied to the
  clipboard automatically when `pbcopy`, `wl-copy`, `xclip`, or `xsel` is available.
- **Quiet, browser-like startup.** Build output is replaced by a single in-place
  spinner instead of a wall of `Compiling ...` lines.
- **Join key per boot.** The server prints a random key at startup. Guests cannot
  join without it. Restart the server and the old key is dead — nothing is persisted.
- **Host controls.** `/kick <name>` disconnects a guest, announces it, and rotates the
  join key so the removed person's old key cannot rejoin. Only the host's local client
  can kick (it holds a per-boot token).
- **Guess throttling.** Wrong keys are rate-limited per source IP across connections:
  5 failures in 60 seconds earns a 60-second block, with a growing delay per failure.
- **Polished TUI.** Fixed input box, timestamps, per-user colors, own messages
  highlighted, join/leave events dimmed, and instant local echo of what you send.
- **tailcat on autopilot.** The server starts tailcat in the background and prints a
  single paste-able command for guests. The client auto-establishes the tunnel.
- **Tiny and dependency-light.** Tokio TCP server; ratatui/crossterm client.

## Requirements

When you install the release, only a terminal is needed — the binaries are
statically linked on Linux and need no Rust toolchain.

Building from source needs:

- Rust 1.75+ (both crates use edition 2021). If your distro ships an older
  toolchain, install current stable with
  [rustup](https://rustup.rs) (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
- Optional: [tailcat](https://github.com/tailscale/tailcat) for chatting across machines
  (`brew install tailcat`). Without it Vanish still runs on localhost.
- Optional: a clipboard tool to auto-copy the guest invite — `wl-copy` (Wayland),
  `xclip`, or `xsel` on Linux; `pbcopy` ships with macOS.

## Build

```bash
cargo build --release
```

This is a Cargo workspace and builds both binaries:
`target/release/vanish-server` and `target/release/vanish-client`.

## Quick start (same machine)

One command starts the server **and** the host's client in this terminal:

```bash
./vanish
# (equivalently: cargo run -p vanish-server)
```

On first run you see a spinner while it builds, then the join key, then the chat
client opens by itself. Type a **username** and you are in — the host never types
the join key.

When the host quits (`Ctrl+C` in the chat), the server stops and the room ends.

The invite for guests — guest command **and** join key — is printed at startup, copied
to the clipboard when a clipboard tool exists, and shown at the top of the host's chat
window, so you can share it without leaving the chat.

Guests on the same machine open their own terminal:

```bash
vanish-client
```

Guests type a **username** and the **join key** printed at startup. Open more
terminals for more people; usernames must be unique while connected.

To run the server without opening a local client (headless, for `systemd`, a
server box, or scripted tests):

```bash
CHAT_HOST=0 vanish
```

Host mode is also skipped automatically when stdin is not a terminal.

## Chatting across machines (tailcat, automatic)

The server starts tailcat for you and prints a ready-to-share one-liner:

```bash
vanish
# tailcat: address tcomFwWCCcjS5nKNqAod034nWoJZW0LZq
# tailcat: guests connect with  sh -c "$(curl -fsSL .../join.sh)" -- tcomFwWCCcjS5nKNqAod034nWoJZW0LZq:8080
# tailcat: guests join key      ABCD-EFGH
```

A guest pastes that command — no install, no clone. The client binary is
downloaded and cached, connects through the tunnel, and opens the chat
directly in the terminal:

```bash
sh -c "$(curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/join.sh)" -- tcomFwWCCcjS5nKNqAod034nWoJZW0LZq:8080
```

The client detects the `tc...` address, runs `tailcat forward` in the background, and
connects through the encrypted tunnel. tailcat must be installed on both machines.

If tailcat is not installed the server falls back to local-only and tells you how to
install it. Disable auto-tailcat explicitly with `CHAT_TAILCAT=0`.

## LAN without tailcat

```bash
# server (listen on all interfaces)
vanish 0.0.0.0:8080

# client (installed)
vanish-client 192.168.1.20:8080

# client (zero-install)
sh -c "$(curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/join.sh)" -- 192.168.1.20:8080
```

## Configuration

Server:

| Setting | How | Default |
|---|---|---|
| Bind address | CLI arg, or `CHAT_BIND` | `127.0.0.1:8080` |
| Local host client | `CHAT_HOST=0` to disable (skipped when stdin is not a TTY) | on |
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
  It is a session bearer token, not a user identity. Wrong keys are throttled per source
  IP: 5 failures within 60 seconds blocks that IP for 60 seconds, and each failure adds
  a 250 ms–1 s delay. Behind tailcat every guest can share one source address, so the
  limit also behaves as a coarse global throttle (and a brief denial-of-service lever).
- **Revocation.** The host client gets a random per-boot token (never printed) and uses
  it to claim host authority after authenticating. `/kick <name>` disconnects that
  client, announces the removal, and rotates the join key; existing guests keep their
  sessions, but the old invite cannot be reused. There is no account system, so a kicked
  user who obtains the new key can join again under a new name.
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
  sent as `*** ... ***` lines. Duplicate usernames are rejected.
- **Host mode**: when stdin is a terminal (and `CHAT_HOST` is not `0`), the server
  launches the sibling `vanish-client` binary on `127.0.0.1:<port>` with inherited
  stdio and the join key in `VANISH_KEY`. The client auto-submits the key, suppressing
  the join-key prompt. When that client exits, the server shuts down and cleans up
  tailcat. Server chat logs are suppressed in host mode so they cannot corrupt the TUI.
- **Client** (`client/`): a ratatui TUI driven by a Tokio event loop. It renders its own
  message immediately on send and skips the server's echo of it, so the chat feels
  instant and never double-prints. A background thread feeds keyboard events; a reader
  task feeds server lines.
- **Kick flow**: the host's client reads `VANISH_HOST_TOKEN` from the server, sends it
  once after `AUTH_OK`, and turns `/kick <name>` into a `KICK` command. The server
  verifies the token, drops the target's connection, rotates the key, and sends the new
  invite to the host's client, which refreshes the TUI line and the clipboard.
- **tailcat glue**: the server spawns `tailcat serve <port>`, parses the address from its
  output, and kills it on shutdown. The client spawns `tailcat forward` for `tc...`
  targets and connects to the resulting local port.

## Keyboard

| Key | Action |
|---|---|
| Enter | submit username / key / message |
| `/kick <name>` | host only: remove a participant and rotate the key |
| Backspace | edit input |
| Ctrl+C | quit (terminal is restored) |

## Project layout

```
Cargo.toml            workspace root
vanish                launcher: quiet spinner build, then server + host client
.cargo/config.toml    quiet cargo output
server/               vanish-server: Tokio TCP chat server
client/               vanish-client: ratatui terminal client
```

## Tests

```bash
cargo test -p vanish-server
```

Covers tailcat address parsing, bind-port parsing, and join-key normalization/comparison.

## Limitations

- No message history or persistence — restart and it is gone ("vanish").
- No file transfer, typing indicators, or rooms.
- Rate limiting is per source IP. Shared NAT (or tailcat, where every guest can share
  one address) means one attacker can briefly throttle or block new joins for everyone.
- After a kick the join key rotates; re-share the new invite with the guests you want.
  A determined removed user who receives the new key can rejoin under another name —
  there are no per-user credentials or cryptographic identities yet.
- Messages are not end-to-end encrypted: the server (the host) can read them. tailcat
  protects the network path, not the server process.
- A readable browser URL is not possible with tailcat alone; use a DNS `TXT` record or a
  different tunnel if you need one.
