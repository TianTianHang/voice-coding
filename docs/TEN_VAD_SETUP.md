# TEN VAD 编译和安装指南

本文档说明如何为 voice-coding 项目安装 TEN VAD 库。

## 支持的平台

| 平台 | 架构 | 状态 |
|------|------|------|
| Linux | x86_64, arm64 | ✅ 完全支持 |
| macOS | x86_64 (Intel), arm64 (Apple Silicon) | ✅ 完全支持 |
| Windows | x64, x86 | ✅ 完全支持 |

## 快速开始

### 方法 1: 使用 npm 脚本 (推荐)

```bash
# 自动检测平台并下载
pnpm run vad:download

# 从源码构建当前平台
pnpm run vad:build

# 自动尝试下载，失败则构建
pnpm run vad:setup
```

### 方法 2: 直接运行脚本

```bash
# 下载当前平台的预编译库
./scripts/download_ten_vad.sh

# 下载所有平台（用于 CI/CD）
./scripts/download_ten_vad.sh --all

# 下载特定平台
./scripts/download_ten_vad.sh --platform=macOS/arm64

# 从源码构建
./scripts/build_ten_vad.sh
```

## 平台特定说明

### Linux (Ubuntu/Debian)

#### 安装依赖

```bash
sudo apt update
sudo apt install -y git cmake clang libc++1
```

#### 验证安装

```bash
# 检查库文件
file src-tauri/libs/Linux/x64/libten_vad.so

# 检查依赖
ldd src-tauri/libs/Linux/x64/libten_vad.so
```

#### NixOS

在 NixOS 上，需要手动安装 libc++：

```nix
# 在 configuration.nix 中添加
environment.systemPackages = with pkgs; [
  libc++
  libc++abi
];
```

或使用 nix-shell：
```bash
nix-shell -p libc++ libc++abi
```

### macOS

#### 安装依赖

```bash
# 安装 Xcode 命令行工具
xcode-select --install

# 或使用 Homebrew
brew install cmake
```

#### 验证安装

```bash
# 检查库文件
file src-tauri/libs/macOS/$(uname -m)/libten_vad.dylib

# 检查依赖
otool -L src-tauri/libs/macOS/$(uname -m)/libten_vad.dylib

# 检查架构
lipo -info src-tauri/libs/macOS/*/libten_vad.dylib
```

#### 架构说明

- **x86_64**: Intel Macs (2019 及更早)
- **arm64**: Apple Silicon Macs (M1, M2, M3 等)

检查你的架构：
```bash
uname -m
# x86_64 = Intel
# arm64 = Apple Silicon
```

### Windows

#### 安装依赖

1. 安装 [Visual C++ Redistributable](https://aka.ms/vs/17/release/vc_redist.x64.exe)
2. 安装 [Git for Windows](https://git-scm.com/download/win)
3. 安装 [CMake](https://cmake.org/download/)

#### 验证安装

使用 PowerShell：
```powershell
# 检查文件
Test-Path src-tauri\libs\Windows\x64\ten_vad.dll

# 获取文件信息
Get-Item src-tauri\libs\Windows\x64\ten_vad.dll | Select-Object Name, Length
```

#### 架构说明

- **x64**: 64 位 Windows
- **x86**: 32 位 Windows

检查你的架构：
```powershell
echo $env:PROCESSOR_ARCHITECTURE
```

## 常见问题

### Linux

#### 缺少 libc++ 错误

**错误信息:**
```
libten_vad.so: cannot open shared object file
ldd: libc++.so.1 => not found
```

**解决方案:**
```bash
sudo apt update && sudo apt install libc++1
```

### macOS

#### 代码签名问题

如果开发时遇到代码签名错误：

```bash
# 移除扩展属性
xattr -cr src-tauri/libs/macOS/*/libten_vad.dylib

# 签名库文件（可选）
codesign --force --deep -s - src-tauri/libs/macOS/*/libten_vad.dylib
```

#### 架构不匹配

错误: `wrong CPU type`

解决方案: 确保下载了正确架构的库：
- Intel Mac: `macOS/x86_64/libten_vad.dylib`
- Apple Silicon: `macOS/arm64/libten_vad.dylib`

### Windows

#### DLL 加载失败

**错误信息:**
```
ten_vad.dll not found
VCRUNTIME140.dll not found
```

**解决方案:**

1. 确保架构匹配（64 位应用用 x64，32 位用 x86）
2. 安装 Visual C++ Redistributable:
   - 64 位: https://aka.ms/vs/17/release/vc_redist.x64.exe
   - 32 位: https://aka.ms/vs/17/release/vc_redist.x86.exe

#### DLL 被阻止

Windows 可能阻止从互联网下载的 DLL：

```powershell
# 解除阻止
Unblock-File src-tauri\libs\Windows\*\ten_vad.dll
```

### 通用问题

#### 库文件权限问题

确保库文件可执行：

```bash
# Linux/macOS
chmod +x src-tauri/libs/*/*/libten_vad*
```

#### 编译失败

检查：
1. 已安装必需工具（cmake, 编译器）
2. 有足够磁盘空间（~500MB）
3. 网络连接正常

## 目录结构

安装后的目录结构：

```
src-tauri/libs/
├── Linux/
│   ├── x64/
│   │   └── libten_vad.so
│   └── arm64/
│       └── libten_vad.so
├── macOS/
│   ├── x86_64/
│   │   └── libten_vad.dylib
│   └── arm64/
│       └── libten_vad.dylib
└── Windows/
    ├── x64/
    │   └── ten_vad.dll
    └── x86/
        └── ten_vad.dll
```

## 开发流程

### 1. 安装 TEN VAD 库

```bash
pnpm run vad:download
```

### 2. 启动开发服务器

```bash
ppnpm tauri dev
```

### 3. 测试语音活动检测功能

应用会自动加载正确的平台库。

## 生产构建

生产构建会自动将所有平台的库打包：

```bash
pnpm tauri build
```

Tauri 会根据目标平台自动包含正确的库文件。

## CI/CD 集成

在 CI/CD 环境中，建议下载所有平台的库：

```bash
# 下载所有平台
./scripts/download_ten_vad.sh --all
```

这样可以确保构建可以为任何平台创建分发版本。

## 更多信息

- [TEN VAD 官方仓库](https://github.com/TEN-framework/ten-vad)
- [TEN VAD 官方文档](https://github.com/TEN-framework/ten-vad/blob/main/README.md)
- [项目设计文档](../../openspec/changes/backend-vad-recording/design.md)
- [库文件目录](../src-tauri/libs/README.md)
