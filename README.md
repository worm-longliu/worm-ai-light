# AI牛马灯

Windows 桌面悬浮灯，通过监控指定目录的文件变更，实时反馈 AI 编码助手（如 Codex CLI、Cursor 等）的工作状态。灯在屏幕一角常驻，让你无需切屏即可感知代码生成是否正在进行。

## 仓库地址

- GitHub：<https://github.com/worm-longliu/worm-ai-light>
- Gitee：<https://gitee.com/liulong_oschina/worm-ai-light>

## 🎬 抖音视频

扫描下方二维码关注我的抖音账号，获取更多 AI 编程效率工具的玩法和视频演示 👇

<p align="center">
  <img src="./douyin.jpg" alt="抖音扫码关注" width="300" />
</p>

## 使用说明

### 一句话用法

1. **启动程序** — 运行 `worm-ai-light.exe`，桌面角落出现悬浮指示灯（默认灰色）
2. **点击文字区域** — 弹出文件夹选择器，选中要监控的目录
3. **观察灯光** — 绿色常亮 = 有文件变更，黄色闪烁 = 空闲超时，灰色 = 已暂停

### 推荐监控目录

本工具**不侵入** AI 开发工具，只需监控工具**自身的配置/会话目录**，通过文件变更判断其是否在工作：

| AI 工具 | 建议监控目录 | 说明 |
|---------|-------------|------|
| **Claude Code** | `C:\Users\<你的用户名>\.claude\` | 记录会话日志、项目配置，每次交互都有文件写入 |
| **Codex CLI** | `C:\Users\<你的用户名>\.codex\` | 管理项目索引和会话状态 |
| **Cursor** | `C:\Users\<你的用户名>\.cursor\` | 后台写入索引和缓存文件 |
| **Windsurf** | `C:\Users\<你的用户名>\.windsurf\` | 保存会话状态和索引 |

> 这些目录位于用户主目录下，AI 工具在后台工作时会自动读写这些目录中的文件。本工具仅通过操作系统文件事件 API 旁听变更，不注入代码、不拦截操作、不修改任何配置。

### 工作原理

```
AI 工具（如 Claude Code、Codex CLI、Cursor）
        │
        │  工作过程中自动读写用户目录下的文件
        ▼
C:\Users\<你>\.claude\ 或 .codex\ 等
        │
        │  notify (ReadDirectoryChangesW) 检测文件变更
        ▼
    🟢 灯亮起 → 刷新空闲计时器
        │
        │  超过 N 秒无变更
        ▼
    🟡 灯闪烁（黄色）
```

## 状态说明

| 状态 | 颜色 | 效果 | 触发条件 |
|------|------|------|----------|
| `working` | 🟢 绿色 | 常亮 | 监控目录内检测到文件变更（notify 事件强制触发，即使当前为 Stopped 灰色） |
| `warning` | 🟡 黄色 | 闪烁 (0.5s) | 超过设定时间（默认 60s）无任何文件变更（仅从 Working 触发） |
| `stopped` | ⚪ 灰色 | 常亮 | 未配置监控目录 / 用户手动暂停 / 目录不存在 |

### 状态转换逻辑

```
                    ┌──────────────────────────────────────┐
                    │          未配置监控目录                │
                    │          Stopped (灰色)               │
                    └─────┬────────────────────────────────┘
                          │ 选择监控目录            ▲
                          ▼                        │
                    ┌──────────────────────────────────────┐
          ┌────────►│          Working (绿色)               │◄────────┐
          │         │  每次文件变更刷新计时器                │         │
          │         └──────────────┬───────────────────────┘         │
          │                        │ 超过 idle_timeout_secs 无变更   │
          │                        ▼                                  │
          │         ┌──────────────────────────────────────┐         │
          │         │          Warning (黄色闪烁)           │         │
          │         │  收到文件变更 → 回到 Working          │─────────┘
          │         └──────────────────────────────────────┘
          │
          │  点击灯切换
          └─────────────────────────────────────────────────────────┘

  ── 虚线：notify 文件事件强制触发（即使为灰色 Stopped 也跳转到 Working）
  ── 定时轮询（idle_timeout）仅从 Working → Warning，不触发表色状态
```

### 核心规则

1. **绿色 (Working)** — 监控到文件变更时立即亮起，同时刷新"最后工作时间"计时器
2. **黄色闪烁 (Warning)** — 距"最后工作时间"超过 `idle_timeout_secs`（默认 60 秒）无新变更时自动切换
3. **灰色 (Stopped)** — 初始状态 / 用户点击灯手动暂停 / 监控目录不存在
4. **文件事件强制触发** — notify 文件变更事件总是将状态置为 Working，即使当前为 Stopped（灰色）。灰色仅阻止定时轮询（idle_timeout）变更，不影响文件事件
5. **目录配置不受状态影响** — 任何时候都可以点击文字区域重新选择监控目录，选择后自动恢复 Working
6. **超时可自定义** — 通过颜色选择器面板中的"超时(秒)"输入框实时调整

## 交互说明

| 操作区域 | 操作 | 效果 |
|----------|------|------|
| 🟢🟡⚪ **指示灯** | 左键单击 | 切换 Working ⟷ Stopped |
| 🟢🟡⚪ **指示灯** | 双击 | 打开/关闭颜色选择器面板 |
| 🟢🟡⚪ **指示灯** | 拖拽 | 移动窗口位置（自动记忆） |
| 📄 **文字区域** | 左键单击 | 打开原生文件夹选择器，设置监控目录 |
| 🧩 **系统托盘图标** | 左键单击 | 显示窗口并聚焦 |
| 🧩 **系统托盘图标** | 右键菜单 | 切换状态 / 退出程序 |
| ❌ **窗口关闭** | Alt+F4 | 隐藏到托盘（不退出） |

### 颜色选择器面板

双击灯打开后，可自定义：

- **工作中** — Working 颜色（默认 `#4CAF50`）
- **警告** — Warning 颜色（默认 `#FFC107`）
- **终止** — Stopped 颜色（默认 `#9E9E9E`）
- **超时(秒)** — 从 Working→Warning 的空闲超时秒数（默认 `60`）
- **开机自启** — 勾选后开机自动启动程序

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2 (Rust + WebView2) |
| 后端 | Rust |
| 前端 | 原生 HTML + CSS + JavaScript（无框架） |
| 文件监控 | `notify` crate（Windows `ReadDirectoryChangesW` 原生事件） |
| 系统托盘 | Tauri `tray-icon` 内置支持 |
| 开机自启 | `tauri-plugin-autostart`（Windows 注册表） |
| 配置格式 | TOML → `%APPDATA%\worm-ai-light\config.toml` |

## 项目结构

```
worm-ai-light/
├── src/                          # Web 前端
│   ├── index.html                # 主界面（指示灯 + 状态文字）
│   ├── app.js                    # 前端逻辑（状态渲染、颜色配置、交互事件）
│   └── styles.css                # 窗口样式、灯光动画
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs               # 程序入口
│   │   ├── lib.rs                # Tauri 命令、文件监控线程、状态推送线程、托盘菜单
│   │   ├── config.rs             # 配置读写（TOML 序列化）
│   │   └── state.rs              # 状态枚举 + 共享状态 (Arc<RwLock<>>)
│   ├── Cargo.toml
│   ├── tauri.conf.json           # 窗口参数、应用标识、图标
│   ├── capabilities/default.json # 权限声明
│   └── icons/                    # 应用图标 (32x32, 128x128, ico)
└── README.md
```

## 环境要求

- **Rust 工具链** — 建议配置国内镜像源加速
- **Visual Studio Build Tools** — 含 Windows SDK（Tauri 依赖 WebView2）
- **Node.js** — Tauri 构建过程中需要

## 构建与运行

```bash
# 克隆项目后进入目录
cd worm-ai-light

# Debug 编译并运行
cd src-tauri
cargo run

# Release 编译（体积小、性能好）
cargo build --release
# 输出: src-tauri\target\release\worm-ai-light.exe

# 制作安装包（需要额外安装 tauri-cli）
cargo install tauri-cli --version "^2"
cd src-tauri
cargo tauri build
# 输出: src-tauri\target\release\bundle\msi\*.msi
```

### 直接启动（隐藏控制台窗口）

```powershell
Start-Process -FilePath "target\release\worm-ai-light.exe" -WindowStyle Hidden
```

## 配置文件

路径：`%APPDATA%\worm-ai-light\config.toml`

```toml
window_x = 1200.0
window_y = 800.0
idle_timeout_secs = 60

[colors]
idle = "#9E9E9E"     # Stopped 颜色
working = "#4CAF50"  # Working 颜色
stopped = "#FFC107"  # Warning 颜色
```

> `monitor_directory` 字段也会保存在此文件中，由程序自动管理。

## 窗口配置（tauri.conf.json）

```json
{
  "windows": [{
    "width": 220,
    "height": 34,
    "resizable": false,
    "maximizable": false,
    "minimizable": false,
    "decorations": false,
    "transparent": true,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "center": true
  }]
}
```

| 参数 | 值 | 说明 |
|------|----|------|
| `decorations` | false | 无边框/标题栏 |
| `resizable` | false | 禁止调整大小 |
| `maximizable` | false | 禁止最大化 |
| `minimizable` | false | 禁止最小化 |
| `transparent` | true | 透明背景 |
| `alwaysOnTop` | true | 始终置顶 |
| `skipTaskbar` | true | 不显示在任务栏 |

## 开发笔记

### 文件监控架构

监控线程采用 **事件驱动 + 超时兜底** 的模式：

1. 使用 `notify` crate 创建 `RecommendedWatcher`，注册对目录的递归监控
2. 每次收到文件事件 → 更新 `last_change_time = Instant::now()` + 状态设为 `Working`（**无论当前状态，灰色 Stopped 也被强制切换**）
3. 使用 `rx.recv_timeout(idle_timeout)` 阻塞等待 — 有事件立即处理，无事件则超时
4. 超时后检查 `last_change_time.elapsed() >= idle_timeout` → 满足则转为 `Warning`（**仅从 Working 触发，不影响灰色 Stopped**）

### 目录选择器回调处理

`tauri-plugin-dialog` v2 的 `pick_folder` 是回调式 API，使用 `mpsc::channel` 同步等待结果：

```rust
let (tx, rx) = mpsc::channel::<Option<String>>();
app_handle.dialog().file().pick_folder(move |file| {
    let _ = tx.send(file.map(|p| p.to_string()));
});
let picked = rx.recv().map_err(|_| "取消选择")?;
```

### 窗口位置记忆

窗口关闭时不退出，而是隐藏到托盘。`Moved` 事件触发时将坐标写入配置，启动时校验坐标是否在当前任意显示器范围内，防止多显示器热插拔导致窗口"飞出"屏幕。

### on_window_event 注意

Tauri v2 的 `on_window_event` 是 setter 而不是注册器，多次调用会覆盖。所有事件处理需放在同一个 `match` 中。

### 左键菜单冲突

Windows 上 Tauri 托盘设置菜单后默认左键也会弹出菜单。需使用 `.show_menu_on_left_click(false)` 分离左键点击事件和右键菜单事件。


## AI 打包提示词

将以下提示词复制给 AI 编码助手（如 Codex CLI、Cursor 等），即可自动完成项目的编译和打包：

> 请帮我编译并打包 worm-ai-light 项目。
>
> Windows 桌面悬浮灯，通过监控指定目录的文件变更实时反馈 AI 编码助手的工作状态。
>
> 技术栈：Tauri v2 (Rust + WebView2)，前端为原生 HTML/CSS/JS。
>
> 打包步骤：
> 1. 进入项目目录 cd worm-ai-light
> 2. 进入 src-tauri 目录 cd src-tauri
> 3. 运行 cargo tauri build（需要已安装 tauri-cli）
> 4. 安装包输出在 src-tauri\target\release\bundle\nsis\worm-ai-light_0.1.0_x64-setup.exe
>
> 项目结构：
> - src/ — Web 前端（index.html, app.js, styles.css）
> - src-tauri/src/ — Rust 后端（lib.rs, config.rs, state.rs）
> - src-tauri/tauri.conf.json — Tauri 配置
> - src-tauri/Cargo.toml — Rust 依赖
>
> 如果先执行 cargo build --release 快速编译，再执行 cargo tauri build 可跳过编译步骤直接打包。

