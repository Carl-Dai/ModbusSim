# Aptabase 匿名遥测接入 — 设计文档

- 日期：2026-06-11
- 状态：待实现
- 目标：在 slave / master 两个 Tauri 应用中接入 Aptabase 匿名使用统计，让作者能知道「装机量、活跃度、版本分布、OS 分布」，但**不采集任何 PII、不识别个体**。

## 背景与约束

- GitHub Release 只提供匿名累计下载数，无法得知谁下载、谁在用。要知道「装机/活跃」必须在应用内埋点。
- 技术栈：Tauri v2.10.3 workspace，两个 app crate（`modbussim-app` = slave，`modbusmaster-app` = master）共享 `modbussim-core`；前端 `frontend` / `master-frontend` + `shared-frontend`。
- 已用插件：`tauri-plugin-store`（设置持久化，store helper 见 `update.rs`）、`tauri-plugin-updater` 等。
- 遵守 `.claude/CLAUDE.md` 第 5 条：Claude 不自启 GUI 做自测；遥测真链路由用户手动验证。

## 已确认的设计决策

| 维度 | 决策 |
| --- | --- |
| 托管 | Aptabase **云托管免费版**（aptabase.com SaaS） |
| 隐私默认 | **opt-out**：默认开启，设置里可关闭 |
| 应用区分 | **单一 Aptabase 应用**，两端共用 App Key `A-US-1280171703`，靠 `app_started` 的 `edition` 属性区分 slave / master |
| 事件范围 | **仅 `app_started`**（应用生命周期），后续按需再加 |
| 开关 UI | **「关于」气泡**：扩展 `VersionBadge`，弹出含版本号 + GitHub 链接 + 遥测勾选框 |

## 架构

- 仅用 **Rust 插件 `tauri-plugin-aptabase`**（`Builder::new(APP_KEY).build()`）。`app_started` 在 Rust `.setup()` 中触发。
- 因此**前端不引入 `@aptabase/tauri` npm 包，也不需要 `aptabase:allow-track-event` ACL**——前端只通过自定义 Tauri 命令读写开关。
- App Key 按 Aptabase 设计为**非机密**（随客户端分发），作为常量直接写入各 app 的源码并提交，不进环境变量。两端共用同一个 Key `A-US-1280171703`。

## 组件与改动（两个 app 对称，沿用现有 `update.rs` 双份模式）

### Rust

每个 app crate 新增 `analytics.rs`（约 30 行）：

- `const APTABASE_KEY: &str = "A-US-1280171703"` — 两端相同。
- `const EDITION: &str` — slave crate 为 `"slave"`，master crate 为 `"master"`（用于区分两端）。
- `const STORE_FILE: &str = "settings.json"`、`const KEY_ENABLED: &str = "analytics_enabled"`。
- `is_enabled(app) -> bool`：读 store，**缺省 `true`**（opt-out）。
- `track_started(app)`：`is_enabled` 为真时 `app.track_event("app_started", Some(json!({"edition": EDITION})))`（需 `use tauri_plugin_aptabase::EventTracker;`）。
- `#[tauri::command] get_analytics_enabled(app) -> bool`
- `#[tauri::command] set_analytics_enabled(app, enabled: bool)`：写 store 并 `save()`。

`lib.rs` 改动：

1. `mod analytics;`
2. 插件链加 `.plugin(tauri_plugin_aptabase::Builder::new(analytics::APTABASE_KEY).build())`。
3. `.setup()` 内加 `analytics::track_started(app.handle());`。
4. `generate_handler!` 注册 `analytics::get_analytics_enabled, analytics::set_analytics_enabled`。
5. 结尾把 `.run(tauri::generate_context!())` 改为：

   ```rust
   .build(tauri::generate_context!())
   .expect("error while building tauri application")
   .run(|app_handle, event| {
       if let tauri::RunEvent::Exit = event {
           use tauri_plugin_aptabase::EventTracker;
           app_handle.flush_events_blocking();
       }
   });
   ```

   理由：插件按定时器批量上报，短会话若不在退出时 flush，`app_started` 可能丢失。

`Cargo.toml`（两个 app）：加 `tauri-plugin-aptabase = "1"`。

### 前端（`shared-frontend`，两个前端共用）

- 扩展 `shared-frontend/src/components/VersionBadge.vue`：把当前「版本号 + GitHub 图标」徽章改为可点击，点击弹出一个轻量气泡（popover），内含：
  - 版本号文本
  - GitHub 仓库链接（保留现有 opener 行为）
  - 一个勾选框「分享匿名使用统计」，下方一行小字说明「仅采集版本/系统等匿名信息，不含任何个人数据」。
- 气泡挂载时 `invoke('get_analytics_enabled')` 初始化勾选状态；切换时 `invoke('set_analytics_enabled', { enabled })`。
- 文案走现有 i18n（`shared-frontend/src/i18n/locales/{en-US,zh-CN}.ts`）新增对应键。
- 两个前端的 `Toolbar.vue` 已经用 `VersionBadge`，**无需改动**（组件内部扩展即可）。

## 数据流

1. 启动 → `.setup()` 读 store 开关 → 开启则 `track_event("app_started")` → 插件缓冲。
2. 定时 flush + 退出时 `flush_events_blocking()` → 送达 Aptabase。
3. 用户在关于气泡切换 → `invoke set_analytics_enabled` → 写 store → **下次启动生效**（`app_started` 每次启动只发一次，延迟到下次生效可接受）。

## 采集内容与隐私

- 仅 `app_started`，自定义属性 `edition`（slave/master）。Aptabase 自动附带：app 版本、OS、locale、国家（由请求 IP 现场推算、**不持久化 IP**）、匿名会话 ID。
- **无任何 PII，不识别个体。**
- README（中英）新增「匿名使用统计 / Anonymous Usage Analytics」小节，说明采集内容、用途、以及在「关于」气泡里关闭的方法。

## 验证

- Rust：两个 app `cargo check` + `cargo clippy` 通过。
- 前端：`vue-tsc --noEmit` / `npm run build` 通过。
- 手动（用户执行）：
  - 启动 app → Aptabase 仪表盘出现 `app_started` 事件、带正确版本/OS。
  - 关闭开关 → 重启 → 仪表盘不再新增事件。

## 前置条件（已就绪）

- App Key 已提供：`A-US-1280171703`（单一应用，两端共用），无占位、可直接实现。
- Aptabase 仪表盘可用 `edition` 属性把 slave / master 拆开查看。

## 非目标（YAGNI）

- 不采集功能级埋点（连接建立、扫描等）——后续按需再加。
- 不做 `app_exited` / 会话时长——首版只要装机与活跃。
- 不做首次启动的弹窗告知——以 README + 设置内说明为准（如需可后续加）。
- 不自建 Aptabase 实例。
