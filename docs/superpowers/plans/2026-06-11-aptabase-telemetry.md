# Aptabase 匿名遥测接入 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 slave / master 两个 Tauri 应用接入 Aptabase 匿名遥测，启动时上报 `app_started`（带 `edition` 区分两端），默认开启、可在「关于」气泡里关闭。

**Architecture:** 仅用 Rust 插件 `tauri-plugin-aptabase`，事件从 `.setup()` 手动触发；opt-out 开关存 `tauri-plugin-store`（`settings.json`），前端通过自定义命令读写，不引入 aptabase 的 JS 包。两端共用 App Key `A-US-1280171703`。

**Tech Stack:** Rust / Tauri v2.10.3 / tauri-plugin-aptabase 1.x / tauri-plugin-store / Vue 3 + vue-tsc / shared-frontend i18n。

**验证约定（重要）：** 本特性是 Tauri 集成胶水代码 + 网络上报，无可独立单测的纯逻辑；按项目 `.claude/CLAUDE.md` 第 5 条，GUI/真链路由用户手动验证，Claude 不自启 GUI。每个任务的自动化验证 = `cargo check` + `cargo clippy`（Rust）或 `npm run build`（含 `vue-tsc -b`，前端）。遥测真链路（事件进 Aptabase 仪表盘、关开关后停报）由用户手动验证，见最后「手动验证」。

---

## File Structure

| 文件 | 动作 | 职责 |
| --- | --- | --- |
| `crates/modbussim-app/Cargo.toml` | 改 | 加 `tauri-plugin-aptabase` 依赖 |
| `crates/modbussim-app/src/analytics.rs` | 建 | slave 端遥测：开关读写 + `track_started`（edition=slave）+ 两个命令 |
| `crates/modbussim-app/src/lib.rs` | 改 | 注册插件/命令、setup 触发、退出 flush |
| `crates/modbusmaster-app/Cargo.toml` | 改 | 加依赖 |
| `crates/modbusmaster-app/src/analytics.rs` | 建 | master 端遥测（edition=master），其余同 slave |
| `crates/modbusmaster-app/src/lib.rs` | 改 | 同 slave |
| `shared-frontend/src/i18n/locales/zh-CN.ts` | 改 | 加 `about` 文案 |
| `shared-frontend/src/i18n/locales/en-US.ts` | 改 | 加 `about` 文案 |
| `shared-frontend/src/components/VersionBadge.vue` | 改 | 加「关于」气泡 + 遥测开关 |
| `README.md` / `README_CN.md` | 改 | 加「匿名使用统计」说明 |

`analytics.rs` 两份几乎相同（只差 `EDITION` 常量），沿用现有 `update.rs` 的双份对称模式，不抽公共 crate。

---

### Task 1: slave 端 Rust 遥测

**Files:**
- Modify: `crates/modbussim-app/Cargo.toml`
- Create: `crates/modbussim-app/src/analytics.rs`
- Modify: `crates/modbussim-app/src/lib.rs`

- [ ] **Step 1: 加依赖**

在 `crates/modbussim-app/Cargo.toml` 的 `[dependencies]` 末尾（`tauri-plugin-opener = "2"` 那行之后）加：

```toml
tauri-plugin-aptabase = "1"
```

- [ ] **Step 2: 新建 `crates/modbussim-app/src/analytics.rs`**

```rust
use serde_json::json;
use tauri::AppHandle;
use tauri_plugin_aptabase::EventTracker;
use tauri_plugin_store::StoreExt;

/// 非机密：Aptabase App Key 随客户端分发，可直接提交。两端共用同一个 Key。
pub const APTABASE_KEY: &str = "A-US-1280171703";
const EDITION: &str = "slave";
const STORE_FILE: &str = "settings.json";
const KEY_ENABLED: &str = "analytics_enabled";

/// 读开关，缺省 true（opt-out）。
fn is_enabled(app: &AppHandle) -> bool {
    app.store(STORE_FILE)
        .ok()
        .and_then(|s| s.get(KEY_ENABLED))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// 启动时调用：开启则上报 app_started（带 edition 区分两端）。
pub fn track_started(app: &AppHandle) {
    if is_enabled(app) {
        app.track_event("app_started", Some(json!({ "edition": EDITION })));
    }
}

#[tauri::command]
pub fn get_analytics_enabled(app: AppHandle) -> bool {
    is_enabled(&app)
}

#[tauri::command]
pub fn set_analytics_enabled(app: AppHandle, enabled: bool) {
    if let Ok(store) = app.store(STORE_FILE) {
        store.set(KEY_ENABLED, serde_json::Value::Bool(enabled));
        let _ = store.save();
    }
}
```

- [ ] **Step 3: 改 `crates/modbussim-app/src/lib.rs` — 声明模块**

把顶部：

```rust
mod commands;
mod state;
pub mod update;
```

改为：

```rust
mod analytics;
mod commands;
mod state;
pub mod update;
```

- [ ] **Step 4: 改 `lib.rs` — 注册插件**

在插件链里 `.plugin(tauri_plugin_opener::init())` 那行之后加一行：

```rust
        .plugin(tauri_plugin_aptabase::Builder::new(analytics::APTABASE_KEY).build())
```

- [ ] **Step 5: 改 `lib.rs` — 注册命令**

在 `generate_handler!` 列表里 `update::snooze_update,` 那行之后加：

```rust
            // Analytics commands
            analytics::get_analytics_enabled,
            analytics::set_analytics_enabled,
```

- [ ] **Step 6: 改 `lib.rs` — setup 触发 + 退出 flush**

把结尾这段：

```rust
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
```

改为：

```rust
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            analytics::track_started(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                use tauri_plugin_aptabase::EventTracker;
                app_handle.flush_events_blocking();
            }
        });
```

> 注：`app.handle()` 在 Tauri 2 返回 `&AppHandle`，直接传入 `track_started`；若编译器报借用类型不符，按提示加/去 `&`。退出 flush 保证短会话的 `app_started` 不丢。

- [ ] **Step 7: 验证编译与 lint**

Run: `cargo check -p modbussim-app && cargo clippy -p modbussim-app -- -D warnings`
Expected: 通过，无错误、无 warning。

- [ ] **Step 8: Commit**

```bash
git add crates/modbussim-app/Cargo.toml crates/modbussim-app/src/analytics.rs crates/modbussim-app/src/lib.rs Cargo.lock
git commit -m "feat(slave): 接入 Aptabase 匿名遥测 app_started"
```

---

### Task 2: master 端 Rust 遥测

**Files:**
- Modify: `crates/modbusmaster-app/Cargo.toml`
- Create: `crates/modbusmaster-app/src/analytics.rs`
- Modify: `crates/modbusmaster-app/src/lib.rs`

- [ ] **Step 1: 加依赖**

在 `crates/modbusmaster-app/Cargo.toml` 的 `[dependencies]` 末尾（`tauri-plugin-opener = "2"` 之后）加：

```toml
tauri-plugin-aptabase = "1"
```

- [ ] **Step 2: 新建 `crates/modbusmaster-app/src/analytics.rs`**

与 slave 版**仅 `EDITION` 不同**：

```rust
use serde_json::json;
use tauri::AppHandle;
use tauri_plugin_aptabase::EventTracker;
use tauri_plugin_store::StoreExt;

/// 非机密：Aptabase App Key 随客户端分发，可直接提交。两端共用同一个 Key。
pub const APTABASE_KEY: &str = "A-US-1280171703";
const EDITION: &str = "master";
const STORE_FILE: &str = "settings.json";
const KEY_ENABLED: &str = "analytics_enabled";

/// 读开关，缺省 true（opt-out）。
fn is_enabled(app: &AppHandle) -> bool {
    app.store(STORE_FILE)
        .ok()
        .and_then(|s| s.get(KEY_ENABLED))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// 启动时调用：开启则上报 app_started（带 edition 区分两端）。
pub fn track_started(app: &AppHandle) {
    if is_enabled(app) {
        app.track_event("app_started", Some(json!({ "edition": EDITION })));
    }
}

#[tauri::command]
pub fn get_analytics_enabled(app: AppHandle) -> bool {
    is_enabled(&app)
}

#[tauri::command]
pub fn set_analytics_enabled(app: AppHandle, enabled: bool) {
    if let Ok(store) = app.store(STORE_FILE) {
        store.set(KEY_ENABLED, serde_json::Value::Bool(enabled));
        let _ = store.save();
    }
}
```

- [ ] **Step 3: 改 `crates/modbusmaster-app/src/lib.rs` — 声明模块**

把顶部 `mod commands;` 那组改为在最前加 `mod analytics;`：

```rust
mod analytics;
mod commands;
mod state;
pub mod update;
```

- [ ] **Step 4: 改 `lib.rs` — 注册插件**

在 `.plugin(tauri_plugin_opener::init())` 之后加：

```rust
        .plugin(tauri_plugin_aptabase::Builder::new(analytics::APTABASE_KEY).build())
```

- [ ] **Step 5: 改 `lib.rs` — 注册命令**

在 `generate_handler!` 列表里 `update::snooze_update,` 之后加：

```rust
            // Analytics commands
            analytics::get_analytics_enabled,
            analytics::set_analytics_enabled,
```

- [ ] **Step 6: 改 `lib.rs` — setup 触发 + 退出 flush**

把结尾 `.setup(...)` + `.run(tauri::generate_context!())` 段改为（与 slave 完全相同）：

```rust
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            analytics::track_started(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                use tauri_plugin_aptabase::EventTracker;
                app_handle.flush_events_blocking();
            }
        });
```

- [ ] **Step 7: 验证编译与 lint**

Run: `cargo check -p modbusmaster-app && cargo clippy -p modbusmaster-app -- -D warnings`
Expected: 通过。

- [ ] **Step 8: Commit**

```bash
git add crates/modbusmaster-app/Cargo.toml crates/modbusmaster-app/src/analytics.rs crates/modbusmaster-app/src/lib.rs Cargo.lock
git commit -m "feat(master): 接入 Aptabase 匿名遥测 app_started"
```

---

### Task 3: 前端 i18n 文案

**Files:**
- Modify: `shared-frontend/src/i18n/locales/zh-CN.ts`
- Modify: `shared-frontend/src/i18n/locales/en-US.ts`

- [ ] **Step 1: zh-CN 加 `about` 段**

在 `shared-frontend/src/i18n/locales/zh-CN.ts` 中，作为顶层键插入（放在 `update:` 段之前），与 `toolbar`/`update` 同级：

```typescript
  about: {
    title: '关于',
    analytics: '分享匿名使用统计',
    analyticsNote: '仅采集版本、系统等匿名信息，不含任何个人数据。下次启动生效。',
  },
```

- [ ] **Step 2: en-US 加 `about` 段**

在 `shared-frontend/src/i18n/locales/en-US.ts` 同样位置插入：

```typescript
  about: {
    title: 'About',
    analytics: 'Share anonymous usage analytics',
    analyticsNote: 'Only anonymous info like app version and OS — no personal data. Takes effect on next launch.',
  },
```

- [ ] **Step 3: 验证类型**

Run: `npm run build -w frontend`
Expected: `vue-tsc -b` 通过（两个 locale 形状一致，无类型错误），vite 构建成功。

- [ ] **Step 4: Commit**

```bash
git add shared-frontend/src/i18n/locales/zh-CN.ts shared-frontend/src/i18n/locales/en-US.ts
git commit -m "i18n: 新增 about 段(遥测开关文案)"
```

---

### Task 4: VersionBadge「关于」气泡 + 遥测开关

**Files:**
- Modify: `shared-frontend/src/components/VersionBadge.vue`

设计：保留现有「版本号 + GitHub 图标」行，新增一个 ⓘ 信息按钮；点它弹出气泡，含「ModbusSim v{version}」标题、遥测勾选框、说明小字。气泡用全屏透明遮罩点击关闭。挂载时 `get_analytics_enabled` 初始化，切换时 `set_analytics_enabled`。非 Tauri 环境（纯浏览器 vite dev）invoke 抛错则静默降级。

- [ ] **Step 1: 用以下完整内容覆盖 `shared-frontend/src/components/VersionBadge.vue`**

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'shared-frontend'

const { t } = useI18n()

const REPO_URL = 'https://github.com/Karl-Dai/ModbusSim'
const version = ref('')
const showPanel = ref(false)
const analyticsEnabled = ref(true)

onMounted(async () => {
  try { version.value = await getVersion() } catch { version.value = '' }
  try { analyticsEnabled.value = await invoke<boolean>('get_analytics_enabled') } catch { /* not in Tauri */ }
})

// In a Tauri webview window.open can't reach the OS browser, so go through the
// opener plugin via raw invoke (shared-frontend only depends on @tauri-apps/api).
// Outside Tauri (plain vite dev in browser) invoke throws — fall back to window.open.
async function openRepo() {
  try {
    await invoke('plugin:opener|open_url', { url: REPO_URL })
  } catch {
    window.open(REPO_URL, '_blank')
  }
}

async function toggleAnalytics() {
  analyticsEnabled.value = !analyticsEnabled.value
  try {
    await invoke('set_analytics_enabled', { enabled: analyticsEnabled.value })
  } catch { /* not in Tauri */ }
}
</script>

<template>
  <div class="version-badge">
    <span v-if="version" class="version-text" :title="`v${version}`">v{{ version }}</span>
    <button
      type="button"
      class="github-link"
      :title="REPO_URL"
      :aria-label="REPO_URL"
      @click="openRepo"
    >
      <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true">
        <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38
                 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13
                 -.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66
                 .07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15
                 -.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0
                 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82
                 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01
                 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"/>
      </svg>
    </button>
    <button
      type="button"
      class="about-toggle"
      :title="t('about.title')"
      :aria-label="t('about.title')"
      @click="showPanel = !showPanel"
    >
      <svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor" aria-hidden="true">
        <path d="M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13zM8 4a1 1 0 1 1 0 2 1 1 0 0 1 0-2zM7 7h2v5H7z"/>
      </svg>
    </button>

    <template v-if="showPanel">
      <div class="about-backdrop" @click="showPanel = false"></div>
      <div class="about-panel" role="dialog" :aria-label="t('about.title')">
        <div class="about-title">ModbusSim<span v-if="version"> v{{ version }}</span></div>
        <label class="about-row">
          <input type="checkbox" :checked="analyticsEnabled" @change="toggleAnalytics" />
          <span>{{ t('about.analytics') }}</span>
        </label>
        <div class="about-note">{{ t('about.analyticsNote') }}</div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.version-badge {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 0 4px;
  font-size: 11px;
  color: #6c7086;
  font-variant-numeric: tabular-nums;
}
.version-text {
  padding: 2px 4px;
  line-height: 1;
  white-space: nowrap;
}
.github-link,
.about-toggle {
  display: inline-flex;
  align-items: center;
  padding: 3px 4px;
  margin: 0;
  border: none;
  background: transparent;
  color: inherit;
  cursor: pointer;
  border-radius: 4px;
  line-height: 1;
}
.github-link:hover,
.about-toggle:hover { color: #cdd6f4; background: #313244; }
.github-link svg,
.about-toggle svg { display: block; }

.about-backdrop {
  position: fixed;
  inset: 0;
  z-index: 40;
}
.about-panel {
  position: absolute;
  top: 100%;
  right: 0;
  margin-top: 4px;
  z-index: 41;
  width: 240px;
  padding: 10px 12px;
  background: #1e1e2e;
  border: 1px solid #313244;
  border-radius: 8px;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.35);
  color: #cdd6f4;
  font-size: 12px;
  text-align: left;
}
.about-title {
  font-weight: 600;
  margin-bottom: 8px;
}
.about-row {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
.about-row input { cursor: pointer; }
.about-note {
  margin-top: 6px;
  color: #6c7086;
  line-height: 1.4;
}
</style>
```

- [ ] **Step 2: 验证类型 + 构建（两个前端都消费 shared-frontend）**

Run: `npm run build -w frontend && npm run build -w master-frontend`
Expected: 两个工程 `vue-tsc -b` 通过、vite 构建成功。

- [ ] **Step 3: Commit**

```bash
git add shared-frontend/src/components/VersionBadge.vue
git commit -m "feat(ui): VersionBadge 加关于气泡与匿名遥测开关"
```

---

### Task 5: README 隐私说明

**Files:**
- Modify: `README.md`
- Modify: `README_CN.md`

- [ ] **Step 1: README.md 加一节**

在 `README.md` 末尾（或「License」小节之前）插入：

```markdown
## Anonymous Usage Analytics

ModbusSim sends an anonymous `app_started` event on launch via [Aptabase](https://aptabase.com), so the author can see install counts, active usage, and version/OS distribution. It collects **no personal data** — only app version, OS, locale, and an approximate country derived from your IP (the IP itself is never stored). You can turn it off anytime via the ⓘ "About" popover in the toolbar.
```

- [ ] **Step 2: README_CN.md 加一节**

在 `README_CN.md` 对应位置插入：

```markdown
## 匿名使用统计

ModbusSim 启动时通过 [Aptabase](https://aptabase.com) 发送一个匿名的 `app_started` 事件，便于作者了解装机量、活跃度与版本/系统分布。它**不采集任何个人数据**——只有应用版本、操作系统、语言和由 IP 现场推算的大致国家（IP 本身从不存储）。可随时在工具栏的 ⓘ「关于」气泡里关闭。
```

- [ ] **Step 3: Commit**

```bash
git add README.md README_CN.md
git commit -m "docs(readme): 补匿名使用统计(Aptabase)隐私说明"
```

---

## 手动验证（用户执行，Claude 不自启 GUI）

实现并构建通过后，由用户：

1. 启动 slave 与 master 应用各一次。
2. 登录 aptabase.com 仪表盘，确认出现 `app_started` 事件，且能按 `edition`（slave/master）区分，版本/OS 正确。
3. 在工具栏 ⓘ「关于」气泡里取消勾选 → 重启对应应用 → 确认仪表盘不再新增该端事件；重新勾选 → 重启 → 恢复上报。

## 完成判据

- [ ] 两个 crate `cargo check` + `cargo clippy -D warnings` 通过。
- [ ] `frontend` 与 `master-frontend` `npm run build` 通过。
- [ ] 用户手动验证 3 步全部符合预期。
