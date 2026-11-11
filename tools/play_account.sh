#!/usr/bin/env bash
# Make a test account for the play stack (card play-1).
# Usage: tools/play_account.sh <LoginName> <UserName> [Alias]
# The script asks for the password (keyhere in docs). It stores only the SRP
# salt and verifier in secrets/play-accounts.tsv, and one game account for
# game code "bns" in secrets/play-game-accounts.tsv. Restart AuthSrv and
# GameAccountSrv after a change (tools/run_play.sh stop; start).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
[ $# -ge 2 ] || { echo "usage: tools/play_account.sh <LoginName> <UserName> [Alias]"; exit 2; }
LOGIN="$1"; USER_NAME="$2"; ALIAS="${3:-$2}"
ACCOUNTS="$ROOT/secrets/play-accounts.tsv"
GAME_ACCOUNTS="$ROOT/secrets/play-game-accounts.tsv"
if [ -f "$ACCOUNTS" ] && cut -f1 "$ACCOUNTS" | grep -qixF "$LOGIN"; then echo "login name exists"; exit 1; fi
(cd "$ROOT" && cargo build -q -p authsrv)
if [ -t 0 ]; then read -r -s -p "Password: " PASSWORD; echo; else read -r PASSWORD; fi
LINE="$(printf '%s\n' "$PASSWORD" | "$ROOT/Program/output/debug/AuthSrv" enroll "$LOGIN" "$USER_NAME")"
unset PASSWORD
USER_ID="$(printf '%s' "$LINE" | cut -f2)"
GAME_ACCOUNT_ID="$(python3 -c 'import uuid; print(str(uuid.uuid4()).upper())')"
umask 077
printf '%s\n' "$LINE" >>"$ACCOUNTS"
printf '%s\tbns\t%s\t%s\n' "$USER_ID" "$ALIAS" "$GAME_ACCOUNT_ID" >>"$GAME_ACCOUNTS"
echo "account $LOGIN added (UserId $USER_ID, alias $ALIAS)"
