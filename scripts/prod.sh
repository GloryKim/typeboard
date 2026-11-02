#!/usr/bin/env bash
# Linux background start (macOS uses scripts/service.sh + LaunchAgent).
set -eu

DIR="$(cd "$(dirname "$0")/.." && pwd)"
LOG="$DIR/logs/server.log"
BIN="$DIR/target/release/bun-report"
CARGO_BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
export PATH="$CARGO_BIN:$HOME/.bun/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$DIR/target}"

HOST="${HOST:-0.0.0.0}"
PORT="${PORT:-3000}"

mkdir -p "$DIR/logs"

if [[ "${1:-}" == "--build" ]] || [[ ! -x "$BIN" ]]; then
  echo "빌드 중..."
  (cd "$DIR" && cargo build --release --quiet && topcoat asset bundle --release >/dev/null)
fi

pkill -f "$BIN" 2>/dev/null || true
sleep 1

nohup env HOST="$HOST" PORT="$PORT" REPORT_ROOT="$DIR" "$BIN" >"$LOG" 2>&1 &
disown
echo "시작: http://$HOST:$PORT  (로그: $LOG, pid=$!)"
