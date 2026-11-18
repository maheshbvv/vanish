#!/bin/sh
# Vanish zero-trace join: downloads the client binary to a temporary folder,
# connects to the chat, and completely removes all downloaded files on exit.
#
# Usage (guests paste this one-liner shared by the host):
#
#   sh -c "$(curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/join.sh)" -- <address>
#
# Leaves zero trace on disk once the chat is closed.
set -eu

ADDRESS="${1:-}"
KEY="${2:-}"

if [ -z "$ADDRESS" ]; then
  echo "usage: join.sh <address> [key]" >&2
  echo "  address: host:port or tailcat-address:port" >&2
  echo "  key:     optional join key (if omitted, you will be prompted)" >&2
  exit 1
fi

if [ -n "$KEY" ]; then
  export VANISH_KEY="$KEY"
fi

# Standard bin paths in case of minimal subshell PATH
export PATH="$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

# If vanish-client is already installed on the system, run it directly
if command -v vanish-client >/dev/null 2>&1; then
  if [ ! -t 0 ] && [ -e /dev/tty ]; then
    exec vanish-client "$ADDRESS" </dev/tty
  else
    exec vanish-client "$ADDRESS"
  fi
fi

REPO="maheshbvv/vanish"
VERSION="${VANISH_VERSION:-latest}"

for tool in curl tar; do
  command -v "$tool" >/dev/null 2>&1 || { echo "vanish: needs $tool" >&2; exit 1; }
done

case "$(uname -s)" in
  Darwin) os_part="apple-darwin" ;;
  Linux)  os_part="unknown-linux-musl" ;;
  *)      echo "vanish: unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  arm64 | aarch64) arch_part="aarch64" ;;
  x86_64 | amd64)  arch_part="x86_64" ;;
  *)               echo "vanish: unsupported CPU: $(uname -m)" >&2; exit 1 ;;
esac

target="${arch_part}-${os_part}"

if [ "$VERSION" = "latest" ]; then
  api="https://api.github.com/repos/${REPO}/releases/latest"
  if command -v python3 >/dev/null 2>&1; then
    VERSION=$(curl -fsSL "$api" 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("tag_name",""))' 2>/dev/null || true)
  else
    VERSION=$(curl -fsSL "$api" 2>/dev/null | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
  fi
  [ -n "$VERSION" ] || VERSION="v0.1.1"
fi

name="vanish-${VERSION}-${target}"
url="https://github.com/${REPO}/releases/download/${VERSION}/${name}.tar.gz"

tmp=$(mktemp -d 2>/dev/null || mktemp -d -t vanish)
cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT INT TERM

printf 'vanish: opening secret chat for %s…\n' "$target" >&2
curl -fsSL "$url" -o "${tmp}/vanish.tar.gz" || { echo "vanish: download failed from $url" >&2; exit 1; }

tar -xzf "${tmp}/vanish.tar.gz" -C "$tmp"
bin="${tmp}/${name}/vanish-client"
chmod +x "$bin"

# Reconnect stdin to controlling terminal if available, because curl | sh
# leaves stdin attached to the pipe, which would break raw mode / keyboard events.
if [ ! -t 0 ] && [ -e /dev/tty ]; then
  "$bin" "$ADDRESS" </dev/tty || true
else
  "$bin" "$ADDRESS" || true
fi

# Chat has closed: clean up completely so zero trace is left on the machine
cleanup
trap - EXIT INT TERM
