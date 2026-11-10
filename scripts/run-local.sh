#!/bin/sh
set -eu

: "${INFRAI_API_KEY:?set INFRAI_API_KEY before running}"
cargo run --bin matter_intake -- "$@"

