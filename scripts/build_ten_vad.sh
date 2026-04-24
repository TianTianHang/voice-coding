#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TEMP_DIR="${PROJECT_ROOT}/.temp/ten-vad"
TEN_VAD_VERSION="v1.0"
TEN_VAD_REPO="https://github.com/TEN-framework/ten-vad.git"

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

# Get build script for platform
get_build_script() {
    local platform="$1"

    case "$platform" in
        Linux/*)
            echo "build-and-deploy-linux.sh"
            ;;
        macOS/*)
            echo "build-and-deploy-mac.sh"
            ;;
        Windows/*)
            echo "build-and-deploy-windows.bat"
            ;;
        *)
            echo ""
            ;;
    esac
}

# Get library path for platform
get_lib_info() {
    local platform="$1"

    case "$platform" in
        Linux/x64)
            echo "lib/Linux/x64/libten_vad.so"
            ;;
        Linux/arm64)
            echo "lib/Linux/arm64/libten_vad.so"
            ;;
        macOS/x86_64)
            echo "lib/macOS/x86_64/libten_vad.dylib"
            ;;
        macOS/arm64)
            echo "lib/macOS/arm64/libten_vad.dylib"
            ;;
        Windows/x64)
            echo "lib/Windows/x64/ten_vad.dll"
            ;;
        Windows/x86)
            echo "lib/Windows/x86/ten_vad.dll"
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
    echo "   Supported platforms: Linux (x64), macOS (x86_64, arm64), Windows (x64)"
    exit 1
fi

echo "======================================"
echo "TEN VAD Library Build Script"
echo "======================================"
echo ""
echo "Detected platform: $PLATFORM"
echo "Version: ${TEN_VAD_VERSION}"
echo ""

# Parse command line arguments
BUILD_ALL=false
TARGET_PLATFORM=""

for arg in "$@"; do
    case $arg in
        --all)
            BUILD_ALL=true
            ;;
        --platform=*)
            TARGET_PLATFORM="${arg#*=}"
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --all              Build libraries for all supported platforms"
            echo "  --platform=PLATFORM Build for specific platform"
            echo "  --help, -h         Show this help message"
            echo ""
            echo "Supported platforms:"
            echo "  - Linux/x64"
            echo "  - macOS/x86_64, macOS/arm64"
            echo "  - Windows/x64"
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Determine platforms to build
if [ "$BUILD_ALL" = true ]; then
    echo "⚠️  Warning: Building for all platforms is not recommended"
    echo "   Each platform requires specific build tools and environment"
    echo ""
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 0
    fi
    PLATFORMS=(
        "Linux/x64"
        "macOS/x86_64"
        "macOS/arm64"
        "Windows/x64"
    )
elif [ -n "$TARGET_PLATFORM" ]; then
    PLATFORMS=("$TARGET_PLATFORM")
else
    PLATFORMS=("$PLATFORM")
fi

# Check dependencies for current platform
echo "Checking dependencies..."

if ! command -v git &> /dev/null; then
    echo "❌ Error: git is required but not installed"
    exit 1
fi

# Platform-specific checks
case "$CURRENT_OS" in
    Linux*)
        if ! command -v cmake &> /dev/null; then
            echo "❌ Error: cmake is required but not installed"
            exit 1
        fi
        if ! command -v clang &> /dev/null && ! command -v gcc &> /dev/null; then
            echo "❌ Error: clang or gcc is required but not installed"
            exit 1
        fi
        ;;
    Darwin*)
        if ! command -v cmake &> /dev/null; then
            echo "❌ Error: cmake is required but not installed"
            echo "   Install with: brew install cmake"
            exit 1
        fi
        if ! command -v xcodebuild &> /dev/null; then
            echo "❌ Error: Xcode command line tools are required"
            echo "   Install with: xcode-select --install"
            exit 1
        fi
        ;;
    MINGW*|MSYS*|Windows*)
        if ! command -v cmake &> /dev/null; then
            echo "❌ Error: cmake is required but not installed"
            exit 1
        fi
        ;;
esac

echo "✅ All dependencies found"
echo ""

# Clone repository
echo "Cloning ten-vad repository..."
if [ -d "${TEMP_DIR}" ]; then
    echo "Repository already exists, updating..."
    cd "${TEMP_DIR}"
    git fetch --tags
    git checkout "${TEN_VAD_VERSION}"
    git pull
else
    git clone --depth 1 --branch "${TEN_VAD_VERSION}" "${TEN_VAD_REPO}" "${TEMP_DIR}"
fi
echo "✅ Repository ready at: ${TEMP_DIR}"
echo ""

# Build for each platform
SUCCESS_COUNT=0
FAIL_COUNT=0

for TARGET_PLATFORM in "${PLATFORMS[@]}"; do
    BUILD_SCRIPT=$(get_build_script "$TARGET_PLATFORM")
    SOURCE_LIB=$(get_lib_info "$TARGET_PLATFORM")

    if [ -z "$BUILD_SCRIPT" ]; then
        echo "⚠️  Warning: No build script for $TARGET_PLATFORM - skipping"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi

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

    echo "--------------------------------------"
    echo "Building for: $TARGET_PLATFORM"
    echo "Build script: $BUILD_SCRIPT"
    echo ""

    # Check if we can build for this platform on current system
    CAN_BUILD=false
    case "$CURRENT_OS/$TARGET_PLATFORM" in
        Linux/Linux/*)
            CAN_BUILD=true
            ;;
        Darwin/macOS/*)
            CAN_BUILD=true
            ;;
        MINGW*/Windows/*|MSYS*/Windows/*)
            CAN_BUILD=true
            ;;
        *)
            CAN_BUILD=false
            ;;
    esac

    if [ "$CAN_BUILD" = false ]; then
        echo "⚠️  Warning: Cannot build $TARGET_PLATFORM on $CURRENT_OS"
        echo "   Please build on the target platform or use --all with download script"
        echo ""
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi

    # Run build script
    cd "${TEMP_DIR}/examples"

    if [ ! -f "$BUILD_SCRIPT" ]; then
        echo "❌ Error: Build script not found: $BUILD_SCRIPT"
        echo ""
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi

    chmod +x "$BUILD_SCRIPT"

    # Execute platform-specific build
    if [[ "$BUILD_SCRIPT" == *.bat ]]; then
        # Windows batch file
        cmd.exe //c "$BUILD_SCRIPT" || {
            echo "❌ Build failed for $TARGET_PLATFORM"
            echo ""
            FAIL_COUNT=$((FAIL_COUNT + 1))
            continue
        }
    else
        # Unix shell script
        ./"$BUILD_SCRIPT" || {
            echo "❌ Build failed for $TARGET_PLATFORM"
            echo ""
            FAIL_COUNT=$((FAIL_COUNT + 1))
            continue
        }
    fi

    # Copy built library
    if [ -f "$SOURCE_LIB" ]; then
        cp "$SOURCE_LIB" "$DEST_DIR/"
        chmod +x "$DEST_DIR"/*

        echo "✅ Build successful: $TARGET_PLATFORM"

        # Show file info
        if command -v file &> /dev/null; then
            file "$DEST_DIR"/* | head -1 || true
        fi

        SIZE=$(du -sh "$DEST_DIR" | cut -f1)
        echo "Size: $SIZE"
        echo ""
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    else
        echo "❌ Error: Built library not found at $SOURCE_LIB"
        echo ""
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
done

# Summary
echo "======================================"
echo "Build Summary"
echo "======================================"
echo ""
echo "Successful: $SUCCESS_COUNT"
echo "Failed: $FAIL_COUNT"
echo ""

if [ $SUCCESS_COUNT -gt 0 ]; then
    echo "✅ Libraries are ready!"
    echo ""
    echo "Built libraries:"
    find "${PROJECT_ROOT}/src-tauri/libs" -name "*ten_vad*" -type f 2>/dev/null || true
    echo ""
    echo "You can now build the project:"
    echo "  pnpm tauri dev"
else
    echo "❌ No libraries were built"
    exit 1
fi

# Cleanup
echo ""
read -p "Do you want to remove temporary build files? (y/N): " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Cleaning up temporary files..."
    rm -rf "${TEMP_DIR}"
    echo "✅ Cleanup complete"
else
    echo "Temporary files kept at: ${TEMP_DIR}"
fi
