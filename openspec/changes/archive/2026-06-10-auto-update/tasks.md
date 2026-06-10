# 任务：auto-update

## 1. Rust 后端（两个 app 对称实施）

- [x] 1.1 `modbussim-app` / `modbusmaster-app` 的 Cargo.toml 添加 `tauri-plugin-updater`、`tauri-plugin-process`、`tauri-plugin-store`（均为 "2"）+ `chrono`
- [x] 1.2 从 104 移植 `update.rs` 到两个 app（逐字节相同）+ `tests/update_helpers.rs`（仅改 lib 名）
- [x] 1.3 `lib.rs` 注册三个插件并挂载 `check_for_update` / `install_update` / `snooze_update` 三个 command
- [x] 1.4 capabilities 无需修改 — 与 104 对齐：插件仅 Rust 侧调用，不走 JS API，无 ACL 条目（gen/schemas 由 tauri-build 自动再生成，纳入提交）
- [x] 1.5 `cargo check --workspace` + `cargo test --workspace` 全绿（两 app 各 6 个 update_helpers 测试通过）

## 2. Tauri 配置

- [x] 2.1 两个 `tauri.conf.json` 添加 `plugins.updater`：5 个 endpoint（gh.daichangyu.com → GitHub 直连 → gh-proxy.com → gh.idayer.com → ghfast.top，URL 指向 Karl-Dai/ModbusSim release 的 `latest-slave*.json` / `latest-master*.json`）+ 104 同款 pubkey
- [x] 2.2 两个 `tauri.conf.json` 设置 `bundle.createUpdaterArtifacts: true`
- [x] 2.3 两个 `tauri.conf.json` 的 `version` 从 `0.1.0` 同步为 `0.15.0`（最新 tag v0.15.0）

## 3. 前端

- [x] 3.1 从 104 移植 `UpdateDialog.vue` 到 `shared-frontend/src/components/`（CSS 变量映射为本项目 Catppuccin hex），i18n update.* 键加入共享字典（zh/en）
- [x] 3.2 `frontend/src/App.vue` 接线：onMounted 延迟 2s 静默检查、`provide('checkUpdate')`、UpdateDialog 挂载、snooze 处理
- [x] 3.3 `master-frontend/src/App.vue` 同步接线
- [x] 3.4 两个 Toolbar 添加"检查更新"按钮（force=true，失败 showAlert 错误、最新版提示 alreadyLatest；不引入 opener 镜像回退）
- [x] 3.5 `npm run build`（含 vue-tsc -b，两个前端）+ shared-frontend vitest 16 用例全绿

## 4. 发布链路

- [x] 4.1 从 104 移植 `scripts/gen-update-manifest.mjs` + `gen-update-manifest.test.mjs`：改 `REPO`、资产前缀（`ModbusSlave_` / `ModbusMaster_`），测试改写为 node:test 零依赖，10/10 通过
- [x] 4.2 从 104 移植 `scripts/test-update-proxies.sh` 并适配仓库名
- [x] 4.3 release.yml 两个 tauri-action step 注入 `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)` env，并加 `releaseDraft: true` + `includeUpdaterJson: false`（对齐 104 防竞态/防半成品 release）
- [x] 4.4 release.yml 新增 `publish-manifest` job：undraft → 生成 manifest → 重试上传 10 份 JSON
- [x] 4.5 提醒用户从 104 仓库复制两个 secrets 到本仓库（见交付说明）

## 5. 验证与收尾

- [x] 5.1 复查 endpoint 顺序与 `MANIFEST_VARIANTS` 顺序一致（cn0 → 直连 → cn2 → cn3 → cn1，与 104 相同）
- [x] 5.2 CHANGELOG `[Unreleased]` 记录新功能（中英对照）
- [x] 5.3 交付说明写明首次端到端验证路径：下一个 tag 发版后,装旧版手动触发"检查更新"走完整流程（用户执行）
