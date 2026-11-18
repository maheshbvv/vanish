# Vanish

Ephemeral terminal chat in Rust. The room closes when the host quits.

Self-hosted and gated by a one-session join key, with an optional WireGuard tunnel through
[tailcat](https://github.com/tailscale/tailcat). No accounts, no database, no inbound ports.

```text
+ Vanish ------------------------------------------------------------------------------+
|* connected  127.0.0.1:8080  ·  you: alice                                            |
+--------------------------------------------------------------------------------------+
  > join key K4YV-7QT2 · invite copied to clipboard
  *** alice joined the chat ***
  *** bob joined the chat ***
10:47 alice   no accounts to make, no database to leak
10:47 bob     one key per boot, and it dies with the room
10:47 alice   nothing is written to disk. not the key, not the messages
  *** bob was removed by the host ***

+ Message -----------------------------------------------------------------------------+
|>                                                                                     |
+--------------------------------------------------------------------------------------+
```

The host window after `/kick bob`. Before the kick the key was GDNW-ZGGV; after it, K4YV-7QT2.

Site: [vanish.pendura.in](https://vanish.pendura.in).
Source: [github.com/maheshbvv/vanish](https://github.com/maheshbvv/vanish).

## Install

macOS (arm64, x86_64) and Linux (x86_64, arm64), prebuilt:

```bash
curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/install.sh | sh
```

The script downloads the release for your machine, checks its sha256, and puts `vanish`,
`vanish-server` and `vanish-client` in `~/.local/bin`. Override the directory with
`VANISH_INSTALL_DIR`; pin a release with `VANISH_VERSION=v0.1.1`.

```bash
vanish                       # start a room, as the host
vanish-client <address>      # join someone else's room
```

Prefer the source? See [Build](#build).

## Run it

### Host a room

```bash
vanish
```

One command starts the server **and** the host's client in this terminal. On first run you
see a quiet spinner while it builds, then the join key, then the chat opens by itself. Type
a username and you are in; the host never types the key.

When the host quits (`Ctrl+C` in the chat), the server stops and the room ends.

The guest command and join key are printed at startup and copied to the clipboard when
`pbcopy`, `wl-copy`, `xclip` or `xsel` is available. They also sit at the top of the chat
window, so you can share them without leaving the chat.

For a server box, `systemd`, or scripted tests, skip the local client:

```bash
CHAT_HOST=0 vanish
```

Host mode is also skipped automatically when stdin is not a terminal.

### Join with one command

Guests install nothing. The host shares one line, printed at startup:

```bash
sh -c "$(curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/join.sh)" -- <address>
```

It downloads the prebuilt client into a temporary sandbox, connects through the tunnel, and
opens the chat in the terminal. The guest types a username, enters the join key from the
host, and starts talking.

When the host closes the room or the guest leaves, the client and every downloaded file are
deleted. Nothing stays on the guest's machine.

An installed client can wipe itself on exit too:

```bash
vanish-client <address> --vanish
```

### Across machines, with tailcat

The server starts tailcat for you and prints a ready-to-share one-liner:

```bash
vanish
# tailcat: address tcomFwWCCcjS5nKNqAod034nWoJZW0LZq
# tailcat: guests connect with  sh -c "$(curl -fsSL .../join.sh)" -- tcomFwWCCcjS5nKNqAod034nWoJZW0LZq:8080
# tailcat: guests join key      ABCD-EFGH
```

A guest pastes that command: no install, no clone. The client sees the `tc...` address, runs
`tailcat forward` in the background, and connects through the encrypted tunnel. tailcat must
be installed on both machines (`brew install tailcat`).

Without tailcat the server falls back to local-only and says how to install it. Turn the
automatic tunnel off with `CHAT_TAILCAT=0`.

### Same LAN, no tailcat

```bash
# server, on all interfaces
vanish 0.0.0.0:8080

# client, installed
vanish-client 192.168.1.20:8080

# client, zero-install
sh -c "$(curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/join.sh)" -- 192.168.1.20:8080
```

Plain TCP on a LAN, so keep it on a network you trust.

## What the room holds

| Part | What it does |
|---|---|
| **Join key** | Eight characters from the OS RNG, never on disk, dead on restart. |
| **Guess throttle** | Five wrong keys in a minute block that address. |
| **`/kick bob`** | Disconnects the guest, announces it, rotates the key. |
| **Zero trace** | Messages live only in RAM. The alternate screen clears terminal scrollback on exit. |
| **Self-destruct** | The guest one-liner leaves no binary or temp files. Installed clients can pass `--vanish` to wipe the binary. |
| **Transport** | Plain TCP locally. tailcat adds WireGuard between machines. |

The chat itself is a fixed input box, timestamps, a colour per user, your own lines
highlighted, join and leave events dimmed, and local echo so a sent line appears instantly.
The server is a Tokio TCP listener; the client is ratatui and crossterm. Two small binaries,
no daemon left behind.

## Configuration

Server:

| Setting | How | Default |
|---|---|---|
| Bind address | CLI arg, or `CHAT_BIND` | `127.0.0.1:8080` |
| Local host client | `CHAT_HOST=0` to disable (skipped when stdin is not a TTY) | on |
| Join key | `CHAT_KEY` env (pins it, dev only) | random 8 chars per boot |
| tailcat | `CHAT_TAILCAT=0` to disable | auto-start |

Client:

| Setting | How | Default |
|---|---|---|
| Target | CLI arg, or `CHAT_SERVER`; accepts `host:port`, `tc...`, `tc...:port` | `127.0.0.1:8080` |
| Self-destruct | `--vanish` / `--self-destruct`, or `VANISH_SELF_DESTRUCT=1` | off (deletes binary on exit) |

The join key is normalized: `abcd-efgh`, `ABCD EFGH` and `ABCDEFGH` are equivalent.

## Keyboard

| Key | Action |
|---|---|
| Enter | submit username, key, or message |
| `/kick <name>` | host only: remove a participant and rotate the key |
| Backspace | edit input |
| Ctrl+C | quit, terminal restored |

## Security model

**Nothing on disk.** No database, no chat log, no temp files. Messages and usernames exist
only in process memory while the room is open. When the host quits, the server exits and
everything is gone.

**Nothing in scrollback.** Host and guest clients use the terminal's alternate screen. When
the chat closes, the terminal returns to what it showed before Vanish started. No transcript
is left behind.

**Nothing from the guest.** The one-liner runs inside `mktemp -d`. When the chat ends or the
host disconnects, the downloaded binary and its temporary files are purged at once.

**Nothing from an installed client either.** With `--vanish`, `--self-destruct`, or
`VANISH_SELF_DESTRUCT=1`, the client unlinks its own executable the moment the chat finishes.

**The key.** Eight random characters, about 40 bits, from the OS RNG, regenerated every
boot, never written to disk, compared in constant time, with three attempts per connection.
It is a session bearer token, not a user identity.

Wrong keys are throttled per source IP: five failures within 60 seconds block that address
for 60 seconds, and each failure adds a 250 ms to 1 s delay. Behind tailcat every guest can
share one source address, so the limit is also a coarse global throttle, and briefly a
denial-of-service lever.

**Revocation.** The host client gets a random per-boot token, never printed, and uses it to
claim host authority after authenticating. `/kick <name>` drops that client, announces the
removal, and rotates the key. Existing guests keep their sessions; the old invite cannot be
reused. There are no accounts, so a removed user who gets the new key can rejoin under a
new name.

**Transport.** Localhost is plain TCP. Across machines, run it through tailcat or Tailscale:
the tunnel is WireGuard-encrypted, the tailcat address is itself a secret capability, and no
inbound port is opened.

**The limits, plainly.** Plain TCP carries the join key and messages in cleartext. Do not
expose a bare `0.0.0.0` socket to an untrusted network; put tailcat in front of it. There is
no per-user authentication, no persistence, and no app-layer encryption beyond the transport
you provide.

## How it works

**Server** (`server/`). A Tokio TCP listener. Per connection it asks for a username, then a
join key (3 tries), then sends `AUTH_OK` and registers the client. Every chat line is
broadcast to all connected clients as `[username] message`; system events go out as
`*** ... ***`. Duplicate usernames are rejected.

**Host mode.** When stdin is a terminal and `CHAT_HOST` is not `0`, the server launches the
sibling `vanish-client` on `127.0.0.1:<port>` with inherited stdio and the join key in
`VANISH_KEY`. The client submits the key itself and suppresses the prompt. When that client
exits, the server shuts down and cleans up tailcat. Server chat logs stay off in host mode so
they cannot corrupt the TUI.

**Client** (`client/`). A ratatui TUI on a Tokio event loop. It renders a sent message
immediately and skips the server's echo, so the chat feels instant and never double-prints. A
background thread feeds keyboard events; a reader task feeds server lines.

**Kick flow.** The host's client reads `VANISH_HOST_TOKEN` from the server, sends it once
after `AUTH_OK`, and turns `/kick <name>` into a `KICK` command. The server verifies the
token, drops the target, rotates the key, and sends the new invite to the host's client,
which refreshes the TUI line and the clipboard.

**tailcat glue.** The server spawns `tailcat serve <port>`, parses the address from its
output, and kills it on shutdown. The client spawns `tailcat forward` for `tc...` targets and
connects to the resulting local port.

## Build

```bash
cargo build --release
```

This is a Cargo workspace and builds both binaries:
`target/release/vanish-server` and `target/release/vanish-client`.

Requirements:

- Rust 1.85+ (edition 2024). If your distro ships an older toolchain, install current stable
  with [rustup](https://rustup.rs)
  (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
- Optional: [tailcat](https://github.com/tailscale/tailcat) for chatting across machines.
- Optional: a clipboard tool for the guest invite: `wl-copy` (Wayland), `xclip` or `xsel` on
  Linux; `pbcopy` ships with macOS.

With a release install, only a terminal is needed. The Linux binaries are statically linked
and need no Rust toolchain.

```bash
./vanish
# equivalently: cargo run -p vanish-server
```

## Tests

```bash
cargo test -p vanish-server
```

Covers tailcat address parsing, bind-port parsing, and join-key normalization and comparison.

## Project layout

```text
Cargo.toml            workspace root
vanish                launcher: quiet spinner build, then server + host client
.cargo/config.toml    quiet cargo output
server/               vanish-server: Tokio TCP chat server
client/               vanish-client: ratatui terminal client
```

## Limitations

- No message history or persistence. Restart and it is gone. That is the point.
- No file transfer, typing indicators, or rooms.
- Rate limiting is per source IP. Shared NAT, or tailcat where every guest shares one
  address, means one attacker can briefly throttle or block new joins for everyone.
- After a kick the key rotates. Re-share the new invite with the guests you want. A
  determined removed user who receives the new key can rejoin under another name; there are
  no per-user credentials or cryptographic identities yet.
- Messages are not end-to-end encrypted. The server, the host, can read them. tailcat
  protects the network path, not the server process.
- A readable browser URL is not possible with tailcat alone. Use a DNS `TXT` record or a
  different tunnel if you need one.
