## 上下文

子站现有随机变位为"设备 + 寄存器类型"粒度:工具栏 `MutationControl.vue` 用全局间隔 + 四个类型复选框,前端 `setTimeout` 周期调用后端 `random_mutate_registers`,后端 `apply_random_mutation_inner` 在每个 u16 地址上随机 ±100。问题:(1) 无法按单点开关/设模式与范围;(2) 逐 u16 扰动忽略点位 `data_type`,破坏 `UInt32/Int32/Float32` 跨寄存器编码。

参考项目 `IEC60870-5-104-Simulator` 已实现点位粒度变位(每点 `MutationParams{mode,step,min,max}` + 独立周期,每点一个 tokio 任务,Flip/Increment/Decrement,配置不持久化)。本变更将该模型移植到 Modbus 子站并按 Modbus 特性调整。

相关结构:`RegisterDef{address,register_type,data_type,endian,name,comment}`(持久化点位定义)、`RegisterMap`(四个 `HashMap` 存值)、`SlaveDevice{...,register_defs,jitter}`。`jitter.rs`/`JitterConfig` 挂在 `SlaveDevice` 但 `apply_tick` 无任何调用,为死代码。

## 目标 / 非目标

**目标:**
- 每个寄存器点位可独立配置随机变位(模式/周期/范围/步长/启用),随项目持久化。
- 后端单任务统一驱动,每点独立周期。
- 变位按 `data_type` 在工程值层正确编解码,修正现有跨寄存器编码 bug。
- UI:表格行内配置 + 状态指示 + 工具栏总开关。

**非目标:**
- 不改动 Modbus 协议服务/读写命令路径本身。
- 不为主站(modbusmaster-app)引入变位。
- 不实现按点位独立的"随机种子可复现"机制(随机即可)。
- 不保留旧的设备+类型级随机变位 UI 与 `random_mutate_registers` 命令。

## 决策

**D1 配置挂 `RegisterDef` 并持久化(否决:运行时不持久化)。**
Modbus 点位定义本就随项目持久化,把变位作为点位属性符合用户对"项目可复现"的预期。104 选择不持久化是因其变位是临时调试行为;Modbus 子站更多用于长期模拟场景,持久化更有价值。新增 `RegisterDef.mutation: Option<MutationConfig>`,`#[serde(default)]` 保证旧文件兼容。

**D2 单个后端 tick 任务 + 每点 `next_due`(否决:每点一个 tokio 任务)。**
点位可达成百上千,104 的"每点一任务"在 Modbus 场景下任务数膨胀、总开关与生命周期管理复杂。改为:`AppState` 持有一个长驻 tick 任务,基准间隔固定(100ms)。运行时为每个启用点维护 `next_due: Instant`;每次基准 tick 扫描所有点,`now >= next_due` 才变位并 `next_due += period_ms`。如此单任务即可实现每点独立周期。`period_ms` 下限取基准间隔。

**D3 变位在工程值层按 `data_type` 编解码(修正 bug)。**
变位单元是"点位"而非"u16 地址"。流程:按 `data_type`+`endian` 从 `register_map` 解码工程值 → 按模式计算新工程值 → clamp 到 `data_type` 值域 → 按 `endian` 编码写回其占用的 1~2 个寄存器。`min/max/step` 为工程值单位。bool 点位(`data_type=Bool`)走独立翻转分支。复用 `register.rs` 现有的编解码逻辑(读写命令已用)。

**D4 运行时状态与持久化配置分离。**
持久化:`MutationConfig`(用户意图)。运行时(不持久化):三角波方向 `dir` 与 `next_due`,存于 `AppState` 的运行时表,key 为 `(connection_id, slave_id, register_type, address)`。总开关 `mutation_running: bool` 为运行时状态。

**D5 数值 Flip = `min/max` 两态切换(否决:取反)。**
取反对无符号/带范围语义不清(如 `UInt16` 取反含义模糊)。两态切换 `value ≤ (min+max)/2 ? max : min` 可控、可测,与"模拟开关量在两个标定值间跳变"的实际诉求一致。

**D6 移除死代码 `jitter.rs`/`JitterConfig`。**
`apply_tick` 无任何调用,与点位级变位语义重叠。一并移除 `SlaveDevice.jitter` 字段(`#[serde(default)]` 保证旧文件中残留该字段时被忽略,不影响反序列化)。

**D7 命令面。**
新增 `set_point_mutation(connection_id, slave_id, register_type, address, config)`(写入 `RegisterDef.mutation` 并刷新运行时表)、`clear_point_mutation(...)`(置 `enabled=false`/移除)、`list_point_mutations(connection_id, slave_id) -> Vec<{register_type,address,mode}>`(前端表格指示/轮询)、`set_mutation_running(bool)`(总开关)。废弃 `random_mutate_registers`。

**D8 前端值刷新复用现有机制。**
后端 tick 改值后,前端沿用 `useRegisterValues` 的批量刷新拉取最新值;变位总开关启用时每约 2 秒刷新当前从站值,并独立轮询 `list_point_mutations` 更新行内状态指示(对齐 104)。

## 风险 / 权衡

- **跨 2 寄存器点位地址重叠** → 后端按 `data_type.register_count` 校验完整占用范围并拒绝重叠或越过 65535 的定义,避免两个点位争用同一原始寄存器。
- **浮点 `min/max/step` 与整型寄存器钳制** → 编码前按 `data_type` 值域 clamp;`Float32` 不额外钳制(除非超 f32 范围)。
- **基准 100ms 限制最小周期** → `period_ms` 实际下限为 100ms;文档说明,满足模拟场景。比 100ms 更密的变位非目标。
- **大量启用点的扫描开销** → 单任务每 100ms 线性扫描启用点;点位规模(数千)下可接受;仅扫描"启用"集合而非全部寄存器。
- **总开关与单点 enabled 的关系** → 总开关为运行时全局闸;`enabled` 为点位持久意图。仅当总开关开 且 点位 `enabled` 时该点变位。

## 迁移计划

1. core 加 `MutationConfig`/`MutationMode` 与 `RegisterDef.mutation`(serde default);移除 `jitter.rs` 及 `SlaveDevice.jitter`。
2. core 加点位变位作用函数(按 data_type 编解码 + 四模式 + 三角波方向)。
3. app 加运行时变位状态表 + tick 任务 + 4 个命令;移除 `random_mutate_registers` 及其注册。
4. 前端:`RegisterTable.vue` 行内图标+popover;`MutationControl.vue` 改总开关;类型与 i18n。
5. 旧项目文件无需迁移脚本:`#[serde(default)]` 自动兼容;残留 `jitter` 字段被忽略。
6. 回滚:本变更为新增能力 + 局部替换,回滚即还原上述文件。
