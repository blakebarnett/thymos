#!/bin/bash
# Wrapper script to run Python examples with required environment variables
# This is needed due to jemalloc TLS limitations in Python extensions

export MALLOC_CONF='background_thread:false'
cd "$(dirname "$0")"
uv run python "$@"

