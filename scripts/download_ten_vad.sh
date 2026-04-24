#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TEN_VAD_VERSION="v1.0"

# Platform detection
detect_platform() {
    local os="$1"
    local arch="$2"

    case "$os" in
        Linux*)
            case "$arch" in
                x86_64|amd64)
                    echo "Linux/x64"
                    ;;
                aarch64|arm64)
                    echo "Linux/arm64"
                    ;;
                *)
                    echo "Unknown"
                    ;;
            esac
            ;;
        Darwin*)
            case "$arch" in
                x86_64|amd64)
                    echo "macOS/x86_64"
                    ;;
                arm64|aarch64)
                    echo "macOS/arm64"
                    ;;
                *)
                    echo "Unknown"
                    ;;
            esac
            ;;
        MINGW*|MSYS*|Windows*)
            case "$arch" in
                x86_64|amd64)
                    echo "Windows/x64"
                    ;;
                i686|i386)
                    echo "Windows/x86"
                    ;;
                *)
                    echo "Unknown"
                    ;;
            esac
            ;;
        *)
            echo "Unknown"
            ;;
    esac
}

# Get library path for platform
get_lib_info() {
    local platform="$1"

    case "$platform" in
        Linux/x64)
            echo "lib/Linux/x64/libten_vad.so:.so"
            ;;
        Linux/arm64)
            echo "lib/Linux/arm64/libten_vad.so:.so"
            ;;
        macOS/x86_64)
            echo "lib/macOS/x86_64/libten_vad.dylib:.dylib"
            ;;
        macOS/arm64)
            echo "lib/macOS/arm64/libten_vad.dylib:.dylib"
            ;;
        Windows/x64)
            echo "lib/Windows/x64/ten_vad.dll:.dll"
            ;;
        Windows/x86)
            echo "lib/Windows/x86/ten_vad.dll:.dll"
            ;;
        *)
            echo ""
            ;;
    esac
}

# Detect current platform
CURRENT_OS="$(uname -s)"
CURRENT_ARCH="$(uname -m)"
PLATFORM=$(detect_platform "$CURRENT_OS" "$CURRENT_ARCH")

if [ "$PLATFORM" = "Unknown" ]; then
    echo "❌ Error: Unsupported platform ${CURRENT_OS}/${CURRENT_ARCH}"
    echo "   Supported platforms: Linux (x64, arm64), macOS (x86_64, arm64), Windows (x64, x86)"
    exit 1
fi

echo "======================================"
echo "TEN VAD Library Download Script"
echo "======================================"
echo ""
echo "Detected platform: $PLATFORM"
echo "Version: ${TEN_VAD_VERSION}"
echo ""

# Parse command line arguments
DOWNLOAD_ALL=false
TARGET_PLATFORM=""

for arg in "$@"; do
    case $arg in
        --all)
            DOWNLOAD_ALL=true
            ;;
        --platform=*)
            TARGET_PLATFORM="${arg#*=}"
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --all              Download libraries for all platforms"
            echo "  --platform=PLATFORM Download for specific platform (Linux/x64, macOS/arm64, etc.)"
            echo "  --help, -h         Show this help message"
            echo ""
            echo "Supported platforms:"
            echo "  - Linux/x64, Linux/arm64"
            echo "  - macOS/x86_64, macOS/arm64"
            echo "  - Windows/x64, Windows/x86"
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Determine platforms to download
if [ "$DOWNLOAD_ALL" = true ]; then
    PLATFORMS=(
        "Linux/x64"
        "macOS/x86_64"
        "macOS/arm64"
        "Windows/x64"
        "Windows/x86"
    )
elif [ -n "$TARGET_PLATFORM" ]; then
    PLATFORMS=("$TARGET_PLATFORM")
else
    PLATFORMS=("$PLATFORM")
fi

# Clone repository
TEMP_CLONE_DIR="/tmp/ten-vad-clone-$$"

if [ -d "${TEMP_CLONE_DIR}" ]; then
    rm -rf "${TEMP_CLONE_DIR}"
fi

echo "Cloning ten-vad repository..."
git clone --depth 1 --branch "${TEN_VAD_VERSION}" --single-branch https://github.com/TEN-framework/ten-vad.git "${TEMP_CLONE_DIR}" 2>/dev/null || {
    echo "❌ Error: Failed to clone repository"
    exit 1
}
echo "✅ Repository cloned"
echo ""

# Download for each platform
SUCCESS_COUNT=0
FAIL_COUNT=0

for TARGET_PLATFORM in "${PLATFORMS[@]}"; do
    LIB_INFO=$(get_lib_info "$TARGET_PLATFORM")
    if [ -z "$LIB_INFO" ]; then
        echo "⚠️  Warning: Unsupported platform $TARGET_PLATFORM - skipping"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi

    SOURCE_PATH="${TEMP_CLONE_DIR}/${LIB_INFO%:*}"
    LIB_EXT="${LIB_INFO#*:}"

    # Convert platform to directory path
    case "$TARGET_PLATFORM" in
        Linux/*)
            DEST_DIR="${PROJECT_ROOT}/src-tauri/libs/${TARGET_PLATFORM%/*}/${TARGET_PLATFORM#*/}"
            ;;
        macOS/*)
            DEST_DIR="${PROJECT_ROOT}/src-tauri/libs/${TARGET_PLATFORM%/*}/${TARGET_PLATFORM#*/}"
            ;;
        Windows/*)
            DEST_DIR="${PROJECT_ROOT}/src-tauri/libs/${TARGET_PLATFORM%/*}/${TARGET_PLATFORM#*/}"
            ;;
    esac

    mkdir -p "$DEST_DIR"

    # Determine library filename
    if [ "$LIB_EXT" = ".dll" ]; then
        LIB_NAME="ten_vad.dll"
    else
        LIB_NAME="libten_vad${LIB_EXT}"
    fi

    DEST_PATH="${DEST_DIR}/${LIB_NAME}"

    echo "--------------------------------------"
    echo "Platform: $TARGET_PLATFORM"
    echo "Source: $SOURCE_PATH"
    echo "Destination: $DEST_PATH"
    echo ""

    if [ -f "$SOURCE_PATH" ]; then
        cp "$SOURCE_PATH" "$DEST_PATH"
        chmod +x "$DEST_PATH"
        echo "✅ Downloaded: $LIB_NAME"

        # Show file info
        if command -v file &> /dev/null; then
            file "$DEST_PATH" || true
        fi

        SIZE=$(du -h "$DEST_PATH" | cut -f1)
        echo "Size: $SIZE"
        echo ""
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    else
        echo "❌ Not found in repository: $SOURCE_PATH"
        echo "   This platform may not be supported yet"
        echo ""
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
done

# Cleanup
rm -rf "${TEMP_CLONE_DIR}"

# Summary
echo "======================================"
echo "Download Summary"
echo "======================================"
echo ""
echo "Successful: $SUCCESS_COUNT"
echo "Failed: $FAIL_COUNT"
echo ""

if [ $SUCCESS_COUNT -gt 0 ]; then
    echo "✅ Libraries are ready!"
    echo ""
    echo "Installed libraries:"
    find "${PROJECT_ROOT}/src-tauri/libs" -name "*ten_vad*" -type f 2>/dev/null || true
    echo ""
    echo "You can now build the project:"
    echo "  pnpm tauri dev"
else
    echo "❌ No libraries were downloaded"
    echo "   Please check the ten-vad repository for available prebuilt libraries"
    exit 1
fi
