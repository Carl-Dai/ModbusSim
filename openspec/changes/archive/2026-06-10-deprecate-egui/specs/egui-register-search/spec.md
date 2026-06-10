# 增量规范：egui-register-search

## REMOVED Requirements

### 需求:寄存器视图 always-visible 搜索框
**Reason**: egui 轨道整体废弃，技术栈收敛到 Tauri+Vue 单轨
**Migration**: 无直接替代；Vue 轨寄存器界面的搜索/过滤能力按其自身规范演进

### 需求:地址跳转（纯数字输入）
**Reason**: egui 轨道整体废弃
**Migration**: 无直接替代；如 Vue 轨需要该能力，另立变更提案

### 需求:名称 / 注释模糊过滤
**Reason**: egui 轨道整体废弃
**Migration**: 无直接替代；如 Vue 轨需要该能力，另立变更提案

### 需求:Cmd+F / Ctrl+F 快捷键聚焦搜索框
**Reason**: egui 轨道整体废弃
**Migration**: 无直接替代；如 Vue 轨需要该能力，另立变更提案
