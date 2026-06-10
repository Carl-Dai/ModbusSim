# 提案：auto-update

## 为什么

ModbusSim 用户目前只能手动到 GitHub Releases 下载新版本，国内用户还要自行寻找代理。姊妹项目 IEC60870-5-104-Simulator 已有一套成熟的应用内自动更新机制（检查、提示、下载、安装、重启，多代理 endpoint 容灾），在 `deprecate-egui` 落地、技术栈收敛为纯 Tauri+Vue 后，可以近乎原样移植。

## 变更内容

- **新增**：两个 Tauri app（`modbussim-app` / `modbusmaster-app`）各增加 `update.rs`，提供 `check_for_update`（6 小时节流 + 24 小时 snooze）、`install_update`（带下载进度事件）、`snooze_update` 三个 command
- **新增**：依赖 `tauri-plugin-updater` / `tauri-plugin-process` / `tauri-plugin-store`，在 `lib.rs` 注册
- **新增**：`tauri.conf.json` 配置 `plugins.updater`（5 个 endpoint：gh.daichangyu.com 代理优先 → GitHub 直连 → 3 个公共代理兜底）与 minisign pubkey（复用 104 项目同一密钥对），`bundle.createUpdaterArtifacts: true`
- **新增**：`shared-frontend` 增加 `UpdateDialog.vue`（版本号、更新说明、下载进度、立即更新 / 稍后提醒），两个 App.vue 接线：启动 2 秒后静默检查 + 暴露手动强制检查入口
- **新增**：`scripts/gen-update-manifest.mjs` 生成 `latest-slave*.json` / `latest-master*.json`（每个代理一份 URL 前缀变体），release.yml 增加 `publish-manifest` job
- **修改**：release.yml 构建 job 注入 `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets，产出 `.sig` 签名文件
- **修改**：`tauri.conf.json` 的 `version` 从过期的 `0.1.0` 同步为当前实际版本（updater 依赖该字段做版本比对）

## 功能 (Capabilities)

### 新增功能

- `auto-update`: 应用内自动更新 — 定义更新检查节流、snooze、多 endpoint 容灾、签名校验、下载安装重启的行为

### 修改功能

（无 — 现有能力的规范级行为不变）

## 影响

- **代码**：`crates/modbussim-app`、`crates/modbusmaster-app`（update.rs + lib.rs + Cargo.toml + tauri.conf.json + capabilities）、`shared-frontend`（UpdateDialog）、`frontend` / `master-frontend`（App.vue 接线）、`scripts/`（manifest 脚本）
- **CI**：release.yml 增加签名 env 与 publish-manifest job；仓库需配置两个 secrets（与 104 项目共用同一对密钥）
- **发布产物**：GitHub Release 新增 `.sig` 文件与 10 个 manifest JSON（slave/master × 5 变体）
- **依赖**：本变更依赖 `deprecate-egui` 先行合并（release.yml diff 基线干净）
- **用户**：旧版本（无 updater）用户无法收到推送，需最后一次手动升级到首个带 updater 的版本
