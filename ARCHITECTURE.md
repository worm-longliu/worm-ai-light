# AI牛马灯 — 项目原理

## 概述

AI牛马灯是一款跨平台桌面状态指示灯，通过监控指定目录的文件变更活动，实时反映 AI 任务的工作状态。状态通过**系统托盘图标**和**桌面悬浮窗**两种方式呈现。

---

## 工作原理

### 核心流程

```
┌─────────────┐    文件变动     ┌──────────────┐    状态事件     ┌──────────┐
│  文件系统    │ ──────────→  │  监控线程     │ ────────────→  │  前端 UI  │
│  (监控目录)  │               │  (start_monitor) │              │  (窗口)   │
└─────────────┘               └──────────────┘               └──────────┘
                                      │                              │
                                      │ 读配置                        │ 托盘菜单
                                      ▼                              ▼
                               ┌──────────────┐               ┌──────────┐
                               │  AppConfig    │               │  系统托盘  │
                               │  (config.toml) │               │  (TrayIcon) │
                               └──────────────┘               └──────────┘
```

### 状态机

```
                   用户切换 / 设置目录
     ┌────────────────────────────────────────────────────────────┐
     │                                                            ▼
  ┌─────────┐    用户切换 / 设置目录     ┌──────────┐    超时无文件变动    ┌─────────┐
  │ Stopped │ ←─────────────────────  │ Working  │ ────────────────→  │ Warning │
  │  (灰色)  │                         │  (绿色)   │                    │ (黄色闪烁)│
  └─────────┘                         └──────────┘                    └─────────┘
       ▲                                  │  ↑                            │
       └──────────────────────────────────┘  └────────────────────────────┘
           用户切换 / 设置目录                 文件变动（重置计时器）
```

---

## 技术架构

### 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | **Tauri v2**（Rust + WebView） |
| 后端语言 | **Rust** |
| 前端语言 | 原生 **JavaScript** + **CSS** |
| 文件监控 | **notify** crate（事件驱动文件系统监听） |
| 配置存储 | **TOML** 文件（%APPDATA%/worm-ai-light/config.toml） |
| 托盘图标 | Tauri tray-icon API |

### 后端模块

#### lib.rs
主入口，管理 Tauri 应用生命周期、托盘菜单、状态命令。

**关键函数：**
- `build_tray` — 构建系统托盘图标和右键菜单（切换状态、轮询间隔选择、退出）
- `start_monitor` — 启动文件监控线程，使用 `notify` 库监听目录变更
- `poll_state_changes` — 定时轮询状态变化并推送事件到前端（每 200ms）
- `build_state_info` — 构造发送给前端的完整状态信息（含倒计时）

**Tauri 命令（前端可调用）：**
- `get_state` — 获取当前状态
- `toggle_light` — 切换工作/停止状态
- `pick_and_set_directory` — 选择监控目录
- `get_config` / `save_config` — 读写超时配置
- `get_colors` / `save_colors` — 读写颜色配置
- `get_autostart` / `set_autostart` — 开机自启

#### config.rs
基于 `serde` 的 TOML 配置管理。
- 配置文件路径：`%APPDATA%/worm-ai-light/config.toml`
- 字段：`window_x`、`window_y`、`colors`、`monitor_directory`、`idle_timeout_secs`
- 使用 `#[serde(default)]` 保证旧配置向后兼容

#### state.rs
运行时状态管理。
- `AiState` 枚举：`Working` / `Stopped` / `Warning`
- `AppState` 结构体：当前状态 + 监控目录 + 最后变更时间
- `SharedState`：`Arc<RwLock<AppState>>` 实现线程安全共享

### 前端模块

#### app.js
桌面悬浮窗的交互逻辑。
- 通过 `window.__TAURI__.core.invoke` 调用 Rust 命令
- 通过 `window.__TAURI__.event.listen` 监听后端事件
- 状态文本渲染 + 倒计时本地计时器（每秒递减）

#### index.html
极简 UI 布局：状态指示灯 + 状态文字区域。

#### styles.css
紧凑的悬浮窗样式，支持拖拽、颜色过渡、闪烁动画。

---

## 关键特性

### 文件监控
使用 `notify` crate 的 `RecommendedWatcher`，基于操作系统原生文件事件 API（Windows 上为 ReadDirectoryChangesW），比轮询更高效。

### 空闲超时判断
- 超时时间在配置中设定（默认 60 秒，可通过托盘菜单选择）
- 从最后一个文件变更事件开始计时，超时后状态从 `Working` → `Warning`
- 监控线程同时作为事件接收方和超时触发器，使用 `recv_timeout` 实现

### 托盘菜单
右键托盘图标可操作：
- **工作中** — 勾选项，切换监控状态
- **轮询间隔** — 子菜单，选择超时时间（30s/60s/120s/5min/10min）
- **退出** — 退出程序

### 悬浮窗
左侧为状态指示灯（颜色 + 闪烁），右侧为状态文字 + 路径 + 倒计时。
- 点击灯：切换工作/停止
- 双击灯：弹出配置面板（颜色 / 超时 / 开机自启）
- 点击文字：选择监控目录
- 拖拽窗口到新位置后自动保存位置

---

## 数据流

```
用户操作                    后端                         前端
─────────                ──────                       ──────

点击文字区域  ──invoke──→  pick_and_set_directory()
                           ├ 更新 config.toml
                           ├ 更新 SharedState
                           └ emit("state-changed") ──→  applyState()

托盘选间隔    ──→          on_menu_event()
                           ├ 保存 config.toml
                           └ emit("config-changed") ──→  更新超时输入框

文件变动      ──→          start_monitor()
                           ├ 重置 last_change_time
                           ├ state: Working
                           └ (poll_state_changes 捕获变化)
                           └ emit("state-changed") ──→  applyState()

超时触发      ──→          start_monitor()
                           ├ state: Warning
                           └ (poll_state_changes 捕获变化)
                           └ emit("state-changed") ──→  黄灯闪烁 + 警告文字
```

