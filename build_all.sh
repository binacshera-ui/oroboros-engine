#!/bin/bash
# ============================================================================
# GLITCH WARS - Full Build Pipeline
# Builds Rust/WASM -> Copies to React -> Builds React
# ============================================================================

set -e

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║              GLITCH WARS - BUILD PIPELINE                        ║"
echo "╚══════════════════════════════════════════════════════════════════╝"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

# Step 1: Build Rust/WASM
echo -e "${BLUE}[1/4]${NC} Building Rust WASM..."
cargo build --release --target wasm32-unknown-unknown --bin glitch_wars --features=bevy_client_wasm

# Step 2: Generate WASM bindings
echo -e "${BLUE}[2/4]${NC} Generating WASM bindings..."
wasm-bindgen --out-dir web/target --target web target/wasm32-unknown-unknown/release/glitch_wars.wasm

# Step 3: Copy WASM to React public folder
echo -e "${BLUE}[3/4]${NC} Copying WASM to React..."
mkdir -p frontend/public/wasm
cp web/target/*.wasm frontend/public/wasm/
cp web/target/*.js frontend/public/wasm/

# Step 4: Build React (production)
echo -e "${BLUE}[4/4]${NC} Building React frontend..."
cd frontend
npm run build
cd ..

echo ""
echo -e "${GREEN}✓ Build complete!${NC}"
echo ""
echo "To run in development:"
echo "  cd frontend && npm run dev"
echo ""
echo "To run in production:"
echo "  cd frontend && npm run preview"
echo ""
