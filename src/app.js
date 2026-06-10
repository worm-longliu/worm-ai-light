const indicator = document.getElementById("indicator");
const light = indicator.querySelector(".light");
const info = document.getElementById("info");
const statusText = document.getElementById("status-text");
const colorPicker = document.getElementById("color-picker");
const colorWorking = document.getElementById("color-working");
const colorStopped = document.getElementById("color-stopped");
const colorIdle = document.getElementById("color-idle");

const MAX_PATH_CHARS = 26;

// 缩短路径：每个段保留至少一个字符，过长时用 . 代替
function shortenPath(path, maxLen) {
  if (!path || path.length <= maxLen) return path;

  const parts = path.split(/[\\/]/);

  // 1. 从尾部往前拼，尽量保留完整段
  let result = "";
  let i;
  for (i = parts.length - 1; i >= 0; i--) {
    const segment = i === parts.length - 1 ? parts[i] : parts[i] + "\\" + result;
    if (segment.length > maxLen) break;
    result = segment;
  }

  // 2. 剩余段各截取首字符 + .
  for (let j = i; j >= 0; j--) {
    const isDrive = parts[j].endsWith(":");
    const short = isDrive ? parts[j] : parts[j][0] + ".";
    const candidate = short + "\\" + result;
    if (candidate.length <= maxLen) {
      result = candidate;
    } else {
      break;
    }
  }

  return result;
}

async function init() {
  try {
    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;

    const initInfo = await invoke("get_state");
    applyState(initInfo);
    startCountdown();

    await listen("state-changed", (event) => { applyState(event.payload); startCountdown(); });
    await listen("config-changed", async (event) => {
      const { idle_timeout_secs } = event.payload;
      const input = document.getElementById("timeout-input");
      if (input) input.value = idle_timeout_secs;
    });
  } catch (e) {
    console.log("Tauri 桥接未就绒:", e);
    return;
  }

  // ── 灯: mousedown/mouseup 判断 点击 vs 拖搋 ──
  let indicatorMoved = false;
  indicator.addEventListener("mousedown", () => { indicatorMoved = false; });
  indicator.addEventListener("mousemove", () => { indicatorMoved = true; });
  indicator.addEventListener("mouseup", async () => {
    if (!indicatorMoved) {
      const { invoke } = window.__TAURI__.core;
      try {
        await invoke("toggle_light");
      } catch (e) {
        showToast("请先配置监控目录");
      }
    }
  });

  indicator.addEventListener("dblclick", () => {
    colorPicker.classList.toggle("hidden");
  });

  // ── 文字区域: mousedown/mouseup 判断 点击 vs 拖搋 ──
  let infoMoved = false;
  info.addEventListener("mousedown", () => { infoMoved = false; });
  info.addEventListener("mousemove", () => { infoMoved = true; });
  info.addEventListener("mouseup", async () => {
    if (!infoMoved) {
      const { invoke } = window.__TAURI__.core;
      const result = await invoke("pick_and_set_directory");
      if (result) console.log("监控目录已设置:", result);
    }
  });

  // ── 超时配置 ──
  const timeoutInput = document.getElementById("timeout-input");
  const defaultTimeout = await loadTimeout();

  timeoutInput.addEventListener("change", async () => {
    const val = parseInt(timeoutInput.value);
    if (val > 0) {
      const { invoke } = window.__TAURI__.core;
      await invoke("save_config", { idleTimeoutSecs: val });
    }
  });

  // ── 颜色配置 ──
  colorWorking.addEventListener("input", saveColors);
  colorStopped.addEventListener("input", saveColors);
  colorIdle.addEventListener("input", saveColors);

  // ── 开机自启配置 ──
  const autostartToggle = document.getElementById("autostart-toggle");
  await loadAutostart();

  autostartToggle.addEventListener("change", async () => {
    const { invoke } = window.__TAURI__.core;
    await invoke("set_autostart", { enabled: autostartToggle.checked });
  });
}

function applyState(info) {
  const { state, color, flashing, colors, monitor_directory, remaining_secs } = info;

  indicator.className = "indicator " + state;
  indicator.dataset.state = state;

  light.style.backgroundColor = color;
  light.style.boxShadow = "0 0 12px " + color + "66";

  if (flashing) {
    light.style.animation = "flash 0.5s ease-in-out infinite";
  } else {
    light.style.animation = "none";
    light.style.opacity = "1";
  }

  // 存储用于倒计时
  window._state = state;
  window._remainingSecs = remaining_secs;
  window._monitorDir = monitor_directory || "";

  updateStatusText();
}

// ─── 倒计时显示 ──────────────────────────────────────────

function updateStatusText() {
  const state = window._state;
  const dir = window._monitorDir;
  const remaining = window._remainingSecs;

  if (state === "stopped" && !dir) {
    statusText.textContent = "点击配置监控目录";
  } else if (state === "stopped" && dir) {
    statusText.textContent = "已终止: " + shortenPath(dir, MAX_PATH_CHARS - 9);
  } else if (state === "working") {
    const countdownStr = remaining > 0 ? " [" + remaining + "s]" : "";
    const maxPath = MAX_PATH_CHARS - 9 - countdownStr.length;
    statusText.textContent = "监控中: " + shortenPath(dir, Math.max(maxPath, 6)) + countdownStr;
  } else if (state === "warning") {
    statusText.textContent = "警告: " + shortenPath(dir, MAX_PATH_CHARS - 9);
  }
}

let countdownTimer = null;

function startCountdown() {
  if (countdownTimer) clearInterval(countdownTimer);
  countdownTimer = setInterval(() => {
    if (window._state === "working" && window._remainingSecs > 0) {
      window._remainingSecs--;
      updateStatusText();
    }
  }, 1000);
}

async function saveColors() {
  const { invoke } = window.__TAURI__.core;
  await invoke("save_colors", {
    idle: colorIdle.value,
    working: colorWorking.value,
    stopped: colorStopped.value,
  });
}

async function loadTimeout() {
  try {
    const { invoke } = window.__TAURI__.core;
    const config = await invoke("get_config");
    const input = document.getElementById("timeout-input");
    if (input) input.value = config.idle_timeout_secs;
    return config.idle_timeout_secs;
  } catch (e) {
    console.log("加载配置失败:", e);
    return 60;
  }
}

async function loadAutostart() {
  try {
    const { invoke } = window.__TAURI__.core;
    const enabled = await invoke("get_autostart");
    const toggle = document.getElementById("autostart-toggle");
    if (toggle) toggle.checked = enabled;
  } catch (e) {
    console.log("加载自启配置失败:", e);
  }
}

// ─── 轻提示 ──────────────────────────────────────────

function showToast(msg) {
  const el = document.getElementById("toast");
  if (!el) return;
  el.textContent = msg;
  el.classList.remove("hidden");
  clearTimeout(el._timer);
  el._timer = setTimeout(() => el.classList.add("hidden"), 2000);
}

document.addEventListener("DOMContentLoaded", init);
