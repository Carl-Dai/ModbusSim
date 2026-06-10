# 设计：deprecate-egui

## 上下文

ModbusSim 自 v0.14 起并行维护两条 UI 轨道：Tauri+Vue（`modbussim-app` / `modbusmaster-app` + `frontend` / `master-frontend` / `shared-frontend`）与 egui 原生（`modbussim-egui` / `modbusmaster-egui` + `modbussim-ui-shared`）。两轨共享 `modbussim-core` 协议内核，功能上互为镜像但 UI 代码各写一遍。姊妹项目 IEC60870-5-104-Simulator 是纯 Tauri+Vue 单轨，已验证该栈满足全部需求；后续要对齐的自动更新机制（tauri-plugin-updater）也只覆盖 Tauri 轨。

## 目标 / 非目标

**目标：**
- 删除 egui 轨道全部代码、CI 与发布产物，workspace 收敛到 core + 两个 Tauri app
- 删除后 `cargo check --workspace`、`cargo test --workspace`、`npm run build` 全绿
- 归档 egui 相关的两个能力规范（`egui-register-search`、`egui-visual-style`）

**非目标：**
- 不在 Tauri 轨补齐 egui 独有的 UI 细节（搜索框、视觉规范等按 Vue 轨自身节奏演进）
- 不改动 `modbussim-core` 任何代码
- 不清理 CHANGELOG、`docs/releases/`、`docs/superpowers/` 中的 egui 历史记录

## 决策

1. **一次性删除而非渐进弃用**。egui 版无独立用户配置/数据格式（寄存器配置 JSON 与 Tauri 版同构），不存在数据迁移问题；保留"deprecated 但仍发布"的中间态只会延长双轨维护。替代方案（保留 egui 只修 bug 不加功能）被否决——CI 和 release 链路的维护成本仍在。

2. **`modbussim-ui-shared` 一并删除**。已确认其唯一消费者是两个 egui crate（`grep modbussim-ui-shared crates/*/Cargo.toml`），删除后无悬空引用。

3. **release.yml 只删 `release-egui` job 与 release body 中的 egui 段落**，不重构其余部分——为后续 auto-update 变更保留干净的 diff 基线。

4. **规范处理用 REMOVED 增量**。两个 egui 能力规范通过本变更的 delta spec 标记移除，归档时由 openspec 工具同步删除项目级 `openspec/specs/` 下的对应目录。

## 风险 / 权衡

- [egui 用户失去更新渠道] → 在下一个 release notes 中显著位置公告停更，并附 Tauri 版下载与功能对照说明
- [删除后某处仍引用 egui crate 导致编译失败] → 验证手段就是 `cargo check --workspace` + 全文 grep `modbussim-egui|modbusmaster-egui|modbussim-ui-shared`，CI 双保险
- [Tauri 版在低配/无 WebView 环境不可用（egui 版原卖点）] → 接受该权衡；104 项目同栈未收到此类反馈，Windows WebView2 与 macOS WKWebView 覆盖面足够

## 迁移计划

1. 删 crate 目录 + workspace 条目 → `cargo check`
2. 删 CI/release 配置 → workflow 语法校验（actionlint 或推分支观察）
3. 改 README → 人工审阅
4. 回滚策略：单 commit 或单 PR 完成，revert 即可整体恢复
