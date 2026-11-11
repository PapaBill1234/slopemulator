#!/usr/bin/env bash
# Play stack (card play-1): start, stop or show the services in tmux session "play".
# Usage: tools/run_play.sh start|stop|status
# Logs: logs/play/<service>.log
# Environment (all optional):
#   PLAY_PUBLIC_IP        address that the client uses (default: Tailscale IPv4, else 127.0.0.1)
#   PLAY_GAME_PORT        GameDaemon client port in the world list (default 10241)
#   PLAY_CLIGATE_PORT     CliGate client port (default 6600)
#   LOBBYGATE_CONFIG_DIR  directory with the 3 LobbyGate XML files
#   HANDSHAKE_KEY         1024-bit private blob for LobbyGate and GameDaemon
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/Program/output/debug"
CFG="$ROOT/tools/play"
LOGS="$ROOT/logs/play"
SESSION=play
ACCOUNTS="$ROOT/secrets/play-accounts.tsv"
GAME_ACCOUNTS="$ROOT/secrets/play-game-accounts.tsv"
LOBBYGATE_CONFIG_DIR="${LOBBYGATE_CONFIG_DIR:-$ROOT/../BnS_2020/LobbyGate/config}"
HANDSHAKE_KEY="${HANDSHAKE_KEY:-$ROOT/../server-keys/handshake_private.blob}"
PUBLIC_IP="${PLAY_PUBLIC_IP:-$(tailscale ip -4 2>/dev/null | head -1 || true)}"
PUBLIC_IP="${PUBLIC_IP:-127.0.0.1}"
GAME_PORT="${PLAY_GAME_PORT:-10241}"
CLIGATE_PORT="${PLAY_CLIGATE_PORT:-6600}"
LOBBYGATE_PORT="$(sed -nE "s/.*name='client-port' value='([0-9]+)'.*/\1/p" "$LOBBYGATE_CONFIG_DIR/lobbygate.config.xml" 2>/dev/null | head -1)"
LOBBYGATE_PORT="${LOBBYGATE_PORT:-10900}"
COMMON="--config $CFG/StServer.ini --route-ini $CFG/Route.ini"

# name | port to wait for | command
SERVICES=(
  "RouteSrv|6605|$BIN/RouteSrv --config $CFG/Route.ini --listen 127.0.0.1:6605 --server-id 1"
  "AuthSrv||$BIN/AuthSrv $COMMON --accounts $ACCOUNTS --server-id 2"
  "GameAccountSrv||$BIN/GameAccountSrv $COMMON --accounts $GAME_ACCOUNTS --server-id 3"
  "SlotSrv||$BIN/SlotSrv $COMMON --server-id 4"
  "PlayStubs||$BIN/PlayStubs $COMMON --server-id 5"
  "CliGate|$CLIGATE_PORT|$BIN/CliGate $COMMON --listen 0.0.0.0:$CLIGATE_PORT --server-id 6"
  "GameGate|6601|$BIN/GameGate $COMMON --listen 127.0.0.1:6601 --server-id 7"
  "LobbyDaemon|21801|$BIN/lobbydaemon 21801 --gamegate 127.0.0.1:6601 --game-code bns --world 1,1,Play,$PUBLIC_IP,$GAME_PORT"
  "LobbyGate|$LOBBYGATE_PORT|env LOBBYGATE_DEBUG=1 $BIN/lobbygate --config-dir $LOBBYGATE_CONFIG_DIR --handshake-key $HANDSHAKE_KEY"
)
# DbSrv is not on the play path yet (AuthSrv reads a file). It starts only with tools/play/Db.ini.
[ -f "$CFG/Db.ini" ] && SERVICES=("DbSrv|10901|$BIN/DbSrv --config $CFG/Db.ini --db AcctDb --listen 127.0.0.1:10901" "${SERVICES[@]}")
# GameDaemon has no server binary before stage 7.
[ -x "$BIN/gamedaemon" ] && SERVICES+=("GameDaemon|$GAME_PORT|$BIN/gamedaemon --listen 0.0.0.0:$GAME_PORT --handshake-key $HANDSHAKE_KEY --lobby 127.0.0.1:21801")

port_open() { (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; }

wait_port() {
  for _ in $(seq 1 50); do port_open "$1" && return 0; sleep 0.1; done
  return 1
}

start() {
  if tmux has-session -t "$SESSION" 2>/dev/null; then echo "session $SESSION is running; stop it first"; exit 1; fi
  for f in "$ACCOUNTS" "$GAME_ACCOUNTS"; do
    [ -s "$f" ] || { echo "missing $f; run tools/play_account.sh <LoginName> <UserName>"; exit 1; }
  done
  # A port that another process holds stops the start. The script does not kill it.
  local busy=0
  for p in 6605 "$CLIGATE_PORT" 6601 21801 "$LOBBYGATE_PORT"; do
    holder="$(ss -ltnpH "sport = :$p" 2>/dev/null | grep -oE 'users:\(\("[^"]+",pid=[0-9]+' | head -1 | sed -E 's/users:\(\("([^"]+)",pid=([0-9]+)/\1 (pid \2)/' || true)"
    if ss -ltnH "sport = :$p" 2>/dev/null | grep -q .; then echo "port $p is in use: ${holder:-unknown process}"; busy=1; fi
  done
  [ "$busy" = 0 ] || { echo "free the ports, then start again"; exit 1; }
  [ -f "$HANDSHAKE_KEY" ] || { echo "missing handshake key $HANDSHAKE_KEY"; exit 1; }
  [ -f "$LOBBYGATE_CONFIG_DIR/lobbygate.config.xml" ] || { echo "missing LobbyGate config in $LOBBYGATE_CONFIG_DIR"; exit 1; }
  if command -v docker >/dev/null && ! docker ps --format '{{.Names}}' 2>/dev/null | grep -qx bns-mssql; then
    echo "note: DB container bns-mssql is not running (the play path does not need it yet)"
  fi
  (cd "$ROOT" && cargo build -q -p routesrv -p authsrv -p gameaccountsrv -p slotsrv -p play-stubs -p cligate -p gamegate -p lobbydaemon -p lobbygate -p gamedaemon)
  mkdir -p "$LOGS"
  tmux new-session -d -s "$SESSION" -n shell -c "$ROOT"
  for entry in "${SERVICES[@]}"; do
    IFS='|' read -r name port cmd <<<"$entry"
    tmux new-window -d -t "$SESSION" -n "$name" -c "$LOGS" "$cmd 2>&1 | tee -a $LOGS/$name.log"
    if [ -n "$port" ] && ! wait_port "$port"; then echo "$name: port $port did not open; see $LOGS/$name.log"; fi
  done
  sleep 1
  status
  echo "client address: $PUBLIC_IP  (CliGate $CLIGATE_PORT, LobbyGate $LOBBYGATE_PORT, world port $GAME_PORT)"
}

stop() {
  tmux kill-session -t "$SESSION" 2>/dev/null && echo "stopped" || echo "not running"
}

status() {
  local windows
  windows="$(tmux list-windows -t "$SESSION" -F '#{window_name}' 2>/dev/null || true)"
  printf '%-16s %-8s %s\n' SERVICE STATE PORT
  for entry in "${SERVICES[@]}"; do
    IFS='|' read -r name port _ <<<"$entry"
    state=down
    if grep -qx "$name" <<<"$windows"; then
      state=up
      if [ -n "$port" ] && ! port_open "$port"; then state=no-port; fi
    fi
    printf '%-16s %-8s %s\n' "$name" "$state" "${port:--}"
  done
  for name in AuthSrv GameAccountSrv SlotSrv PlayStubs CliGate GameGate; do
    grep -q "registration finished(success)" "$LOGS/$name.log" 2>/dev/null || echo "$name: not registered on the route bus (see $LOGS/$name.log)"
  done
}

case "${1:-}" in
  start) start ;;
  stop) stop ;;
  status) status ;;
  *) echo "usage: tools/run_play.sh start|stop|status"; exit 2 ;;
esac
