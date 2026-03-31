#!/bin/bash
set -e
INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')
if ! echo "$COMMAND" | grep -q "git commit"; then exit 0; fi
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
MODEL=$(echo "$INPUT" | jq -r '.model // empty')
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path // empty')
PROMPT=""
if [ -f .oxigit/last-prompt.txt ]; then
  PROMPT=$(head -c 500 .oxigit/last-prompt.txt)
elif [ -n "$TRANSCRIPT" ] && [ -f "$TRANSCRIPT" ]; then
  PROMPT=$(jq -s -r '[.[] | select(.type == "human")] | last | .message.content[]? | select(.type == "text") | .text' "$TRANSCRIPT" 2>/dev/null | head -c 500 || true)
fi
mkdir -p .oxigit
jq -n --arg tool "claude-code" --arg model "$MODEL" --arg session_id "$SESSION_ID" --arg prompt "$PROMPT" \
  '{ tool: $tool,
     model: (if $model == "" then null else $model end),
     session_id: (if $session_id == "" then null else $session_id end),
     prompt: (if $prompt == "" then null else $prompt end) }' \
  > .oxigit/context.json
git add .oxigit/context.json