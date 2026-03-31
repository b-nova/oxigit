#!/bin/bash
set -e
INPUT=$(cat)
PROMPT=$(echo "$INPUT" | jq -r '.prompt // empty')
if [ -n "$PROMPT" ]; then
  mkdir -p .oxigit
  printf '%s' "$PROMPT" > .oxigit/last-prompt.txt
fi
