#!/bin/sh
# Vanish installer.
#
#   curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/install.sh | sh
#
# Downloads the release for this machine, checks its sha256, and installs
# `vanish-server`, `vanish-client` and a `vanish` launcher.
#
# Environment:
#   VANISH_VERSION      tag to install (default: latest release)
#   VANISH_INSTALL_DIR  where the binaries go (default: ~/.local/bin)
#   VANISH_NO_VERIFY=1  skip the checksum check (not recommended)
set -eu

REPO="maheshbvv/vanish"
VERSION="${VANISH_VERSION:-latest}"
BIN_DIR="${VANISH_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
fail() {
  printf 'vanish: %s\n' "$*" >&2
  exit 1
}

for tool in curl tar; do
  command -v "$tool" >/dev/null 2>&1 || fail "needs $tool"
done

case "$(uname -s)" in
  Darwin) os_part="apple-darwin" ;;
  Linux) os_part="unknown-linux-musl" ;;
  *) fail "unsupported OS: $(uname -s). Vanish ships for macOS and Linux." ;;
esac

case "$(uname -m)" in
  arm64 | aarch64) arch_part="aarch64" ;;
  x86_64 | amd64) arch_part="x86_64" ;;
  *) fail "unsupported CPU: $(uname -m). Vanish ships for arm64 and x86_64." ;;
esac

target="${arch_part}-${os_part}"

if [ "$VERSION" = "latest" ]; then
  api="https://api.github.com/repos/${REPO}/releases/latest"
  if command -v python3 >/dev/null 2>&1; then
    VERSION=$(curl -fsSL "$api" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("tag_name",""))')
  else
    VERSION=$(curl -fsSL "$api" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
  fi
  [ -n "$VERSION" ] || fail "could not read the latest release; set VANISH_VERSION=v0.1.0"
fi

name="vanish-${VERSION}-${target}"
url="https://github.com/${REPO}/releases/download/${VERSION}/${name}.tar.gz"

tmp=$(mktemp -d 2>/dev/null || mktemp -d -t vanish)
trap 'rm -rf "$tmp"' EXIT INT TERM

say "vanish ${VERSION} for ${target}"
curl -fsSL "$url" -o "${tmp}/vanish.tar.gz" || fail "download failed: $url"

if [ "${VANISH_NO_VERIFY:-0}" != "1" ]; then
  curl -fsSL "${url}.sha256" -o "${tmp}/vanish.tar.gz.sha256" ||
    fail "checksum download failed: ${url}.sha256"
  expected=$(awk '{print $1}' "${tmp}/vanish.tar.gz.sha256")
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "${tmp}/vanish.tar.gz" | awk '{print $1}')
  else
    actual=$(shasum -a 256 "${tmp}/vanish.tar.gz" | awk '{print $1}')
  fi
  [ "$expected" = "$actual" ] || fail "checksum mismatch: the download is not the published build"
fi

tar -xzf "${tmp}/vanish.tar.gz" -C "$tmp"
src="${tmp}/${name}"
mkdir -p "$BIN_DIR"
install -m 755 "${src}/vanish-server" "${BIN_DIR}/vanish-server"
install -m 755 "${src}/vanish-client" "${BIN_DIR}/vanish-client"

# Start a room: the server opens the host client in this same terminal.
cat >"${BIN_DIR}/vanish" <<'LAUNCHER'
#!/bin/sh
exec "$(dirname "$0")/vanish-server" "$@"
LAUNCHER
chmod 755 "${BIN_DIR}/vanish"

say "installed to ${BIN_DIR}"
case ":${PATH}:" in
  *":${BIN_DIR}:"*) ;;
  *)
    say ""
    say "add it to your PATH:"
    say "  export PATH=\"${BIN_DIR}:\$PATH\""
    ;;
esac
say ""
say "start a room:  vanish"
say "join a room:   vanish-client <address>"
