# 提案：deprecate-egui

## 为什么

ModbusSim 目前维护 Tauri+Vue 与 egui 两条并行的 UI 轨道，每个功能（i18n、TLS、状态反馈等）都要实现两遍，维护成本翻倍。姊妹项目 IEC60870-5-104-Simulator 已验证纯 Tauri+Vue 技术栈可行且开发效率更高，决定废弃 egui 轨道，全面对齐 104 的技术栈，为后续自动更新等能力（依赖 tauri-plugin-updater）扫清障碍。

## 变更内容

- **移除**：`crates/modbussim-egui`、`crates/modbusmaster-egui`、`crates/modbussim-ui-shared` 三个 crate（约 8.4k 行 Rust，`modbussim-ui-shared` 仅被两个 egui crate 引用）
- **移除**：workspace `Cargo.toml` 中对应的 members 与 `[workspace.dependencies]` 条目
- **移除**：`.github/workflows/ci-egui.yml` 整个工作流
- **移除**：`.github/workflows/release.yml` 中的 `release-egui` job，以及 tauri-action release body 中的 egui 版下载说明
- **修改**：`README.md` / `README_CN.md` 删除 egui 双版本介绍，统一为 Tauri 单轨说明
- **保留**：`modbussim-core`（协议内核，两轨共享，Tauri 轨继续使用）；`CHANGELOG.md`、`docs/releases/`、`docs/superpowers/` 中的 egui 历史记录不动
- **BREAKING**：从下一个 release 起不再发布 `-egui-` 后缀的原生二进制，egui 版用户需迁移到 Tauri 版

## 功能 (Capabilities)

### 新增功能

（无）

### 修改功能

- `egui-register-search`: 移除 — 该能力随 egui 轨道废弃，对应需求全部删除
- `egui-visual-style`: 移除 — 该能力随 egui 轨道废弃，对应需求全部删除

## 影响

- **代码**：删除 3 个 crate；workspace members 从 6 个缩减到 3 个（core / slave-app / master-app）
- **CI**：`ci-egui.yml` 删除；`release.yml` 体积缩小，后续 auto-update 变更的 diff 不再与 egui job 纠缠
- **发布产物**：GitHub Release 不再包含 `ModbusSlave-egui-*` / `ModbusMaster-egui-*` 压缩包
- **用户**：egui 版无自动迁移路径，需在 release notes 中公告停更并指引下载 Tauri 版
- **依赖**：`eframe` / `egui` 等依赖从 lockfile 消失，CI 编译时间缩短
