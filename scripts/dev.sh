#!/bin/bash
# Build the Wasm FDW and copy it into the local Supabase db container.
# Needs: cargo-component 0.13.2 and a running `supabase start` (the Rust toolchain is pinned in rust-toolchain.toml).

set -euxo pipefail

# stop Git Bash on Windows from rewriting container paths like /websearch_fdw.wasm
export MSYS_NO_PATHCONV=1

cd "$(dirname "$0")/.."

# build wasm fdw package
cargo component build --release --target wasm32-unknown-unknown

# set wasm file permission and copy it into supabase db container
chmod +r target/wasm32-unknown-unknown/release/websearch_fdw.wasm
db_container=$(docker ps --format "{{.Names}}" | grep supabase_db_)
docker cp target/wasm32-unknown-unknown/release/websearch_fdw.wasm "${db_container}":/websearch_fdw.wasm
