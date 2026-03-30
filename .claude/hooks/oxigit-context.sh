#!/bin/bash
set -e
INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')
if ! echo "$COMMAND" | grep -qE "^git commit"; then exit 0; fi
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
MODEL=$(echo "$INPUT" | jq -r '.model // empty')
mkdir -p .oxigit
jq -n --arg tool "claude-code" --arg model "$MODEL" --arg session_id "$SESSION_ID" \
  '{ tool: $tool,
     model: (if $model == "" then null else $model end),
     session_id: (if $session_id == "" then null else $session_id end) }' \
  > .oxigit/context.json
git add .oxigit/context.json