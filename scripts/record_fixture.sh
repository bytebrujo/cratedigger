#!/bin/sh
# Deliberate live recording; stdout is the unmodified response body.
set -eu
if [ "$#" -ne 1 ]; then
  echo "usage: MCP_CRATES_USER_AGENT='mcp-crates/version (https://repository; email)' $0 '/api/v1/crates/itoa'" >&2
  exit 2
fi
case "$1" in
  /api/v1/crates|/api/v1/crates\?*|/api/v1/crates/*) ;;
  *) echo "endpoint: must start with /api/v1/crates" >&2; exit 2 ;;
esac
: "${MCP_CRATES_USER_AGENT:?Set an identifying User-Agent with repository URL and contact email}"
case "$MCP_CRATES_USER_AGENT" in
  *mcp-crates/*https://*@*) ;;
  *) echo "MCP_CRATES_USER_AGENT: expected mcp-crates/version, HTTPS URL and email" >&2; exit 2 ;;
esac
sleep 1
curl --fail --silent --show-error --retry 2 --retry-max-time 40 \
  --connect-timeout 3 --max-time 10 --max-filesize 8388608 \
  --user-agent "$MCP_CRATES_USER_AGENT" --header 'Accept: application/json' \
  "https://crates.io$1"
