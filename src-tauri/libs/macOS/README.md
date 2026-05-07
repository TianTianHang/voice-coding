# TEN VAD Library for macOS

This directory contains the TEN VAD library for macOS.

## Installation

### Automatic (Recommended)

```bash
# Auto-detect platform and download
./scripts/download_ten_vad.sh
```

### Manual

1. Download from [TEN VAD Releases](https://github.com/TEN-framework/ten-vad/releases)
2. Place `libten_vad.dylib` in this directory

## System Requirements

- macOS 10.15 (Catalina) or later
- Xcode Command Line Tools

Install Xcode Command Line Tools:
```bash
xcode-select --install
```

## Architecture

### x86_64 (Intel Mac)

- Library: `libten_vad.dylib`
- For: Intel-based Macs (2019 and earlier)

### arm64 (Apple Silicon)

- Library: `libten_vad.dylib`
- For: Apple Silicon Macs (M1, M2, M3, etc.)

Check your architecture:
```bash
uname -m
# x86_64 for Intel, arm64 for Apple Silicon
```

## Verification

```bash
# Check file type
file libten_vad.dylib

# Check dependencies
otool -L libten_vad.dylib

# Check architecture
lipo -info libten_vad.dylib
```

Expected output:
- File type: `Mach-O 64-bit dynamically linked shared library`
- Architecture depends on your Mac (x86_64 or arm64)

## Troubleshooting

### Library not found

Error: `dyld: Library not loaded`

Solution: Ensure the library exists and has correct permissions:
```bash
chmod +x libten_vad.dylib
```

### Wrong architecture

Error: `wrong CPU type` or `bad CPU type`

Solution: Download the correct version for your Mac:
- Intel Macs: use `macOS/x86_64/libten_vad.dylib`
- Apple Silicon: use `macOS/arm64/libten_vad.dylib`

### Code signing issues

If you get code signing errors during development:

```bash
# Remove extended attributes (if any)
xattr -cr libten_vad.dylib

# Sign the library (optional for development)
codesign --force --deep -s - libten_vad.dylib
```

## Dependencies

The library has minimal external dependencies:
- System libraries: `libSystem.dylib`, `libc++.1.dylib`, `libgcc_s.1.dylib`

These are included with macOS, so no additional installation is needed.

## More Information

- [Main README](../README.md)
- [TEN VAD Repository](https://github.com/TEN-framework/ten-vad)
