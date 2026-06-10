# AI牛马灯 变更记录

## [Unreleased]

### 新增
- **托盘菜单 — 轮询间隔配置**：在系统托盘中增加「轮询间隔」子菜单，提供 30 秒 / 60 秒 / 120 秒 / 5 分钟 / 10 分钟五个预设选项，选择后立即生效
- **实时倒计时显示**：在窗口状态栏「监控中」状态下，目录路径右侧显示剩余秒数倒计时 [Xs]，每 1 秒刷新
- **前后端配置同步**：托盘修改间隔后，通过 config-changed 事件自动同步更新窗口中的超时输入框
- **文件事件防抖**：引入 notify-debouncer-mini（500ms 窗口），自动合并去重文件事件洪峰（如 IDE 批量保存、git checkout），避免计时器反复重置

### 修复
- **修复中文字符编码损坏**：修复因 PowerShell 管道处理导致的 app.js 中文字符乱码问题
- **修复 applyState 函数截断**：恢复被截断的 if (flashing) 闪烁逻辑代码块
- **修复 toggle 勾选状态同步**：托盘「工作中」CheckMenuItem 现在正确反映实际开关状态

### 优化
- **简化监控线程**：移除手动 drain 循环，使用 DebouncedWatcher 替代 RecommendedWatcher + 手动事件累积处理
- **轮询间隔即时生效**：监控线程每次循环重新读取 AppConfig::load().idle_timeout_secs，修改无需重启
- **poll_state_changes 增强**：增加 last_change_time 追踪，文件事件触发时立即推送新倒计时
- **路径自动适配**：倒计时文本自动适配路径缩短长度，避免 UI 溢出
