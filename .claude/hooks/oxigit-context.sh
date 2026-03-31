#!/bin/bash
set -e
INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')
if ! echo "$COMMAND" | grep -q "git commit"; then exit 0; fi

SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
MODEL=$(echo "$INPUT" | jq -r '.model // empty')
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path // empty')

# Extract the last substantive prompt from the transcript (skip "commit", "push", etc.)
PROMPT=""
if [ -n "$TRANSCRIPT" ] && [ -f "$TRANSCRIPT" ]; then
  PROMPT=$(jq -s -r '
    [.[] | select(.type == "human") | .message.content[]? | select(.type == "text") | .text]
    | map(select(test("^\\s*(/?(commit|push|commit and push|c|p)\\s*$)"; "i") | not))
    | last // empty
  ' "$TRANSCRIPT" 2>/dev/null | head -c 500 || true)
fi

# Write pending metadata to .git/ (not tracked)
GIT_DIR=$(git rev-parse --git-dir 2>/dev/null || echo ".git")
mkdir -p "$GIT_DIR"
jq -n \
  --arg tool "claude-code" \
  --arg model "$MODEL" \
  --arg session_id "$SESSION_ID" \
  --arg prompt "$PROMPT" \
  '{ tool: $tool, model: $model, session_id: $session_id, prompt: $prompt }' \
  > "$GIT_DIR/oxigit-pending.json"

# Install a prepare-commit-msg hook that appends trailers
mkdir -p "$GIT_DIR/hooks"
cat > "$GIT_DIR/hooks/prepare-commit-msg" << 'HOOKEOF'
#!/bin/bash
MSG_FILE="$1"
GIT_DIR=$(git rev-parse --git-dir 2>/dev/null || echo ".git")
PENDING="$GIT_DIR/oxigit-pending.json"
if [ ! -f "$PENDING" ]; then exit 0; fi

TOOL=$(jq -r '.tool // empty' "$PENDING")
MODEL=$(jq -r '.model // empty' "$PENDING")
SESSION=$(jq -r '.session_id // empty' "$PENDING")
PROMPT=$(jq -r '.prompt // empty' "$PENDING")

ARGS=()
[ -n "$TOOL" ] && ARGS+=(--trailer "Oxigit-Tool: $TOOL")
[ -n "$MODEL" ] && ARGS+=(--trailer "Oxigit-Model: $MODEL")
[ -n "$SESSION" ] && ARGS+=(--trailer "Oxigit-Session: $SESSION")
[ -n "$PROMPT" ] && ARGS+=(--trailer "Oxigit-Prompt: $PROMPT")

if [ ${#ARGS[@]} -gt 0 ]; then
  git interpret-trailers --in-place "${ARGS[@]}" "$MSG_FILE"
fi

rm -f "$PENDING"
HOOKEOF
chmod +x "$GIT_DIR/hooks/prepare-commit-msg"
