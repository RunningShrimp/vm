#!/bin/bash
# scripts/quality_gate.sh
# 
# One-stop script for code quality checks across the workspace.
# Target: zero errors, zero warnings, zero fmt issues.
# All commands are protected with timeouts to prevent hanging.

set -e

# 获取脚本目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WITH_TIMEOUT="${SCRIPT_DIR}/with_timeout.sh"

# 确保 with_timeout.sh 可执行
chmod +x "${WITH_TIMEOUT}" 2>/dev/null || true

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Starting Quality Gate Checks ===${NC}"

echo -e "\n${BLUE}--- 1. Running cargo fmt (timeout: 2min) ---${NC}"
"${WITH_TIMEOUT}" 120 cargo fmt --all -- --check || { echo -e "${RED}Format check failed! Run 'cargo fmt --all' to fix.${NC}"; exit 1; }
echo -e "${GREEN}Format check passed.${NC}"

echo -e "\n${BLUE}--- 2. Running clippy (Standard Features, timeout: 10min) ---${NC}"
# Check workspace with default features
"${WITH_TIMEOUT}" 600 cargo clippy --workspace --all-targets -- -D warnings || { echo -e "${RED}Clippy failed on standard features!${NC}"; exit 1; }
echo -e "${GREEN}Clippy (Standard) passed.${NC}"

echo -e "\n${BLUE}--- 3. Running clippy (No Default Features, timeout: 10min) ---${NC}"
# Check workspace with no default features to ensure modularity
"${WITH_TIMEOUT}" 600 cargo clippy --workspace --all-targets --no-default-features -- -D warnings || { echo -e "${RED}Clippy failed on no-default-features!${NC}"; exit 1; }
echo -e "${GREEN}Clippy (No Default) passed.${NC}"

echo -e "\n${BLUE}--- 4. Running clippy (Cranelift Backend, timeout: 10min) ---${NC}"
# Check workspace with cranelift-backend feature
# Note: We use --features instead of all-features to isolate the cranelift path
"${WITH_TIMEOUT}" 600 cargo clippy --workspace --all-targets --features cranelift-backend -- -D warnings || { echo -e "${RED}Clippy failed on cranelift-backend feature!${NC}"; exit 1; }
echo -e "${GREEN}Clippy (Cranelift) passed.${NC}"

echo -e "\n${BLUE}--- 5. Running clippy (All Features, timeout: 10min) ---${NC}"
# Check workspace with all features
"${WITH_TIMEOUT}" 600 cargo clippy --workspace --all-targets --all-features -- -D warnings || { echo -e "${RED}Clippy failed on all features!${NC}"; exit 1; }
echo -e "${GREEN}Clippy (All Features) passed.${NC}"

echo -e "\n${BLUE}--- 6. Running tests (All Features, timeout: 30min) ---${NC}"
"${WITH_TIMEOUT}" 1800 cargo test --workspace --all-targets --all-features || { echo -e "${RED}Tests failed!${NC}"; exit 1; }
echo -e "${GREEN}Tests passed.${NC}"

echo -e "\n${GREEN}=== Quality Gate Passed Successfully! ===${NC}"

