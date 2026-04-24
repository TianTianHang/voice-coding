{
  description = "Tauri development environment for voice-coding";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "rustfmt" "clippy" ];
        };
        
        buildInputs = with pkgs; [
          # Tauri dependencies
          webkitgtk_4_1
          gtk3
          gdk-pixbuf
          cairo
          pango
          atk
          at-spi2-atk
          at-spi2-core
          dbus
          librsvg
          
          # Build tools
          pkg-config
          openssl
          curl
          jq
          file
          cmake
          glib
          
          # Additional system libraries
          libayatana-appindicator
          libdrm
          libxkbcommon

          # ONNX Runtime for STT inference
          onnxruntime

          # GStreamer for WebKitGTK media support (audio recording)
          gst_all_1.gstreamer
          gst_all_1.gst-plugins-base
          gst_all_1.gst-plugins-good
          gst_all_1.gst-plugins-bad

          # ALSA for cpal audio input
          alsa-lib

          # LLVM C++ libraries for ten-vad
          libcxx
        ];
        
        nativeBuildInputs = with pkgs; [
          # Rust toolchain
          rustToolchain
          
          # Node.js ecosystem
          nodejs
          pnpm
          
          # Development tools
          pre-commit
          
          # Additional tools
          git
          wget
          git-xet
          git-lfs
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;
          
          shellHook = ''
            # Rust source path for rust-analyzer
            export RUST_SRC_PATH="${pkgs.rustPlatform.rustLibSrc}"
            
            # ONNX Runtime configuration (use Nix package, avoid runtime download)
            export ORT_DYLIB_PATH="${pkgs.onnxruntime}/lib/libonnxruntime.so"
            export STT_MODEL_DIR="$PWD/models"
            export LD_LIBRARY_PATH="${pkgs.onnxruntime}/lib:$LD_LIBRARY_PATH"
            
            # LLVM C++ libraries for ten-vad
            export LD_LIBRARY_PATH="${pkgs.libcxx}/lib:$LD_LIBRARY_PATH"
            
            # Add pre-commit hook
            if [ ! -f .git/hooks/pre-commit ]; then
              mkdir -p .git/hooks
              echo '#!/bin/sh
cargo clippy --all-targets --all-features -- -D warnings
pnpm run build || exit 1
' > .git/hooks/pre-commit
              chmod +x .git/hooks/pre-commit
            fi
            
            echo "✅ Tauri development environment ready!"
            echo "🦀 Rust: $(rustc --version)"
            echo "📦 Node: $(node --version)"
            echo "🔧 pnpm: $(pnpm --version)"
            echo ""
            echo "🧠 ONNX Runtime: ${pkgs.onnxruntime.version} (from Nix)"
            echo "📂 STT Models: $STT_MODEL_DIR"
            
            if [ ! -d "$STT_MODEL_DIR/onnx_models" ]; then
              echo ""
              echo "⚠️  STT models not found."
              echo "   Download with: bash scripts/download_model.sh"
            fi
            
            echo ""
            echo "Available commands:"
            echo "  pnpm dev      - Start development server"
            echo "  pnpm tauri    - Run Tauri commands"
            echo "  cargo build   - Build Rust backend"
            echo "  cargo test    - Run Rust tests"
          '';
        };
      }
    );
}
