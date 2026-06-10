# 任务：deprecate-egui

## 1. 删除 egui 代码

- [x] 1.1 删除 `crates/modbussim-egui`、`crates/modbusmaster-egui`、`crates/modbussim-ui-shared` 三个目录
- [x] 1.2 从 workspace `Cargo.toml` 移除三个 members 条目及 `[workspace.dependencies]` 中的 `modbussim-ui-shared`、`eframe`/`egui` 等仅 egui 使用的依赖
- [x] 1.3 全文 grep `modbussim-egui|modbusmaster-egui|modbussim-ui-shared|eframe`，确认无残留引用（CHANGELOG / docs 历史记录除外）
- [x] 1.4 运行 `cargo check --workspace` 与 `cargo test --workspace`，确认全绿；`Cargo.lock` 中 egui 系依赖消失

## 2. 清理 CI 与发布链路

- [x] 2.1 删除 `.github/workflows/ci-egui.yml`
- [x] 2.2 从 `.github/workflows/release.yml` 移除 `release-egui` job
- [x] 2.3 从 `release.yml` 两处 tauri-action 的 release body 中删除 egui 版下载说明段落
- [x] 2.4 校验 workflow YAML 语法（actionlint 或 `yamllint`，本地无工具则人工复查缩进与 job 依赖）

## 3. 更新文档

- [x] 3.1 `README.md` 删除 egui 双版本介绍，统一为 Tauri 单轨说明
- [x] 3.2 `README_CN.md` 同步修改
- [x] 3.3 在 CHANGELOG `[Unreleased]` 段新增 BREAKING 条目：egui 版停止发布，附 Tauri 版迁移指引

## 4. 验证与收尾

- [x] 4.1 `npm run build`（frontend / master-frontend）确认前端构建不受影响
- [x] 4.2 复查 git diff：每一行改动可追溯到本提案，未触碰 `modbussim-core` 与历史文档
