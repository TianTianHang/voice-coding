# Tauri 权限系统配置

本文档记录了 Tauri 2.0 的权限系统配置和可用权限列表。

## 项目当前配置

### tauri.conf.json
```json
{
  "app": {
    "windows": [
      {
        "title": "voice-coding",
        "width": 800,
        "height": 600
      }
    ],
    "security": {
      "csp": null
    }
  }
}
```

### capabilities/default.json
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default"
  ]
}
```

## 权限系统工作原理

Tauri 使用 **Capabilities** 系统来控制前端对不同功能的访问权限：

```
前端请求 Tauri 功能
        ↓
Tauri 检查 capabilities 配置
        ↓
匹配窗口/插件 → 检查权限列表
        ↓
允许/拒绝访问
```

### 关键概念

- **Capability（能力）**: 一组权限的集合，可以应用到特定窗口
- **Windows**: 应用中的窗口，通过 label 标识
- **Permissions**: 具体的功能权限，格式为 `插件名:权限名` 或自定义命令
- **Scopes**: 某些权限可以配置详细的访问范围

### 权限文件位置

```
src-tauri/
├── tauri.conf.json      # 主配置文件
├── capabilities/
│   └── default.json     # 权限定义文件
└── gen/schemas/
    └── desktop-schema.json  # 权限的 JSON Schema
```

## 完整权限表

### Default Permissions (core:default)

`core:default` 权限自动包含以下所有默认权限：

- `core:app:default`
- `core:event:default`
- `core:image:default`
- `core:menu:default`
- `core:path:default`
- `core:resources:default`
- `core:tray:default`
- `core:webview:default`
- `core:window:default`

---

### App 权限

**默认权限** (`core:app:default`) 包含：
- `allow-version`
- `allow-name`
- `allow-tauri-version`

| 权限标识符 | 描述 |
|-----------|------|
| `core:app:allow-app-hide` | 允许隐藏应用 |
| `core:app:deny-app-hide` | 拒绝隐藏应用 |
| `core:app:allow-app-show` | 允许显示应用 |
| `core:app:deny-app-show` | 拒绝显示应用 |
| `core:app:allow-default-window-icon` | 允许设置默认窗口图标 |
| `core:app:deny-default-window-icon` | 拒绝设置默认窗口图标 |
| `core:app:allow-name` | 允许获取应用名称 |
| `core:app:deny-name` | 拒绝获取应用名称 |
| `core:app:allow-set-app-theme` | 允许设置应用主题 |
| `core:app:deny-set-app-theme` | 拒绝设置应用主题 |
| `core:app:allow-tauri-version` | 允许获取 Tauri 版本 |
| `core:app:deny-tauri-version` | 拒绝获取 Tauri 版本 |
| `core:app:allow-version` | 允许获取应用版本 |
| `core:app:deny-version` | 拒绝获取应用版本 |

---

### Event 权限

**默认权限** (`core:event:default`) 包含：
- `allow-listen`
- `allow-unlisten`
- `allow-emit`
- `allow-emit-to`

| 权限标识符 | 描述 |
|-----------|------|
| `core:event:allow-emit` | 允许发送事件 |
| `core:event:deny-emit` | 拒绝发送事件 |
| `core:event:allow-emit-to` | 允许向特定目标发送事件 |
| `core:event:deny-emit-to` | 拒绝向特定目标发送事件 |
| `core:event:allow-listen` | 允许监听事件 |
| `core:event:deny-listen` | 拒绝监听事件 |
| `core:event:allow-unlisten` | 允许取消监听事件 |
| `core:event:deny-unlisten` | 拒绝取消监听事件 |

---

### Image 权限

**默认权限** (`core:image:default`) 包含：
- `allow-new`
- `allow-from-bytes`
- `allow-from-path`
- `allow-rgba`
- `allow-size`

| 权限标识符 | 描述 |
|-----------|------|
| `core:image:allow-from-bytes` | 允许从字节创建图片 |
| `core:image:deny-from-bytes` | 拒绝从字节创建图片 |
| `core:image:allow-from-path` | 允许从路径创建图片 |
| `core:image:deny-from-path` | 拒绝从路径创建图片 |
| `core:image:allow-new` | 允许创建新图片 |
| `core:image:deny-new` | 拒绝创建新图片 |
| `core:image:allow-rgba` | 允许获取 RGBA 数据 |
| `core:image:deny-rgba` | 拒绝获取 RGBA 数据 |
| `core:image:allow-size` | 允许获取图片尺寸 |
| `core:image:deny-size` | 拒绝获取图片尺寸 |

---

### Menu 权限

**默认权限** (`core:menu:default`) 包含所有菜单相关权限。

| 权限标识符 | 描述 |
|-----------|------|
| `core:menu:allow-append` | 允许追加菜单项 |
| `core:menu:deny-append` | 拒绝追加菜单项 |
| `core:menu:allow-create-default` | 允许创建默认菜单 |
| `core:menu:deny-create-default` | 拒绝创建默认菜单 |
| `core:menu:allow-get` | 允许获取菜单 |
| `core:menu:deny-get` | 拒绝获取菜单 |
| `core:menu:allow-insert` | 允许插入菜单项 |
| `core:menu:deny-insert` | 拒绝插入菜单项 |
| `core:menu:allow-is-checked` | 允许检查菜单项是否被选中 |
| `core:menu:deny-is-checked` | 拒绝检查菜单项是否被选中 |
| `core:menu:allow-is-enabled` | 允许检查菜单项是否启用 |
| `core:menu:deny-is-enabled` | 拒绝检查菜单项是否启用 |
| `core:menu:allow-items` | 允许获取菜单项 |
| `core:menu:deny-items` | 拒绝获取菜单项 |
| `core:menu:allow-new` | 允许创建新菜单 |
| `core:menu:deny-new` | 拒绝创建新菜单 |
| `core:menu:allow-popup` | 允许弹出菜单 |
| `core:menu:deny-popup` | 拒绝弹出菜单 |
| `core:menu:allow-prepend` | 允许在前面添加菜单项 |
| `core:menu:deny-prepend` | 拒绝在前面添加菜单项 |
| `core:menu:allow-remove` | 允许移除菜单项 |
| `core:menu:deny-remove` | 拒绝移除菜单项 |
| `core:menu:allow-remove-at` | 允许移除指定位置的菜单项 |
| `core:menu:deny-remove-at` | 拒绝移除指定位置的菜单项 |
| `core:menu:allow-set-accelerator` | 允许设置快捷键 |
| `core:menu:deny-set-accelerator` | 拒绝设置快捷键 |
| `core:menu:allow-set-as-app-menu` | 允许设置为应用菜单 |
| `core:menu:deny-set-as-app-menu` | 拒绝设置为应用菜单 |
| `core:menu:allow-set-as-help-menu-for-nsapp` | 允许设置为帮助菜单（macOS） |
| `core:menu:deny-set-as-help-menu-for-nsapp` | 拒绝设置为帮助菜单（macOS） |
| `core:menu:allow-set-as-window-menu` | 允许设置为窗口菜单 |
| `core:menu:deny-set-as-window-menu` | 拒绝设置为窗口菜单 |
| `core:menu:allow-set-as-windows-menu-for-nsapp` | 允许设置为 Windows 菜单（macOS） |
| `core:menu:deny-set-as-windows-menu-for-nsapp` | 拒绝设置为 Windows 菜单（macOS） |
| `core:menu:allow-set-checked` | 允许设置菜单项选中状态 |
| `core:menu:deny-set-checked` | 拒绝设置菜单项选中状态 |
| `core:menu:allow-set-enabled` | 允许设置菜单项启用状态 |
| `core:menu:deny-set-enabled` | 拒绝设置菜单项启用状态 |
| `core:menu:allow-set-icon` | 允许设置菜单图标 |
| `core:menu:deny-set-icon` | 拒绝设置菜单图标 |
| `core:menu:allow-set-text` | 允许设置菜单文本 |
| `core:menu:deny-set-text` | 拒绝设置菜单文本 |
| `core:menu:allow-text` | 允许获取菜单文本 |
| `core:menu:deny-text` | 拒绝获取菜单文本 |

---

### Path 权限

**默认权限** (`core:path:default`) 包含：
- `allow-resolve-directory`
- `allow-resolve`
- `allow-normalize`
- `allow-join`
- `allow-dirname`
- `allow-extname`
- `allow-basename`
- `allow-is-absolute`

| 权限标识符 | 描述 |
|-----------|------|
| `core:path:allow-basename` | 允许获取文件名 |
| `core:path:deny-basename` | 拒绝获取文件名 |
| `core:path:allow-dirname` | 允许获取目录名 |
| `core:path:deny-dirname` | 拒绝获取目录名 |
| `core:path:allow-extname` | 允许获取扩展名 |
| `core:path:deny-extname` | 拒绝获取扩展名 |
| `core:path:allow-is-absolute` | 允许检查是否为绝对路径 |
| `core:path:deny-is-absolute` | 拒绝检查是否为绝对路径 |
| `core:path:allow-join` | 允许拼接路径 |
| `core:path:deny-join` | 拒绝拼接路径 |
| `core:path:allow-normalize` | 允许标准化路径 |
| `core:path:deny-normalize` | 拒绝标准化路径 |
| `core:path:allow-resolve` | 允许解析路径 |
| `core:path:deny-resolve` | 拒绝解析路径 |
| `core:path:allow-resolve-directory` | 允许解析目录 |
| `core:path:deny-resolve-directory` | 拒绝解析目录 |

---

### Resources 权限

**默认权限** (`core:resources:default`) 包含：
- `allow-close`

| 权限标识符 | 描述 |
|-----------|------|
| `core:resources:allow-close` | 允许关闭资源 |
| `core:resources:deny-close` | 拒绝关闭资源 |

---

### Tray 权限

**默认权限** (`core:tray:default`) 包含所有托盘相关权限。

| 权限标识符 | 描述 |
|-----------|------|
| `core:tray:allow-get-by-id` | 允许通过 ID 获取托盘 |
| `core:tray:deny-get-by-id` | 拒绝通过 ID 获取托盘 |
| `core:tray:allow-new` | 允许创建新托盘 |
| `core:tray:deny-new` | 拒绝创建新托盘 |
| `core:tray:allow-remove-by-id` | 允许通过 ID 移除托盘 |
| `core:tray:deny-remove-by-id` | 拒绝通过 ID 移除托盘 |
| `core:tray:allow-set-icon` | 允许设置托盘图标 |
| `core:tray:deny-set-icon` | 拒绝设置托盘图标 |
| `core:tray:allow-set-icon-as-template` | 允许设置图标为模板（macOS） |
| `core:tray:deny-set-icon-as-template` | 拒绝设置图标为模板（macOS） |
| `core:tray:allow-set-menu` | 允许设置托盘菜单 |
| `core:tray:deny-set-menu` | 拒绝设置托盘菜单 |
| `core:tray:allow-set-show-menu-on-left-click` | 允许设置左键显示菜单 |
| `core:tray:deny-set-show-menu-on-left-click` | 拒绝设置左键显示菜单 |
| `core:tray:allow-set-temp-dir-path` | 允许设置临时目录路径 |
| `core:tray:deny-set-temp-dir-path` | 拒绝设置临时目录路径 |
| `core:tray:allow-set-title` | 允许设置托盘标题 |
| `core:tray:deny-set-title` | 拒绝设置托盘标题 |
| `core:tray:allow-set-tooltip` | 允许设置托盘提示 |
| `core:tray:deny-set-tooltip` | 拒绝设置托盘提示 |
| `core:tray:allow-set-visible` | 允许设置托盘可见性 |
| `core:tray:deny-set-visible` | 拒绝设置托盘可见性 |

---

### Webview 权限

**默认权限** (`core:webview:default`) 包含：
- `allow-get-all-webviews`
- `allow-webview-position`
- `allow-webview-size`
- `allow-internal-toggle-devtools`

| 权限标识符 | 描述 |
|-----------|------|
| `core:webview:allow-clear-all-browsing-data` | 允许清除浏览数据 |
| `core:webview:deny-clear-all-browsing-data` | 拒绝清除浏览数据 |
| `core:webview:allow-create-webview` | 允许创建 webview |
| `core:webview:deny-create-webview` | 拒绝创建 webview |
| `core:webview:allow-create-webview-window` | 允许创建 webview 窗口 |
| `core:webview:deny-create-webview-window` | 拒绝创建 webview 窗口 |
| `core:webview:allow-get-all-webviews` | 允许获取所有 webview |
| `core:webview:deny-get-all-webviews` | 拒绝获取所有 webview |
| `core:webview:allow-internal-toggle-devtools` | 允许切换开发者工具 |
| `core:webview:deny-internal-toggle-devtools` | 拒绝切换开发者工具 |
| `core:webview:allow-print` | 允许打印 |
| `core:webview:deny-print` | 拒绝打印 |
| `core:webview:allow-reparent` | 允许重新指定父窗口 |
| `core:webview:deny-reparent` | 拒绝重新指定父窗口 |
| `core:webview:allow-set-webview-focus` | 允许设置 webview 焦点 |
| `core:webview:deny-set-webview-focus` | 拒绝设置 webview 焦点 |
| `core:webview:allow-set-webview-position` | 允许设置 webview 位置 |
| `core:webview:deny-set-webview-position` | 拒绝设置 webview 位置 |
| `core:webview:allow-set-webview-size` | 允许设置 webview 尺寸 |
| `core:webview:deny-set-webview-size` | 拒绝设置 webview 尺寸 |
| `core:webview:allow-set-webview-zoom` | 允许设置 webview 缩放 |
| `core:webview:deny-set-webview-zoom` | 拒绝设置 webview 缩放 |
| `core:webview:allow-webview-close` | 允许关闭 webview |
| `core:webview:deny-webview-close` | 拒绝关闭 webview |
| `core:webview:allow-webview-hide` | 允许隐藏 webview |
| `core:webview:deny-webview-hide` | 拒绝隐藏 webview |
| `core:webview:allow-webview-position` | 允许获取 webview 位置 |
| `core:webview:deny-webview-position` | 拒绝获取 webview 位置 |
| `core:webview:allow-webview-show` | 允许显示 webview |
| `core:webview:deny-webview-show` | 拒绝显示 webview |
| `core:webview:allow-webview-size` | 允许获取 webview 尺寸 |
| `core:webview:deny-webview-size` | 拒绝获取 webview 尺寸 |

---

### Window 权限

**默认权限** (`core:window:default`) 包含大量窗口操作权限。

| 权限标识符 | 描述 |
|-----------|------|
| `core:window:allow-available-monitors` | 允许获取可用显示器列表 |
| `core:window:deny-available-monitors` | 拒绝获取可用显示器列表 |
| `core:window:allow-center` | 允许居中窗口 |
| `core:window:deny-center` | 拒绝居中窗口 |
| `core:window:allow-close` | 允许关闭窗口 |
| `core:window:deny-close` | 拒绝关闭窗口 |
| `core:window:allow-create` | 允许创建窗口 |
| `core:window:deny-create` | 拒绝创建窗口 |
| `core:window:allow-current-monitor` | 允许获取当前显示器 |
| `core:window:deny-current-monitor` | 拒绝获取当前显示器 |
| `core:window:allow-cursor-position` | 允许获取光标位置 |
| `core:window:deny-cursor-position` | 拒绝获取光标位置 |
| `core:window:allow-destroy` | 允许销毁窗口 |
| `core:window:deny-destroy` | 拒绝销毁窗口 |
| `core:window:allow-get-all-windows` | 允许获取所有窗口 |
| `core:window:deny-get-all-windows` | 拒绝获取所有窗口 |
| `core:window:allow-hide` | 允许隐藏窗口 |
| `core:window:deny-hide` | 拒绝隐藏窗口 |
| `core:window:allow-inner-position` | 允许获取窗口内部位置 |
| `core:window:deny-inner-position` | 拒绝获取窗口内部位置 |
| `core:window:allow-inner-size` | 允许获取窗口内部尺寸 |
| `core:window:deny-inner-size` | 拒绝获取窗口内部尺寸 |
| `core:window:allow-internal-toggle-maximize` | 允许切换最大化状态 |
| `core:window:deny-internal-toggle-maximize` | 拒绝切换最大化状态 |
| `core:window:allow-is-closable` | 允许检查窗口是否可关闭 |
| `core:window:deny-is-closable` | 拒绝检查窗口是否可关闭 |
| `core:window:allow-is-decorated` | 允许检查窗口是否有装饰 |
| `core:window:deny-is-decorated` | 拒绝检查窗口是否有装饰 |
| `core:window:allow-is-enabled` | 允许检查窗口是否启用 |
| `core:window:deny-is-enabled` | 拒绝检查窗口是否启用 |
| `core:window:allow-is-focused` | 允许检查窗口是否有焦点 |
| `core:window:deny-is-focused` | 拒绝检查窗口是否有焦点 |
| `core:window:allow-is-fullscreen` | 允许检查窗口是否全屏 |
| `core:window:deny-is-fullscreen` | 拒绝检查窗口是否全屏 |
| `core:window:allow-is-maximizable` | 允许检查窗口是否可最大化 |
| `core:window:deny-is-maximizable` | 拒绝检查窗口是否可最大化 |
| `core:window:allow-is-maximized` | 允许检查窗口是否已最大化 |
| `core:window:deny-is-maximized` | 拒绝检查窗口是否已最大化 |
| `core:window:allow-is-minimizable` | 允许检查窗口是否可最小化 |
| `core:window:deny-is-minimizable` | 拒绝检查窗口是否可最小化 |
| `core:window:allow-is-minimized` | 允许检查窗口是否已最小化 |
| `core:window:deny-is-minimized` | 拒绝检查窗口是否已最小化 |
| `core:window:allow-is-resizable` | 允许检查窗口是否可调整大小 |
| `core:window:deny-is-resizable` | 拒绝检查窗口是否可调整大小 |
| `core:window:allow-is-visible` | 允许检查窗口是否可见 |
| `core:window:deny-is-visible` | 拒绝检查窗口是否可见 |
| `core:window:allow-maximize` | 允许最大化窗口 |
| `core:window:deny-maximize` | 拒绝最大化窗口 |
| `core:window:allow-minimize` | 允许最小化窗口 |
| `core:window:deny-minimize` | 拒绝最小化窗口 |
| `core:window:allow-monitor-from-point` | 允许从点获取显示器 |
| `core:window:deny-monitor-from-point` | 拒绝从点获取显示器 |
| `core:window:allow-outer-position` | 允许获取窗口外部位置 |
| `core:window:deny-outer-position` | 拒绝获取窗口外部位置 |
| `core:window:allow-outer-size` | 允许获取窗口外部尺寸 |
| `core:window:deny-outer-size` | 拒绝获取窗口外部尺寸 |
| `core:window:allow-primary-monitor` | 允许获取主显示器 |
| `core:window:deny-primary-monitor` | 拒绝获取主显示器 |
| `core:window:allow-request-user-attention` | 允许请求用户注意 |
| `core:window:deny-request-user-attention` | 拒绝请求用户注意 |
| `core:window:allow-scale-factor` | 允许获取缩放因子 |
| `core:window:deny-scale-factor` | 拒绝获取缩放因子 |
| `core:window:allow-set-always-on-bottom` | 允许设置窗口总是在底部 |
| `core:window:deny-set-always-on-bottom` | 拒绝设置窗口总是在底部 |
| `core:window:allow-set-always-on-top` | 允许设置窗口总是在顶部 |
| `core:window:deny-set-always-on-top` | 拒绝设置窗口总是在顶部 |
| `core:window:allow-set-closable` | 允许设置窗口是否可关闭 |
| `core:window:deny-set-closable` | 拒绝设置窗口是否可关闭 |
| `core:window:allow-set-content-protected` | 允许设置内容保护 |
| `core:window:deny-set-content-protected` | 拒绝设置内容保护 |
| `core:window:allow-set-cursor-grab` | 允许设置光标抓取 |
| `core:window:deny-set-cursor-grab` | 拒绝设置光标抓取 |
| `core:window:allow-set-cursor-icon` | 允许设置光标图标 |
| `core:window:deny-set-cursor-icon` | 拒绝设置光标图标 |
| `core:window:allow-set-cursor-position` | 允许设置光标位置 |
| `core:window:deny-set-cursor-position` | 拒绝设置光标位置 |
| `core:window:allow-set-cursor-visible` | 允许设置光标可见性 |
| `core:window:deny-set-cursor-visible` | 拒绝设置光标可见性 |
| `core:window:allow-set-decorations` | 允许设置窗口装饰 |
| `core:window:deny-set-decorations` | 拒绝设置窗口装饰 |
| `core:window:allow-set-effects` | 允许设置窗口效果 |
| `core:window:deny-set-effects` | 拒绝设置窗口效果 |
| `core:window:allow-set-enabled` | 允许设置窗口启用状态 |
| `core:window:deny-set-enabled` | 拒绝设置窗口启用状态 |
| `core:window:allow-set-focus` | 允许设置窗口焦点 |
| `core:window:deny-set-focus` | 拒绝设置窗口焦点 |
| `core:window:allow-set-fullscreen` | 允许设置全屏 |
| `core:window:deny-set-fullscreen` | 拒绝设置全屏 |
| `core:window:allow-set-icon` | 允许设置窗口图标 |
| `core:window:deny-set-icon` | 拒绝设置窗口图标 |
| `core:window:allow-set-ignore-cursor-events` | 允许设置忽略光标事件 |
| `core:window:deny-set-ignore-cursor-events` | 拒绝设置忽略光标事件 |
| `core:window:allow-set-max-size` | 允许设置最大尺寸 |
| `core:window:deny-set-max-size` | 拒绝设置最大尺寸 |
| `core:window:allow-set-maximizable` | 允许设置是否可最大化 |
| `core:window:deny-set-maximizable` | 拒绝设置是否可最大化 |
| `core:window:allow-set-min-size` | 允许设置最小尺寸 |
| `core:window:deny-set-min-size` | 拒绝设置最小尺寸 |
| `core:window:allow-set-minimizable` | 允许设置是否可最小化 |
| `core:window:deny-set-minimizable` | 拒绝设置是否可最小化 |
| `core:window:allow-set-position` | 允许设置窗口位置 |
| `core:window:deny-set-position` | 拒绝设置窗口位置 |
| `core:window:allow-set-progress-bar` | 允许设置进度条 |
| `core:window:deny-set-progress-bar` | 拒绝设置进度条 |
| `core:window:allow-set-resizable` | 允许设置是否可调整大小 |
| `core:window:deny-set-resizable` | 拒绝设置是否可调整大小 |
| `core:window:allow-set-shadow` | 允许设置阴影 |
| `core:window:deny-set-shadow` | 拒绝设置阴影 |
| `core:window:allow-set-size` | 允许设置窗口尺寸 |
| `core:window:deny-set-size` | 拒绝设置窗口尺寸 |
| `core:window:allow-set-size-constraints` | 允许设置尺寸约束 |
| `core:window:deny-set-size-constraints` | 拒绝设置尺寸约束 |
| `core:window:allow-set-skip-taskbar` | 允许设置跳过任务栏 |
| `core:window:deny-set-skip-taskbar` | 拒绝设置跳过任务栏 |
| `core:window:allow-set-theme` | 允许设置窗口主题 |
| `core:window:deny-set-theme` | 拒绝设置窗口主题 |
| `core:window:allow-set-title` | 允许设置窗口标题 |
| `core:window:deny-set-title` | 拒绝设置窗口标题 |
| `core:window:allow-set-title-bar-style` | 允许设置标题栏样式 |
| `core:window:deny-set-title-bar-style` | 拒绝设置标题栏样式 |
| `core:window:allow-set-visible-on-all-workspaces` | 允许设置在所有工作区可见 |
| `core:window:deny-set-visible-on-all-workspaces` | 拒绝设置在所有工作区可见 |
| `core:window:allow-show` | 允许显示窗口 |
| `core:window:deny-show` | 拒绝显示窗口 |
| `core:window:allow-start-dragging` | 允许开始拖拽 |
| `core:window:deny-start-dragging` | 拒绝开始拖拽 |
| `core:window:allow-start-resize-dragging` | 允许开始调整大小拖拽 |
| `core:window:deny-start-resize-dragging` | 拒绝开始调整大小拖拽 |
| `core:window:allow-theme` | 允许获取窗口主题 |
| `core:window:deny-theme` | 拒绝获取窗口主题 |
| `core:window:allow-title` | 允许获取窗口标题 |
| `core:window:deny-title` | 拒绝获取窗口标题 |
| `core:window:allow-toggle-maximize` | 允许切换最大化 |
| `core:window:deny-toggle-maximize` | 拒绝切换最大化 |
| `core:window:allow-unmaximize` | 允许取消最大化 |
| `core:window:deny-unmaximize` | 拒绝取消最大化 |
| `core:window:allow-unminimize` | 允许取消最小化 |
| `core:window:deny-unminimize` | 拒绝取消最小化 |

---

## 媒体设备访问问题

### 问题描述

当前项目中使用 `navigator.mediaDevices.getUserMedia()` 访问麦克风时遇到以下错误：

```
The request is not allowed by the user agent or the platform in the current context.
```

### 问题分析

1. **Tauri Core 权限不包含媒体设备权限**
   - Tauri 的权限系统主要控制 Tauri 命令的访问
   - `getUserMedia` 是浏览器 Web API，不在 Tauri 权限范围内

2. **可能的限制来源**
   - Tauri WebView 的安全限制
   - 操作系统权限（特别是 Linux）
   - 缺少必要的配置

3. **当前配置状态**
   ```json
   {
     "app": {
       "windows": [{
         "title": "voice-coding",
         "width": 800,
         "height": 600
       }],
       "security": {
         "csp": null
       }
     }
   }
   ```

### 潜在解决方案

1. **检查 WebView 配置** - 可能需要在窗口配置中添加特定选项
2. **使用 Tauri 插件** - 可能需要专门的媒体设备插件
3. **平台特定配置** - Linux 可能需要额外的系统权限配置

### 调试步骤

1. 在浏览器模式测试（`pnpm dev`）- 确认代码本身是否正常
2. 检查系统权限设置
3. 查看 Tauri 日志获取详细错误信息
4. 参考 Tauri 社区关于媒体设备的讨论

## 参考资源

- [Tauri Capabilities 官方文档](https://v2.tauri.app/security/capabilities/)
- [Tauri 权限系统](https://v2.tauri.app/security/features/)
- [Tauri Security](https://v2.tauri.app/security/)
