#!/bin/bash
set -e

# Read hook input from stdin
INPUT=$(cat)

COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

# Only act on git commit commands
if ! echo "$COMMAND" | grep -qE "^git commit"; then
  exit 0
fi

SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path // empty')

# Extract the last user prompt from the transcript
PROMPT=""
if [ -n "$TRANSCRIPT" ] && [ -f "$TRANSCRIPT" ]; then
  PROMPT=$(jq -r '
    [.[] | select(.type == "human")] | last |
    .message.content[]? | select(.type == "text") | .text
  ' "$TRANSCRIPT" 2>/dev/null | head -c 500 || true)
fi

# Write .oxigit/context.json matching OxigitContext struct fields
mkdir -p .oxigit
jq -n \
  --arg tool "claude-code" \
  --arg model "claude-opus-4-6" \
  --arg session_id "$SESSION_ID" \
  --arg prompt "$PROMPT" \
  '{
    tool: $tool,
    model: $model,
    session_id: (if $session_id == "" then null else $session_id end),
    prompt: (if $prompt == "" then null else $prompt end)
  }' > .oxigit/context.json

# Stage it so it's included in the commit
git add .oxigit/context.json

exit 0
