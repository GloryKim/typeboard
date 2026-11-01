#!/bin/sh
# Keeps the doc server running in the background as a macOS LaunchAgent: it starts at login and
# is restarted by launchd if it exits. The service runs the release build of the Topcoat server
# (target/release/bun-report) with its asset bundle; `start` and `restart` rebuild both first.
# docs/ edits still show up live (the server watches docs/ and reloads open pages).
set -eu

LABEL="com.glory.bun-report"
DIR="$(cd "$(dirname "$0")/.." && pwd)"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
DOMAIN="gui/$(id -u)"
LOG="$DIR/logs/server.log"
PORT="${PORT:-3000}"
BIN="$DIR/target/release/bun-report"
CARGO_BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
export PATH="$CARGO_BIN:$PATH"

build() {
  echo "빌드 중 (cargo build --release, topcoat asset bundle --release)..."
  (cd "$DIR" && cargo build --release --quiet && topcoat asset bundle --release >/dev/null)
}

write_plist() {
  mkdir -p "$DIR/logs" "$(dirname "$PLIST")"
  cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$BIN</string>
  </array>
  <key>WorkingDirectory</key>
  <string>$DIR</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PORT</key>
    <string>$PORT</string>
    <key>REPORT_ROOT</key>
    <string>$DIR</string>
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ThrottleInterval</key>
  <integer>5</integer>
  <key>StandardOutPath</key>
  <string>$LOG</string>
  <key>StandardErrorPath</key>
  <string>$LOG</string>
</dict>
</plist>
EOF
}

unload() {
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
}

case "${1:-}" in
  build)
    build
    echo "빌드했습니다: $BIN"
    ;;
  start)
    build
    write_plist
    unload
    launchctl bootstrap "$DOMAIN" "$PLIST"
    echo "시작했습니다: http://localhost:$PORT  (로그: $LOG)"
    ;;
  stop)
    unload
    rm -f "$PLIST"
    echo "중지했습니다. 로그인할 때도 더 이상 자동으로 시작하지 않습니다."
    ;;
  restart)
    build
    launchctl kickstart -k "$DOMAIN/$LABEL"
    echo "다시 빌드하고 재시작했습니다."
    ;;
  status)
    if launchctl print "$DOMAIN/$LABEL" >/dev/null 2>&1; then
      launchctl print "$DOMAIN/$LABEL" | grep -E "^\s+(state|pid|last exit code) ="
    else
      echo "등록되어 있지 않습니다. 'sh scripts/service.sh start'로 시작하세요."
    fi
    ;;
  logs)
    tail -n 50 -f "$LOG"
    ;;
  *)
    echo "사용법: sh scripts/service.sh {build|start|stop|restart|status|logs}"
    exit 1
    ;;
esac
