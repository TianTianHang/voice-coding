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
        
        cargoWrapper = pkgs.writeShellScriptBin "cargo" ''
          set -euo pipefail

          real_cargo="${rustToolchain}/bin/cargo"
          command_name="''${1:-}"

          has_manifest_arg() {
            for arg in "$@"; do
              case "$arg" in
                --manifest-path|--manifest-path=*) return 0 ;;
              esac
            done
            return 1
          }

          has_version_arg() {
            for arg in "$@"; do
              case "$arg" in
                --version|-V) return 0 ;;
              esac
            done
            return 1
          }

          has_help_arg() {
            for arg in "$@"; do
              case "$arg" in
                --help|-h) return 0 ;;
              esac
            done
            return 1
          }

          case "$command_name" in
            clippy)
              shift
              if has_version_arg "$@" || has_help_arg "$@"; then
                exec "${rustToolchain}/bin/cargo-clippy" "$@"
              fi

              if [ ! -f Cargo.toml ] && [ -f src-tauri/Cargo.toml ] && ! has_manifest_arg "$@"; then
                exec "${rustToolchain}/bin/cargo-clippy" clippy --manifest-path "$PWD/src-tauri/Cargo.toml" "$@"
              fi

              exec "${rustToolchain}/bin/cargo-clippy" clippy "$@"
              ;;
            build|check|test|run|doc|fmt|clean|metadata)
              if [ ! -f Cargo.toml ] && [ -f src-tauri/Cargo.toml ] && ! has_manifest_arg "$@" && ! has_version_arg "$@"; then
                shift
                exec "$real_cargo" "$command_name" --manifest-path "$PWD/src-tauri/Cargo.toml" "$@"
              fi
              ;;
          esac

          exec "$real_cargo" "$@"
        '';
        
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
          cargoWrapper
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

          # Virtual audio tooling (PipeWire/PulseAudio compatibility)
          pulseaudio
          pipewire
          wireplumber
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;
          
          shellHook = ''
            # Keep Cargo pinned to this flake's Rust toolchain. This avoids
            # inheriting rustup linker/configuration from the user's shell.
            export RUSTC="${rustToolchain}/bin/rustc"
            export RUSTDOC="${rustToolchain}/bin/rustdoc"
            export RUSTFMT="${rustToolchain}/bin/rustfmt"
            export CLIPPY_DRIVER="${rustToolchain}/bin/clippy-driver"
            export CARGO_TARGET_DIR="$PWD/src-tauri/target/nix"
            export CC="${pkgs.stdenv.cc}/bin/cc"
            export CXX="${pkgs.stdenv.cc}/bin/c++"
            
            # Rust source path for rust-analyzer
            export RUST_SRC_PATH="${pkgs.rustPlatform.rustLibSrc}"
            
            # ONNX Runtime configuration (use Nix package, avoid runtime download)
            export ORT_DYLIB_PATH="${pkgs.onnxruntime}/lib/libonnxruntime.so"
            export STT_MODEL_DIR="$PWD/models"
            export LD_LIBRARY_PATH="${pkgs.onnxruntime}/lib:$LD_LIBRARY_PATH"
            export LD_LIBRARY_PATH="${pkgs.libayatana-appindicator}/lib:$LD_LIBRARY_PATH"
            
            # LLVM C++ libraries for ten-vad
            export LD_LIBRARY_PATH="${pkgs.libcxx}/lib:$LD_LIBRARY_PATH"

            # Virtual audio helper commands
            setup-virtual-audio() {
              bash scripts/setup_virtual_audio.sh
            }

            cleanup-virtual-audio() {
              bash scripts/cleanup_virtual_audio.sh
            }

            # Add pre-commit hook
            if [ ! -f .git/hooks/pre-commit ] || grep -qx 'nix develop --command cargo clippy --all-targets --all-features -- -D warnings' .git/hooks/pre-commit; then
              mkdir -p .git/hooks
              echo '#!/bin/sh
nix develop --command cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
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
            echo "Virtual audio setup (for Rust backend recording tests):"
            echo "  setup-virtual-audio   - Create virtual input via PipeWire/PulseAudio"
            echo "  cleanup-virtual-audio - Remove virtual audio modules"
            echo "  docs/VIRTUAL_AUDIO_GUIDE.md"
            
            echo ""
            echo "Available commands:"
            echo "  pnpm dev      - Start development server"
            echo "  pnpm tauri    - Run Tauri commands"
            echo "  cargo build   - Build Rust backend"
            echo "  cargo test    - Run Rust tests"
            echo "  cargo clippy  - Lint Rust backend"
          '';
        };
      }
    );
}
