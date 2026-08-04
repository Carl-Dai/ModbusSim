## 为什么

当前子站的随机变位只有**设备 + 寄存器类型**粒度：工具栏 `MutationControl` 用一个全局间隔 + 四个类型复选框，前端 `setTimeout` 周期性调用后端 `random_mutate_registers`，后端随机挑一批地址做 ±100 扰动。无法针对单个点位单独开关变位、设定变化模式与范围;且现有扰动直接在 u16 地址上 ±100、**忽略点位的 `data_type`**,对 `UInt32/Int32/Float32`(跨 2 个寄存器)的点位会破坏数值编码。

上一级别的 IEC 104 模拟器已有成熟的**点位粒度**随机变位(每点独立配置模式/周期/范围、独立驱动)。本变更将该模型移植到 Modbus 子站,使每个寄存器点位可独立配置随机变位。

## 变更内容

- **新增**点位级变位配置 `MutationConfig { enabled, mode, period_ms, step, min, max }`,挂在 `RegisterDef` 上并**随项目文件持久化**;`mode ∈ {Flip, Increment, Decrement, Random}`。
- **新增**后端单个长驻 tick 任务,以 100ms 基准扫描并按每点 `period_ms` 独立调度;总开关控制整体启停。运行时表仅保存不持久化的 `next_due` 与三角波方向。
- **修正**变位作用层:按点位 `data_type` 解码为工程值 → 变位 → 按 `endian` 编码写回 1~2 个寄存器(`min/max/step` 为工程值单位)。bool 点位(Coil/DiscreteInput)只支持翻转。
- **新增** Tauri 命令 `set_point_mutation` / `clear_point_mutation` / `list_point_mutations` / `set_mutation_running`。
- **BREAKING** 废弃命令 `random_mutate_registers` 与设备+类型级随机变位;工具栏 `MutationControl` 改造为总开关,周期在每个点位配置。
- **移除**从未接线的死代码 `jitter.rs` / `JitterConfig`(`apply_tick` 无任何调用,与点位级变位语义重叠)。
- **新增** UI:`RegisterTable` 每行变位状态图标 + 配置弹窗(`MutationConfigModal`),bool 点位仅显示翻转、隐藏范围/步长。

## 功能 (Capabilities)

### 新增功能
- `point-mutation`: 子站寄存器点位级随机变位 —— 每个点位独立配置变位模式/周期/范围,后端统一 tick 按每点周期在 data_type 工程值层驱动变位并持久化配置;前端行内配置入口与状态指示、工具栏总开关。

### 修改功能
<!-- 旧的设备+类型级随机变位无独立规范,仅为工具栏组件;无现有规范级需求变更。 -->

## 影响

- **core** `crates/modbussim-core`: `register.rs`(新增 `MutationConfig`/`MutationMode`、`RegisterDef.mutation`)、新增变位作用逻辑模块、移除 `jitter.rs`。
- **app** `crates/modbussim-app`: `commands.rs`(新增 4 命令、移除 `random_mutate_registers`)、`state.rs`(tick 任务与每点运行时状态)、`lib.rs`(命令注册)。
- **前端** `frontend/src`: `RegisterTable.vue`(行内图标+popover)、`MutationControl.vue`(改为总开关)、`Toolbar.vue`、类型定义与 i18n 文案。
- **持久化**: 项目文件 schema 增加 `RegisterDef.mutation`(`#[serde(default)]` 向后兼容旧文件)。
- **测试**: core 单测(各模式编解码、三角波边界反转、bool 仅翻转、序列化往返)、tick 调度单测。
