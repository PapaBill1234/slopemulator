#!/usr/bin/env bash
# Show the newest slice card session live: tool calls and text, one line each.
#   tools/watch_card.sh          newest card session
#   tools/watch_card.sh st-06    session of card st-06
# Stop with Ctrl-c. The card keeps running.
card="${1:-}"
dir=$(ls -dt ~/.claude/projects/-home-ubuntu-Documents-Codex-bns-wt-${card}* 2>/dev/null | head -1)
[ -z "$dir" ] && { echo "no card session found"; exit 1; }
f=$(ls -t "$dir"/*.jsonl 2>/dev/null | head -1)
[ -z "$f" ] && { echo "no transcript in $dir"; exit 1; }
echo "== $f"
tail -n 40 -F "$f" | jq -r --unbuffered '
  select(.type=="assistant" or .type=="user") | .message.content[]? |
  if .type=="text" then "💬 " + (.text | gsub("\n";" ") | .[0:200])
  elif .type=="tool_use" then "🔧 " + .name + " " + ((.input.description // .input.code // .input.command // .input.file_path // (.input|tostring)) | tostring | gsub("\n";" ") | .[0:160])
  elif .type=="tool_result" then "   ↳ " + ((.content | if type=="array" then (map(.text? // "") | join(" ")) else tostring end) | gsub("\n";" ") | .[0:120])
  else empty end'
