## 1. core 数据模型与死代码清理

- [x] 1.1 在 `register.rs` 新增 `MutationMode`(flip|increment|decrement|random,serde rename_all snake_case)与 `MutationConfig{enabled,mode,period_ms,step,min,max}`(Serialize/Deserialize)
- [x] 1.2 在 `RegisterDef` 增加 `#[serde(default)] pub mutation: Option<MutationConfig>`
- [x] 1.3 移除 `jitter.rs`、`lib.rs` 中 `pub mod jitter;`、`SlaveDevice.jitter` 字段及其 `new()`/构造处初始化(保留旧文件反序列化兼容:`SlaveDevice` 反序列化忽略残留 `jitter`)
- [x] 1.4 移除 `SlaveDevice::apply_random_mutation*` 与相关旧路径(若仅服务于旧设备级变位);`cargo check -p modbussim-core` 通过

## 2. core 点位变位作用逻辑

- [x] 2.1 新增按 `data_type`+`endian` 从 `RegisterMap` 解码工程值 / 编码写回 1~2 寄存器的辅助(复用 register.rs 现有编解码)
- [x] 2.2 实现单点变位函数:输入 `RegisterDef`+`MutationConfig`+当前值+运行时方向,输出新值与新方向;bool 走翻转分支,数值按四模式计算并 clamp 到 data_type 值域
- [x] 2.3 实现 increment/decrement 三角波到边界反向;flip 两态切换 `≤(min+max)/2?max:min`;random 在 `[min,max]` 均匀取值
- [x] 2.4 core 单测:序列化往返、bool 仅翻转、Float32 编解码不破坏、各模式与三角波边界反向、clamp 值域;`cargo test -p modbussim-core` 通过

## 3. app 运行时状态与 tick 任务

- [x] 3.1 在 `state.rs`/`AppState` 增加运行时变位表(key=`connection_id,slave_id,register_type,address` → `dir`,`next_due`)与 `mutation_running` 标志
- [x] 3.2 实现单个长驻 tick 任务(基准 100ms):扫描启用点,`now>=next_due` 时调用 core 单点变位、写回 `register_map`、刷新 `next_due += period_ms`、更新方向;总开关关闭时不变位
- [x] 3.3 tick 任务在应用启动/AppState 初始化时拉起(由 `set_mutation_running` 控制实际是否执行变位)
- [x] 3.4 调度逻辑可单测的纯函数化(到期判定/方向推进),补 app 侧或 core 侧调度单测

## 4. app 命令面

- [x] 4.1 新增 `set_point_mutation(connection_id,slave_id,register_type,address,config)`:写入对应 `RegisterDef.mutation`,刷新运行时表
- [x] 4.2 新增 `clear_point_mutation(...)`:关闭该点变位并从运行时表移除
- [x] 4.3 新增 `list_point_mutations(connection_id,slave_id) -> Vec<{register_type,address,mode}>`
- [x] 4.4 新增 `set_mutation_running(running: bool)` 总开关
- [x] 4.5 移除 `random_mutate_registers` 命令及 `lib.rs` invoke_handler 注册;注册 4 个新命令;`cargo check -p modbussim-app` 通过

## 5. 前端 UI

- [x] 5.1 类型定义:`MutationMode`、`MutationConfig`、`PointMutationInfo`;i18n(zh-CN/en-US)新增变位模式/字段文案
- [x] 5.2 `RegisterTable.vue` 行内变位状态图标(启用指示 + 模式标识 ⇅/↑/↓/🎲),点击打开 popover 配置面板
- [x] 5.3 popover 配置:模式下拉 + period_ms + min/max/step,按 `data_type` 智能默认与字段显隐(bool 仅显示翻转);保存调用 `set_point_mutation`,清除调用 `clear_point_mutation`
- [x] 5.4 `MutationControl.vue` 改造为"全部启停"总开关,调用 `set_mutation_running`;移除旧类型复选/全局间隔与 `setTimeout` 调度;`Toolbar.vue` 接线调整
- [x] 5.5 每约 2 秒轮询 `list_point_mutations` 更新行内状态;变位值刷新复用现有 `useRegisterValues` 轮询

## 6. 集成验证

- [x] 6.1 `cargo check`/`cargo test` 全工作区通过;`vue-tsc --noEmit` 与前端 `npm run build` 通过
- [x] 6.2 交付说明:列出需用户手工验证的交互(行内配置变位、各模式效果、总开关、保存/重载项目后配置保留)
- [x] 6.3 `openspec-cn validate slave-point-mutation` 通过
