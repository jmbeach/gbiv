#!/bin/bash

# Read JSON input from stdin
input=$(cat)

# Extract the command field
command=$(echo "$input" | jq -r '.tool_input.command // ""')

deny() {
    jq -n --arg reason "$1" '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: $reason
    }
  }'
}

# Block push to/from main
if echo "$command" | grep -qE '^\s*git\s+push'; then
    current_branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
    if [ "$current_branch" = "main" ]; then
        deny "Do not push from the main branch."
        exit 0
    fi
    if echo "$command" | grep -qE '\s+main(\s|$)'; then
        deny "Do not push to the main branch."
        exit 0
    fi
fi
