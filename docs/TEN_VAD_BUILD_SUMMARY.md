# TEN VAD 编译脚本 - 使用总结

## 已创建的文件

### 1. 编译脚本

#### `scripts/build_ten_vad.sh`
从源码编译 ten-vad 库的脚本。
- 克隆官方仓库
- 运行官方构建脚本
- 自动复制编译好的库到正确位置

#### `scripts/download_ten_vad.sh`
下载预编译库的脚本。
- 克隆官方仓库
- 复制预编译的库文件
- 更快速，无需编译

### 2. 文档

#### `src-tauri/libs/Linux/x64/README.md`
库文件的详细说明，包含：
- 安装方法
- 系统要求
- 故障排除
- 版本信息

#### `docs/TEN_VAD_SETUP.md`
完整的设置指南，包含：
- 快速开始
- 系统要求 (Ubuntu/Debian/NixOS)
- 验证方法
- 常见问题

### 3. npm 脚本

在 `package.json` 中添加了以下快捷命令：

```json
{
  "vad:build": "./scripts/build_ten_vad.sh",
  "vad:download": "./scripts/download_ten_vad.sh",
  "vad:setup": "npm run vad:build || npm run vad:download"
}
```

## 使用方法

### 最简单的方式

```bash
# 安装依赖并下载库
npm run vad:download

# 启动开发服务器
pnpm tauri dev
```

### 验证安装

```bash
# 检查库文件
file src-tauri/libs/Linux/x64/libten_vad.so

# 检查依赖
ldd src-tauri/libs/Linux/x64/libten_vad.so

# 编译检查
cd src-tauri && cargo check
```

## 集成状态

✅ 库文件已安装: `src-tauri/libs/Linux/x64/libten_vad.so`
✅ 项目编译成功 (cargo check 通过)
✅ FFI 封装已完成: `src/vad/engine.rs`
✅ 状态机已实现: `src/vad/state_machine.rs`
✅ Tauri 命令已集成: `src/vad_commands.rs`

## 关键特性

1. **自动检测和安装**: 脚本会自动检测系统依赖并提示缺失的库
2. **回退机制**: `vad:setup` 命令会在下载失败时自动尝试编译
3. **验证功能**: 脚本会验证库文件的完整性和依赖关系
4. **跨平台**: 支持从源码构建，适用于不同 Linux 发行版

## 系统依赖

- **必需**: libc++1 (Ubuntu: `sudo apt install libc++1`)
- **构建工具**: cmake, clang 或 gcc, git
- **空间**: ~500MB 用于构建 (下载方式 ~50MB)

## 下一步

1. 测试 VAD 功能: `pnpm tauri dev`
2. 查看设计文档: `openspec/changes/backend-vad-recording/design.md`
3. 阅读官方文档: https://github.com/TEN-framework/ten-vad

## 故障排除

如遇到问题，请查看:
1. `docs/TEN_VAD_SETUP.md` - 详细安装指南
2. `src-tauri/libs/Linux/x64/README.md` - 库文件说明
3. 官方仓库: https://github.com/TEN-framework/ten-vad
