# TEN VAD Library for Windows

This directory contains the TEN VAD library for Windows.

## Installation

### Automatic (Recommended)

```bash
# Using Git Bash or WSL
./scripts/download_ten_vad.sh

# Or use PowerShell
./scripts/download_ten_vad.ps1
```

### Manual

1. Download from [TEN VAD Releases](https://github.com/TEN-framework/ten-vad/releases)
2. Place `ten_vad.dll` in the appropriate architecture directory

## System Requirements

- Windows 10 or later
- Visual C++ Redistributable (usually pre-installed)

### Installing Visual C++ Redistributable

If you get "MSVCP140.dll not found" error:

1. Download from [Microsoft's website](https://aka.ms/vs/17/release/vc_redist.x64.exe)
2. Install the redistributable
3. Restart your application

## Architecture

### x64 (64-bit)

- Library: `ten_vad.dll`
- For: 64-bit Windows applications
- Directory: `Windows/x64/`

### x86 (32-bit)

- Library: `ten_vad.dll`
- For: 32-bit Windows applications
- Directory: `Windows/x86/`

Check your Windows architecture:
```powershell
# PowerShell
echo $env:PROCESSOR_ARCHITECTURE

# CMD
echo %PROCESSOR_ARCHITECTURE
```

## Verification

### Using Dependency Walker

1. Download [Dependencies](https://github.com/lucasg/Dependencies)
2. Open `ten_vad.dll` in the tool
3. Check for missing dependencies

### Using PowerShell

```powershell
# Check file exists
Test-Path ten_vad.dll

# Get file info
Get-Item ten_vad.dll | Select-Object Name, Length

# Check file details
Get-Item ten_vad.dll | Format-List *
```

## Troubleshooting

### DLL not found

Error: `ten_vad.dll not found` or `Can't load DLL`

Solutions:
1. Ensure the DLL is in the correct directory
2. Check that the architecture matches (x64 vs x86)
3. Verify the file isn't blocked by Windows:
   ```powershell
   Unblock-File ten_vad.dll
   ```

### Missing MSVC runtime

Error: `VCRUNTIME140.dll not found` or `MSVCP140.dll not found`

Solution: Install Visual C++ Redistributable:
- Download from: https://aka.ms/vs/17/release/vc_redist.x64.exe
- Or: https://aka.ms/vs/17/release/vc_redist.x86.exe (for 32-bit)

### Wrong architecture

Error: `%1 is not a valid Win32 application` or `wrong CPU type`

Solution: Ensure you're using the correct DLL for your application:
- 64-bit applications → `Windows/x64/ten_vad.dll`
- 32-bit applications → `Windows/x86/ten_vad.dll`

### DLL blocked by Windows

Windows may block DLLs downloaded from the internet:

```powershell
# Unblock the DLL
Unblock-File ten_vad.dll

# Or unblock all files in directory
Get-ChildItem -Path . -Recurse | Unblock-File
```

## Dependencies

The library requires:
- `VCRUNTIME140.dll` (Visual C++ Runtime)
- `MSVCP140.dll` (C++ Standard Library)
- Windows system libraries (usually pre-installed)

These are typically included with:
- Visual C++ Redistributable
- Visual Studio installation
- Windows 10/11 updates

## Building from Source

If you need to build the DLL yourself:

1. Install Visual Studio 2019 or later with C++ support
2. Install CMake
3. Run the build script:
   ```bash
   ./scripts/build_ten_vad.sh --platform=Windows/x64
   ```

Or manually:
```bash
git clone https://github.com/TEN-framework/ten-vad.git
cd ten-vad/examples
build-and-deploy-windows.bat
```

## Development Notes

- The DLL should be placed alongside your executable
- Tauri will automatically bundle the DLL in production builds
- In development, ensure the DLL is in `libs/Windows/<arch>/`

## More Information

- [Main README](../README.md)
- [TEN VAD Repository](https://github.com/TEN-framework/ten-vad)
- [Visual C++ Redistributable Download](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)
