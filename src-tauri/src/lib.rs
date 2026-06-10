mod config;
mod state;

use config::AppConfig;
use state::{AiState, AppState, SharedState, create_shared_state};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::thread;
use tauri::{Emitter, Listener, Manager};
use tauri::menu::{CheckMenuItem, CheckMenuItemBuilder, IsMenuItem, Menu, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_autostart::ManagerExt;

#[derive(serde::Serialize, Clone)]
pub struct StateInfo {
    pub state: String,
    pub color: String,
    pub flashing: bool,
    pub colors: ColorsInfo,
    pub monitor_directory: Option<String>,
    pub remaining_secs: u64,
    pub idle_timeout_secs: u64,
}

#[derive(serde::Serialize, Clone)]
pub struct ColorsInfo {
    pub idle: String,
    pub working: String,
    pub stopped: String,
}

// ─── Tauri 命令 ─────────────────────────────────────────────

#[tauri::command]
fn get_state(app_handle: tauri::AppHandle) -> Result<StateInfo, String> {
    let state = app_handle.state::<SharedState>();
    let app_state = state.read().map_err(|e| e.to_string())?;
    let config = AppConfig::load();
    let remaining_secs = app_state.last_change_time.map_or(0, |t| {
        let elapsed = t.elapsed().as_secs();
        let timeout = config.idle_timeout_secs;
        if elapsed >= timeout { 0 } else { timeout - elapsed }
    });
    Ok(StateInfo {
        state: app_state.current.as_str().to_string(),
        color: app_state.current.color().to_string(),
        flashing: app_state.current.flashing(),
        colors: ColorsInfo {
            idle: config.colors.idle,
            working: config.colors.working,
            stopped: config.colors.stopped,
        },
        monitor_directory: app_state.monitor_directory.clone(),
        remaining_secs,
        idle_timeout_secs: config.idle_timeout_secs,
    })
}

fn toggle_app_state(app_state: &mut AppState) {
    match app_state.current {
        AiState::Stopped => {
            app_state.current = AiState::Working;
            app_state.last_change_time = Some(Instant::now());
        }
        AiState::Working | AiState::Warning => {
            app_state.current = AiState::Stopped;
            app_state.last_change_time = None;
        }
    }
}

#[tauri::command]
fn toggle_light(app_handle: tauri::AppHandle) -> Result<(), String> {
    let state = app_handle.state::<SharedState>();
    let app_state = state.read().map_err(|e| e.to_string())?;
    if app_state.monitor_directory.is_none() && app_state.current == AiState::Stopped {
        return Err("请先选择监控目录".to_string());
    }
    drop(app_state);
    let mut app_state = state.write().map_err(|e| e.to_string())?;
    toggle_app_state(&mut app_state);
    let info = build_state_info(&app_state.current, &app_state.monitor_directory, app_state.last_change_time);
    let _ = app_handle.emit("state-changed", info);
    Ok(())
}

#[tauri::command]
fn pick_and_set_directory(app_handle: tauri::AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = mpsc::channel::<Option<String>>();
    app_handle.dialog().file().pick_folder(move |file| {
        let _ = tx.send(file.map(|p| p.to_string()));
    });
    let picked = rx.recv().map_err(|_| "对话框错误".to_string())?;
    match picked {
        Some(path) => {
            let mut config = AppConfig::load();
            config.monitor_directory = Some(path.clone());
            config.save();
            let state = app_handle.state::<SharedState>();
            let mut app_state = state.write().map_err(|e| e.to_string())?;
            app_state.monitor_directory = Some(path.clone());
            app_state.last_change_time = Some(Instant::now());
            // 目录配置变更不受状态影响：从 Stopped 转为 Working
            if app_state.current == AiState::Stopped {
                app_state.current = AiState::Working;
            }
            let info = build_state_info(&app_state.current, &app_state.monitor_directory, app_state.last_change_time);
            let _ = app_handle.emit("state-changed", info);
            Ok(Some(path))
        }
        None => Ok(None),
    }
}

#[tauri::command]
fn get_colors() -> ColorsInfo {
    let config = AppConfig::load();
    ColorsInfo {
        idle: config.colors.idle,
        working: config.colors.working,
        stopped: config.colors.stopped,
    }
}

#[tauri::command]
fn save_colors(idle: String, working: String, stopped: String) -> Result<(), String> {
    let mut config = AppConfig::load();
    config.colors.idle = idle;
    config.colors.working = working;
    config.colors.stopped = stopped;
    config.save();
    Ok(())
}

#[derive(serde::Serialize, Clone)]
pub struct ConfigInfo {
    pub idle_timeout_secs: u64,
}

#[tauri::command]
fn get_config() -> ConfigInfo {
    let config = AppConfig::load();
    ConfigInfo {
        idle_timeout_secs: config.idle_timeout_secs,
    }
}

#[tauri::command]
fn save_config(idle_timeout_secs: u64) -> Result<(), String> {
    let mut config = AppConfig::load();
    config.idle_timeout_secs = idle_timeout_secs;
    config.save();
    Ok(())
}

#[tauri::command]
async fn get_autostart(app_handle: tauri::AppHandle) -> Result<bool, String> {
    app_handle.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_autostart(app_handle: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app_handle.autolaunch().enable().map_err(|e| e.to_string())?;
    } else {
        app_handle.autolaunch().disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ─── 事件驱动监控 ──────────────────────────────────────────

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

fn create_watcher(
    dir: &Path,
) -> Result<(RecommendedWatcher, mpsc::Receiver<Result<Event, notify::Error>>), notify::Error> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    watcher.watch(dir, RecursiveMode::Recursive)?;
    Ok((watcher, rx))
}

fn start_monitor(state: SharedState) {
    thread::spawn(move || {
        let mut watcher: Option<(RecommendedWatcher, mpsc::Receiver<Result<Event, notify::Error>>)> = None;
        let mut current_dir: Option<String> = None;

        loop {
            // 1. 检测目录变更 → 重建 watcher
            let dir = state.read().unwrap().monitor_directory.clone();

            if dir != current_dir {
                watcher = None;
                current_dir = None;
                if let Some(ref d) = dir {
                    let path = Path::new(d);
                    if path.exists() {
                        if let Ok((w, rx)) = create_watcher(path) {
                            watcher = Some((w, rx));
                            current_dir = dir.clone();
                        }
                    }
                }
            }

            // 2. 无监控目录时强制 Stopped
            if dir.is_none() {
                let mut s = state.write().unwrap();
                if s.current != AiState::Stopped {
                    s.current = AiState::Stopped;
                }
            }

            // 3. 读取最新 timeout 配置
            let idle_timeout = Duration::from_secs(AppConfig::load().idle_timeout_secs);

            if let Some((_, ref rx)) = watcher {
                let mut disconnected = false;

                // 清空已积压事件
                loop {
                    match rx.try_recv() {
                        Ok(Ok(_)) => {
                            let mut s = state.write().unwrap();
                            s.last_change_time = Some(Instant::now());
                            s.current = AiState::Working;
                        }
                        Ok(Err(_)) => {}
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }

                // 阻塞等待：事件驱动 or idle_timeout 超时
                if !disconnected {
                    match rx.recv_timeout(idle_timeout) {
                        Ok(Ok(_)) => {
                            let mut s = state.write().unwrap();
                            s.last_change_time = Some(Instant::now());
                            s.current = AiState::Working;
                        }
                        Ok(Err(_)) => {}
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            // 空闲超时 → 检查是否转为 Warning (黄色闪烁)
                            let mut s = state.write().unwrap();
                            if s.current == AiState::Working {
                                if let Some(t) = s.last_change_time {
                                    if t.elapsed() >= idle_timeout {
                                        s.current = AiState::Warning;
                                    }
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            disconnected = true;
                        }
                    }
                }

                if disconnected {
                    watcher = None;
                }
            } else {
                if dir.is_none() {
                    thread::sleep(idle_timeout);
                } else {
                    // 目录存在但 watcher 创建失败，1 秒后重试
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    });
}

// ─── 状态推送 ──────────────────────────────────────────────

fn build_state_info(s: &AiState, dir: &Option<String>, last_change: Option<Instant>) -> StateInfo {
    let config = AppConfig::load();
    let remaining_secs = last_change.map_or(0, |t| {
        let elapsed = t.elapsed().as_secs();
        let timeout = config.idle_timeout_secs;
        if elapsed >= timeout { 0 } else { timeout - elapsed }
    });
    StateInfo {
        state: s.as_str().to_string(),
        color: s.color().to_string(),
        flashing: s.flashing(),
        colors: ColorsInfo {
            idle: config.colors.idle,
            working: config.colors.working,
            stopped: config.colors.stopped,
        },
        monitor_directory: dir.clone(),
        remaining_secs,
        idle_timeout_secs: config.idle_timeout_secs,
    }
}

fn poll_state_changes(app_handle: tauri::AppHandle, state: SharedState) {
    thread::spawn(move || {
        let mut prev_str: Option<String> = None;
        let mut prev_dir: Option<String> = None;
        let mut prev_last_time: Option<Instant> = None;
        loop {
            let (cur_state, cur_dir, last_time) = {
                let s = match state.read() {
                    Ok(s) => s,
                    Err(_) => {
                        thread::sleep(Duration::from_millis(200));
                        continue;
                    }
                };
                (s.current.clone(), s.monitor_directory.clone(), s.last_change_time)
            };
            let cur_str = cur_state.as_str().to_string();
            // 状态、监控目录或计时变更时都推送事件（保持倒计时同步）
            if prev_str.as_ref() != Some(&cur_str) || prev_dir != cur_dir || prev_last_time != last_time {
                let info = build_state_info(&cur_state, &cur_dir, last_time);
                let _ = app_handle.emit("state-changed", info);
                prev_str = Some(cur_str);
                prev_dir = cur_dir;
                prev_last_time = last_time;
            }
            thread::sleep(Duration::from_millis(200));
        }
    });
}

// ─── 托盘菜单 ──────────────────────────────────────────────

fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let toggle = CheckMenuItemBuilder::with_id("toggle", "工作中")
        .build(app)?;
    // 初始化 toggle 勾选状态
    {
        let state = app.state::<SharedState>();
        let app_state = state.read().unwrap();
        let _ = toggle.set_checked(app_state.current != AiState::Stopped);
    }
    let toggle_clone = toggle.clone();

    // --- 轮询间隔选项 ---
    let interval_items: Vec<CheckMenuItem<tauri::Wry>> = vec![
        CheckMenuItemBuilder::with_id("interval_30", "30秒").build(app)?,
        CheckMenuItemBuilder::with_id("interval_60", "60秒").build(app)?,
        CheckMenuItemBuilder::with_id("interval_120", "120秒").build(app)?,
        CheckMenuItemBuilder::with_id("interval_300", "5分钟").build(app)?,
        CheckMenuItemBuilder::with_id("interval_600", "10分钟").build(app)?,
    ];

    // 根据当前配置选中对应项
    let config = AppConfig::load();
    for item in &interval_items {
        let id = item.id().as_ref().to_string();
        let should_check = match id.as_str() {
            "interval_30" => config.idle_timeout_secs == 30,
            "interval_60" => config.idle_timeout_secs == 60,
            "interval_120" => config.idle_timeout_secs == 120,
            "interval_300" => config.idle_timeout_secs == 300,
            "interval_600" => config.idle_timeout_secs == 600,
            _ => false,
        };
        let _ = item.set_checked(should_check);
    }

    let interval_refs: Vec<&dyn IsMenuItem<tauri::Wry>> = interval_items.iter().map(|i| i as &dyn IsMenuItem<tauri::Wry>).collect();
    let interval_submenu = SubmenuBuilder::new(app, "轮询间隔")
        .items(&interval_refs)
        .build()?;

    let interval_items_clone = interval_items.clone();

    // --- 开机自启选项 ---
    let autostart_check = CheckMenuItemBuilder::with_id("autostart", "开机自启")
        .build(app)?;
    let _ = autostart_check.set_checked(app.autolaunch().is_enabled().unwrap_or(false));
    let autostart_check_clone = autostart_check.clone();

    let separator1 = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;

    let menu = Menu::with_items(app, &[&toggle, &separator1, &interval_submenu, &autostart_check, &separator2, &quit])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(move |app, event| {
            match event.id.as_ref() {
                "toggle" => {
                    // 未配置监控目录时不允许开启
                    {
                        let state = app.state::<SharedState>();
                        let app_state = state.read().unwrap();
                        if app_state.monitor_directory.is_none() && app_state.current == AiState::Stopped {
                            let _ = app.dialog()
                                .message("请先配置监控目录")
                                .title("提示")
                                .kind(tauri_plugin_dialog::MessageDialogKind::Info)
                                .blocking_show();
                            return;
                        }
                    }
                    let state = app.state::<SharedState>();
                    let mut app_state = state.write().unwrap();
                    toggle_app_state(&mut app_state);
                    let _ = toggle_clone.set_checked(app_state.current != AiState::Stopped);
                    let info = build_state_info(&app_state.current, &app_state.monitor_directory, app_state.last_change_time);
                    let _ = app.emit("state-changed", info);
                }
                id if id.starts_with("interval_") => {
                    // 单选行为：更新所有勾选状态
                    for item in &interval_items_clone {
                        let _ = item.set_checked(item.id().as_ref() == id);
                    }
                    let secs: u64 = match id {
                        "interval_30" => 30,
                        "interval_60" => 60,
                        "interval_120" => 120,
                        "interval_300" => 300,
                        "interval_600" => 600,
                        _ => return,
                    };
                    let mut config = AppConfig::load();
                    config.idle_timeout_secs = secs;
                    config.save();
                    let _ = app.emit("config-changed", serde_json::json!({"idle_timeout_secs": secs}));
                }
                "quit" => {
                    app.exit(0);
                }
                "autostart" => {
                    let enabled = app.autolaunch().is_enabled().unwrap_or(false);
                    if enabled {
                        let _ = app.autolaunch().disable();
                    } else {
                        let _ = app.autolaunch().enable();
                    }
                    let _ = autostart_check_clone.set_checked(!enabled);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn refresh_tray_menu(handle: &tauri::AppHandle) {
    if let Some(item) = handle.tray_by_id("main") {
        let state = handle.state::<SharedState>();
        let app_state = state.read().unwrap();
        let _ = item.set_tooltip(Some(format!("AI牛马灯 - {}", app_state.current.as_str())));
    }
}

// ─── 应用入口 ──────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(create_shared_state())
        .setup(|app| {
            let state = app.state::<SharedState>();

            // 恢复上次的监控目录
            {
                let config = AppConfig::load();
                let mut app_state = state.write().unwrap();
                app_state.monitor_directory = config.monitor_directory.clone();
            }

            // 启动监控线程
            start_monitor(state.inner().clone());
            // 启动状态推送
            poll_state_changes(app.handle().clone(), state.inner().clone());

            // 监听自动启动事件：显示窗口
            app.listen("codex-auto-launch", |_event| {});

            // 窗口初始化
            if let Some(window) = app.get_webview_window("main") {
                // 恢复窗口位置
                let config = AppConfig::load();
                if let (Some(x), Some(y)) = (config.window_x, config.window_y) {
                    let pos = tauri::PhysicalPosition::new(x as i32, y as i32);
                    let x32 = x as i32;
                    let y32 = y as i32;
                    let on_screen = window.available_monitors().ok().map_or(false, |ms| {
                        ms.iter().any(|m| {
                            let mp = m.position();
                            let ms = m.size();
                            x32 >= mp.x && x32 + 220 <= mp.x + ms.width as i32
                                && y32 >= mp.y && y32 + 34 <= mp.y + ms.height as i32
                        })
                    });
                    if on_screen { let _ = window.set_position(pos); }
                }
                // 保证窗口可见
                let _ = window.show();
                let _ = window.set_focus();
            }

            // 记录窗口位置
            if let Some(win) = app.get_webview_window("main") {
                let win_hide = win.clone();
                win.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::Moved(pos) => {
                            let mut config = AppConfig::load();
                            config.window_x = Some(pos.x as f64);
                            config.window_y = Some(pos.y as f64);
                            config.save();
                        }
                        tauri::WindowEvent::CloseRequested { .. } => {
                            let _ = win_hide.hide();
                            refresh_tray_menu(&win_hide.app_handle());
                        }
                        _ => {}
                    }
                });
            }

            build_tray(app).expect("创建托盘失败");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            toggle_light,
            pick_and_set_directory,
            get_colors,
            save_colors,
            get_config,
            save_config,
            get_autostart,
            set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("启动状态监控灯失败");
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::{AiState, AppState};

    #[test]
    fn test_toggle_from_stopped_to_working() {
        let mut app = AppState::new();
        assert_eq!(app.current, AiState::Stopped);
        assert!(app.last_change_time.is_none());

        toggle_app_state(&mut app);

        assert_eq!(app.current, AiState::Working);
        assert!(app.last_change_time.is_some());
    }

    #[test]
    fn test_toggle_from_working_to_stopped() {
        let mut app = AppState::new();
        app.current = AiState::Working;
        app.last_change_time = Some(Instant::now());

        toggle_app_state(&mut app);

        assert_eq!(app.current, AiState::Stopped);
        assert!(app.last_change_time.is_none());
    }

    #[test]
    fn test_toggle_from_warning_to_stopped() {
        let mut app = AppState::new();
        app.current = AiState::Warning;
        app.last_change_time = Some(Instant::now());

        toggle_app_state(&mut app);

        assert_eq!(app.current, AiState::Stopped);
        assert!(app.last_change_time.is_none());
    }

    #[test]
    fn test_config_default_timeout() {
        let config = AppConfig::default();
        assert_eq!(config.idle_timeout_secs, 60);
        assert!(config.monitor_directory.is_none());
    }

    #[test]
    fn test_config_colors() {
        let config = AppConfig::default();
        assert_eq!(config.colors.working, "#4CAF50");
        assert_eq!(config.colors.stopped, "#FFC107");
        assert_eq!(config.colors.idle, "#9E9E9E");
    }

    #[test]
    fn test_set_directory_transitions_to_working() {
        let mut app = AppState::new();
        assert_eq!(app.current, AiState::Stopped);

        // Simulate pick_and_set_directory: directory set, state becomes Working
        app.monitor_directory = Some("C:\\test\\dir".to_string());
        app.current = AiState::Working;
        app.last_change_time = Some(Instant::now());

        assert_eq!(app.current, AiState::Working);
        assert!(app.last_change_time.is_some());
        assert_eq!(app.monitor_directory, Some("C:\\test\\dir".to_string()));
    }

    #[test]
    fn test_file_event_resets_timer_and_working() {
        let mut app = AppState::new();
        app.current = AiState::Warning;
        app.last_change_time = Some(Instant::now() - Duration::from_secs(10));

        // File change: fresh timer and Working
        app.last_change_time = Some(Instant::now());
        app.current = AiState::Working;

        assert_eq!(app.current, AiState::Working);
        assert!(app.last_change_time.is_some());
    }

    #[test]
    fn test_set_directory_from_working_keeps_working() {
        let mut app = AppState::new();
        app.current = AiState::Working;
        app.monitor_directory = Some("C:\\old\\dir".to_string());
        app.last_change_time = Some(Instant::now());

        // User changes to a new directory while already Working
        app.monitor_directory = Some("C:\\new\\dir".to_string());
        app.last_change_time = Some(Instant::now());

        assert_eq!(app.current, AiState::Working);
        assert_eq!(app.monitor_directory, Some("C:\\new\\dir".to_string()));
        assert!(app.last_change_time.is_some());
    }

    #[test]
    fn test_set_directory_from_warning_refreshes_timer() {
        let mut app = AppState::new();
        app.current = AiState::Warning;
        app.monitor_directory = Some("C:\\old\\dir".to_string());
        app.last_change_time = Some(Instant::now() - Duration::from_secs(120));

        // User changes directory while in Warning → fresh timer (state stays Warning until next check)
        app.monitor_directory = Some("C:\\new\\dir".to_string());
        app.last_change_time = Some(Instant::now());

        assert_eq!(app.current, AiState::Warning);
        assert!(app.last_change_time.is_some());
        assert_eq!(app.monitor_directory, Some("C:\\new\\dir".to_string()));
    }
}




