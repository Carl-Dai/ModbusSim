# 设计：auto-update

## 上下文

IEC60870-5-104-Simulator 已运行同构的自动更新机制多个版本，组件齐全：每 app 一份 `update.rs`（137 行）、共享 `UpdateDialog.vue`、`gen-update-manifest.mjs`（含单测）、release.yml 签名与 manifest 发布链路、`test-update-proxies.sh`。ModbusSim 与 104 的工程结构互为镜像（slave/master 双 Tauri app + shared-frontend），本变更本质是模板移植 + 改名。前置依赖：`deprecate-egui` 已合并。

## 目标 / 非目标

**目标：**
- 两个 app 启动后自动检查更新，发现新版弹窗提示，一键下载安装重启
- 国内网络可用：代理 endpoint 优先，GitHub 直连兜底，再失败轮询其余公共代理
- 更新包 minisign 签名校验，防篡改
- 检查行为克制：6 小时节流，"稍后提醒"对同版本静默 24 小时，手动检查无视两者

**非目标：**
- 不做静默后台自动安装（始终用户确认后才下载）
- 不做增量/差分更新
- 不为旧版本（无 updater 的 ≤ 当前版）用户提供推送通道
- 不改 104 的交互设计——照搬，不二次创新

## 决策

1. **复用 104 的 minisign 密钥对**。pubkey 是开发者级而非项目级，两仓库共用同一对 `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)` secrets，免去新密钥的生成与保管成本。替代方案（每项目独立密钥）安全收益微小，运维成本翻倍，否决。

2. **endpoint 顺序与 104 完全一致**：`gh.daichangyu.com`（自建代理）→ GitHub 直连 → `gh-proxy.com` → `gh.idayer.com` → `ghfast.top`。manifest 命名沿用 `latest-{slave,master}{-cnN,}.json`；仓库不同所以 URL 无冲突。`MANIFEST_VARIANTS` 顺序须与 `tauri.conf.json` endpoints 顺序同步维护（脚本内已有注释约定）。

3. **`gen-update-manifest.mjs` 移植时只改三处**：`REPO` 常量、资产名前缀匹配（`IEC104Slave_`/`IEC104Master_` → `ModbusSlave_`/`ModbusMaster_`）、保留其余逻辑（含 release API 404 重试、.sig 缺失即 fail-fast）。配套移植 `gen-update-manifest.test.mjs` 与 `test-update-proxies.sh`。

4. **`tauri.conf.json` version 同步为真实版本**。当前停在 `0.1.0`，而 updater 用它与 manifest version 比对，不修则永远"有更新"。本变更内一次性同步到当前 release 版本；后续由 `release` skill 的版本号巡检维护。

5. **状态持久化用 `tauri-plugin-store`**（`update_state.json`：last_check_at / snoozed_version / snoozed_until），与 104 相同；不引入数据库或自定义文件格式。

6. **前端接线模式照搬 104**：App.vue `onMounted` 延迟 2 秒静默检查（启动失败不打扰用户，仅 console.warn）；`provide('checkUpdate')` 供工具栏"检查更新"按钮强制检查（force=true 时网络错误要弹给用户）。UpdateDialog 放 `shared-frontend/components/`，两端共用。

## 风险 / 权衡

- [公共代理失效或被滥用限流] → 5 个 endpoint 互为冗余；`test-update-proxies.sh` 可定期手测；自建代理在首位
- [secrets 未配置导致 release 产物无 .sig] → manifest 脚本遇缺失 .sig 直接报错中断 publish-manifest job，不会发布坏 manifest
- [tauri.conf.json version 与 git tag 漂移] → release skill 发版时统一巡检 bump；manifest version 以 tag 为准
- [macOS 未签名公证应用更新后 Gatekeeper 拦截] → 与现状一致（用户已接受首次安装的绕过流程），updater 的 minisign 校验独立于 OS 签名
- [双 app 各持一份 update.rs 产生重复代码] → 接受；104 同样如此，137 行的重复成本低于抽共享 crate 的耦合成本

## 迁移计划

1. Rust 侧（插件 + update.rs + 配置）→ `cargo check` + `cargo test`（should_check / is_snoozed 单测）
2. 前端侧（UpdateDialog + 接线）→ `vue-tsc --noEmit` + `npm run build`
3. CI 侧（签名 env + publish-manifest job + 脚本）→ 脚本单测 `node --test`；首次真实验证在下一个 tag 发版时
4. 仓库 secrets 配置（用户手动操作，从 104 仓库复制两个 secrets）
5. 回滚：updater 配置错误不影响应用主功能（检查失败仅 console.warn），可单独 revert
