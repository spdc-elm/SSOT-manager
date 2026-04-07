#!/bin/sh

set -eu

REPO="${SSOT_MANAGER_REPO:-spdc-elm/SSOT-manager}"
VERSION="${SSOT_MANAGER_VERSION:-latest}"
INSTALL_DIR="${SSOT_MANAGER_INSTALL_DIR:-}"
BINARY_NAME="ssot-manager"
INSTALL_DIR_EXPLICIT=0

usage() {
  cat <<EOF
Install ${BINARY_NAME} from GitHub Releases.

Usage:
  install.sh [--version <tag-or-latest>] [--install-dir <dir>] [--repo <owner/name>]

Examples:
  sh install.sh
  sh install.sh --version v0.1.0
  sh install.sh --install-dir /usr/local/bin

Environment overrides:
  SSOT_MANAGER_VERSION
  SSOT_MANAGER_INSTALL_DIR
  SSOT_MANAGER_REPO
EOF
}

path_contains_dir() {
  dir="$1"
  case ":${PATH:-}:" in
    *:"$dir":*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

dir_is_writable_or_creatable() {
  dir="$1"

  if [ -d "$dir" ]; then
    [ -w "$dir" ]
    return
  fi

  parent_dir=$(dirname "$dir")
  [ -d "$parent_dir" ] && [ -w "$parent_dir" ]
}

pick_install_dir() {
  if command -v "$BINARY_NAME" >/dev/null 2>&1; then
    dirname "$(command -v "$BINARY_NAME")"
    return
  fi

  for candidate in "/usr/local/bin" "/opt/homebrew/bin" "${HOME}/.local/bin" "${HOME}/bin"; do
    if path_contains_dir "$candidate" && dir_is_writable_or_creatable "$candidate"; then
      printf '%s\n' "$candidate"
      return
    fi
  done

  printf '%s\n' "${HOME}/.local/bin"
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

download_to() {
  url="$1"
  dest="$2"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 -o "$dest" "$url"
    return
  fi

  if command -v wget >/dev/null 2>&1; then
    wget -qO "$dest" "$url"
    return
  fi

  echo "need curl or wget to download release assets" >&2
  exit 1
}

resolve_latest_tag() {
  api_url="https://api.github.com/repos/${REPO}/releases/latest"
  tmp_json="$1"

  download_to "$api_url" "$tmp_json"

  tag_name=$(
    sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' "$tmp_json" \
      | head -n 1
  )

  if [ -z "$tag_name" ]; then
    echo "failed to resolve latest release tag from ${api_url}" >&2
    exit 1
  fi

  printf '%s\n' "$tag_name"
}

detect_target() {
  uname_s=$(uname -s)
  uname_m=$(uname -m)

  case "$uname_s" in
    Linux)
      case "$uname_m" in
        x86_64|amd64)
          printf '%s\n' 'x86_64-unknown-linux-gnu'
          ;;
        *)
          echo "unsupported Linux architecture: ${uname_m}" >&2
          echo "published Unix binaries currently cover Linux x86_64 and macOS x86_64/aarch64" >&2
          exit 1
          ;;
      esac
      ;;
    Darwin)
      case "$uname_m" in
        x86_64|amd64)
          printf '%s\n' 'x86_64-apple-darwin'
          ;;
        arm64|aarch64)
          printf '%s\n' 'aarch64-apple-darwin'
          ;;
        *)
          echo "unsupported macOS architecture: ${uname_m}" >&2
          exit 1
          ;;
      esac
      ;;
    *)
      echo "unsupported operating system: ${uname_s}" >&2
      echo "use the GitHub release assets directly for unsupported platforms" >&2
      exit 1
      ;;
  esac
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --install-dir)
      INSTALL_DIR="$2"
      INSTALL_DIR_EXPLICIT=1
      shift 2
      ;;
    --repo)
      REPO="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [ "${SSOT_MANAGER_INSTALL_DIR+x}" = "x" ]; then
  INSTALL_DIR_EXPLICIT=1
fi

need_cmd uname
need_cmd sed
need_cmd tar
need_cmd mktemp
need_cmd mkdir
need_cmd chmod

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT INT TERM HUP

if [ "$VERSION" = "latest" ]; then
  VERSION=$(resolve_latest_tag "$tmpdir/latest-release.json")
fi

if [ "$INSTALL_DIR_EXPLICIT" -ne 1 ]; then
  INSTALL_DIR=$(pick_install_dir)
fi

TARGET=$(detect_target)
ARCHIVE_NAME="${BINARY_NAME}-${VERSION}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE_NAME}"
ARCHIVE_PATH="${tmpdir}/${ARCHIVE_NAME}"
EXTRACT_DIR="${tmpdir}/extract"
DEST_PATH="${INSTALL_DIR}/${BINARY_NAME}"

mkdir -p "$EXTRACT_DIR"

echo "Downloading ${DOWNLOAD_URL}"
download_to "$DOWNLOAD_URL" "$ARCHIVE_PATH"

echo "Extracting ${ARCHIVE_NAME}"
tar -xzf "$ARCHIVE_PATH" -C "$EXTRACT_DIR"

if [ ! -f "${EXTRACT_DIR}/${BINARY_NAME}" ]; then
  echo "archive did not contain ${BINARY_NAME}" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
cp "${EXTRACT_DIR}/${BINARY_NAME}" "$DEST_PATH"
chmod 0755 "$DEST_PATH"

echo "Installed ${BINARY_NAME} ${VERSION} to ${DEST_PATH}"
"$DEST_PATH" --version

case ":${PATH:-}:" in
  *:"${INSTALL_DIR}":*)
    ;;
  *)
    echo "warning: ${INSTALL_DIR} is not on PATH" >&2
    ;;
esac
