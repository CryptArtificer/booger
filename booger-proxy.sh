#!/bin/bash
# Thin MCP proxy that spawns a fresh booger process per JSON-RPC message.
# This means rebuilding/reinstalling booger takes effect immediately
# without restarting the Cursor session.
ROOT="${1:-.}"
while IFS= read -r line; do
  echo "$line" | booger mcp "$ROOT"
done
