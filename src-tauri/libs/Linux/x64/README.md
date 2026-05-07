# TEN VAD Library

This directory should contain the `libten_vad.so` library file.

## Quick Start

### Option 1: Build from Source (Recommended)

```bash
cd /path/to/voice-coding
./scripts/build_ten_vad.sh
```

This will clone the ten-vad repository and build the library from source.

### Option 2: Download Prebuilt Library

```bash
cd /path/to/voice-coding
./scripts/download_ten_vad.sh
```

### Option 3: Manual Setup

1. Clone the ten-vad repository:
```bash
git clone --depth 1 --branch v1.0 https://github.com/TEN-framework/ten-vad.git /tmp/ten-vad
```

2. Copy the library:
```bash
cp /tmp/ten-vad/lib/Linux/x64/libten_vad.so src-tauri/libs/Linux/x64/
```

## System Requirements

- Linux x64
- libc++1 (install with: `sudo apt update && sudo apt install libc++1`)

## Verification

After installation, verify the library:

```bash
file src-tauri/libs/Linux/x64/libten_vad.so
ldd src-tauri/libs/Linux/x64/libten_vad.so
```

Expected output:
- File type: `ELF 64-bit LSB shared object`
- Dependencies: `libc.so.6`, `libstdc++.so.6`, `libc++.so.1`, `libm.so.6`, `libgcc_s.so.1`

## Troubleshooting

### Library not found

If you get an error about `libten_vad.so` not found, make sure:
1. The file exists at `src-tauri/libs/Linux/x64/libten_vad.so`
2. The file is readable: `chmod +r src-tauri/libs/Linux/x64/libten_vad.so`
3. All dependencies are installed (check with `ldd`)

### Missing libc++

Error: `libten_vad.so: cannot open shared object file: No such file or directory`

Solution: Install libc++1
```bash
sudo apt update
sudo apt install libc++1
```

## Version Information

- Current version: v1.0
- Repository: https://github.com/TEN-framework/ten-vad
- License: Apache License 2.0

