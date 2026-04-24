# TEN VAD 快速参考

## 🚀 快速开始

```bash
# 一键安装（自动检测平台）
npm run vad:download

# 开始开发
pnpm tauri dev
```

## 📋 常用命令

### 安装库

```bash
# 自动检测并下载当前平台
npm run vad:download

# 或使用脚本
./scripts/download_ten_vad.sh

# 下载所有平台（用于 CI/CD）
./scripts/download_ten_vad.sh --all

# 下载特定平台
./scripts/download_ten_vad.sh --platform=macOS/arm64
```

### 从源码构建

```bash
# 构建当前平台
npm run vad:build

# 或使用脚本
./scripts/build_ten_vad.sh
```

### 验证安装

```bash
# Linux
file src-tauri/libs/Linux/x64/libten_vad.so
ldd src-tauri/libs/Linux/x64/libten_vad.so

# macOS
file src-tauri/libs/macOS/$(uname -m)/libten_vad.dylib
otool -L src-tauri/libs/macOS/$(uname -m)/libten_vad.dylib

# Windows (PowerShell)
Test-Path src-tauri\libs\Windows\x64\ten_vad.dll
Get-Item src-tauri\libs\Windows\x64\ten_vad.dll
```

## 📦 库文件位置

| 平台 | 路径 |
|------|------|
| Linux x64 | `src-tauri/libs/Linux/x64/libten_vad.so` |
| macOS Intel | `src-tauri/libs/macOS/x86_64/libten_vad.dylib` |
| macOS ARM | `src-tauri/libs/macOS/arm64/libten_vad.dylib` |
| Windows 64-bit | `src-tauri/libs/Windows/x64/ten_vad.dll` |
| Windows 32-bit | `src-tauri/libs/Windows/x86/ten_vad.dll` |

## 🔧 故障排除

### Linux: 缺少 libc++

```bash
sudo apt update && sudo apt install libc++1
```

### macOS: 代码签名问题

```bash
xattr -cr src-tauri/libs/macOS/*/libten_vad.dylib
```

### Windows: DLL 加载失败

安装 Visual C++ Redistributable:
https://aka.ms/vs/17/release/vc_redist.x64.exe

### 权限问题

```bash
chmod +x src-tauri/libs/*/*/libten_vad*
```

## 📚 详细文档

- [完整安装指南](./TEN_VAD_SETUP.md)
- [跨平台实现总结](./TEN_VAD_CROSS_PLATFORM_SUMMARY.md)
- [库文件目录说明](../src-tauri/libs/README.md)
- [项目设计文档](../openspec/changes/backend-vad-recording/design.md)

## 🔗 外部链接

- [TEN VAD 官方仓库](https://github.com/TEN-framework/ten-vad)
- [Tauri 文档](https://tauri.app/)
