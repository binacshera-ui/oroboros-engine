#!/bin/bash
# =============================================================================
# GLITCH WARS - Web Build Script
# =============================================================================
# Builds the Bevy client for WASM and serves it locally
# Target: Chrome with WebGL2
#
# NOTE: WASM build requires additional dependency configuration.
# Some crates (uuid, getrandom) need WASM-compatible features.
# For now, use native build with: cargo run --release --bin bevy_client --features bevy_client
# =============================================================================

set -e

echo "🎮 GLITCH WARS - Web Build"
echo "=========================="

# 1. Add WASM target if not already added
echo "📦 Ensuring WASM target..."
rustup target add wasm32-unknown-unknown 2>/dev/null || true

# 2. Install wasm-bindgen-cli if missing
if ! command -v wasm-bindgen &> /dev/null; then
    echo "📦 Installing wasm-bindgen-cli..."
    cargo install wasm-bindgen-cli
fi

# 3. Create web directory
mkdir -p web/target

# 4. Build for WASM (using glitch_wars binary - no networking)
echo "🔨 Building GLITCH WARS for WASM (this may take a while)..."
cargo build --release --target wasm32-unknown-unknown --bin glitch_wars --features bevy_client_wasm

# 5. Generate JS bindings
echo "🔗 Generating WASM bindings..."
wasm-bindgen \
    --out-dir ./web/target \
    --target web \
    --no-typescript \
    ./target/wasm32-unknown-unknown/release/glitch_wars.wasm

# 6. Copy assets if they exist
if [ -d "assets" ]; then
    echo "📁 Copying assets..."
    cp -r assets web/ 2>/dev/null || true
fi

# 7. Check if index.html exists
if [ ! -f "web/index.html" ]; then
    echo "⚠️  web/index.html not found! Creating default..."
    cat > web/index.html << 'EOF'
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>GLITCH WARS</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        html, body { 
            width: 100%; 
            height: 100%; 
            background: #050505; 
            overflow: hidden;
            font-family: 'Courier New', monospace;
        }
        #loading {
            position: fixed;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            color: #00FF41;
            font-size: 24px;
            text-shadow: 0 0 10px #00FF41, 0 0 20px #00FF41;
            animation: pulse 1s infinite;
        }
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }
        canvas {
            width: 100%;
            height: 100%;
            display: block;
        }
    </style>
</head>
<body>
    <div id="loading">INITIALIZING SIMULATION...</div>
    <script type="module">
        import init from './target/bevy_client.js';
        
        async function run() {
            try {
                await init();
                document.getElementById('loading').style.display = 'none';
            } catch (e) {
                document.getElementById('loading').innerHTML = 
                    'ERROR: ' + e.message + '<br><br>Check console for details.';
                document.getElementById('loading').style.color = '#FF0055';
                console.error('WASM initialization failed:', e);
            }
        }
        
        run();
    </script>
</body>
</html>
EOF
fi

echo ""
echo "✅ Build complete!"
echo ""
echo "🌐 Serving on http://localhost:8000"
echo "   Press Ctrl+C to stop"
echo ""

# 8. Serve
python3 -m http.server 8000 --bind 0.0.0.0 --directory web
