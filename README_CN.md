<div align="center">

# 🔌 ModbusSim

**跨平台 Modbus 仿真工具 —— 从站与主站,一套桌面工具全包。**

[![Release](https://img.shields.io/github/v/release/Karl-Dai/ModbusSim?label=release&color=2ea043)](https://github.com/Karl-Dai/ModbusSim/releases)
[![Downloads](https://img.shields.io/github/downloads/Karl-Dai/ModbusSim/total?color=1f6feb&cacheSeconds=3600)](https://github.com/Karl-Dai/ModbusSim/releases)
[![Stars](https://img.shields.io/github/stars/Karl-Dai/ModbusSim?color=e3b341)](https://github.com/Karl-Dai/ModbusSim/stargazers)
[![License: MIT](https://img.shields.io/badge/License-MIT-lightgrey.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20·%20macOS%20·%20Linux-informational)]()

基于 **Rust** · **Tauri 2** · **Vue 3** 构建

[English](README.md) · **中文**

![ModbusMaster 通过 TCP 轮询仿真从站](docs/screenshots/tut-3-master-data.png)
<br>
<sub>从站寄存器仿真 → 扫描组轮询 → 随机变位 → 报文级日志</sub>

</div>

---

## 项目简介

测试 Modbus 集成往往需要接一台真实 PLC 或借一台主站设备。本项目把**通信两端都搬到你的桌面**:

- 🛰️ **从站与主站同仓** —— 模拟一台现场设备,或去驱动一台,无需任何外部硬件。
- 🔌 **五种传输,同一内核** —— TCP、TCP+TLS、RTU、ASCII、RTU-over-TCP,覆盖功能码 FC01–FC06 / FC15 / FC16。
- 📈 **内置数据仿真** —— 每个寄存器都能跑自己的数据源(固定值 / 随机 / 正弦 / 锯齿 / 三角 / 计数器 / CSV 回放)或独立周期的变位计划;支持 20,000+ 寄存器虚拟滚动。
- 🌐 **网络可达性** —— 主站的 TCP、TCP+TLS、RTU-over-TCP 连接可走 SOCKS5 代理,支持可选认证与代理侧域名解析。
- 🖥️ **原生桌面应用** —— Rust + Tauri 的小体积安装包,覆盖 Windows / macOS / Linux,内置应用内自动更新。
- 🌏 **中英双语界面** —— 完整 English / 简体中文,运行时即时切换。

## 目录

- [应用截图](#应用截图)
- [功能特性](#功能特性)
- [下载安装](#下载安装)
- [支持的功能码](#支持的功能码)
- [传输模式](#传输模式)
- [从源码构建](#从源码构建)
- [快速开始(教程)](#快速开始教程)
- [项目结构](#项目结构)
- [参与贡献](#参与贡献)
- [更新日志](#更新日志)
- [macOS 首次启动](#macos-首次启动)
- [匿名使用统计](#匿名使用统计)
- [许可证](#许可证)

## 应用截图

**从站 · 20,000 寄存器 + 实时随机变位**

ModbusSlave 在 `0.0.0.0:502` 上跑一个真实 Modbus TCP 服务器,挂两个从站设备。寄存器表格虚拟滚动浏览 20,000+ 保持寄存器;开启**随机变位**后数值原地跳动(橙色闪烁),右侧值解析面板同时按有符号 / 无符号 / 十六进制 / 二进制解码选中寄存器。

![从站:运行中的服务器、变位中的寄存器与值解析面板](docs/screenshots/tut-1-slave.png)

**主站 · 新建连接对话框**

一个对话框覆盖全部传输方式 —— TCP(可选 TLS)、RTU 串口、ASCII 串口、RTU-over-TCP —— 配置目标地址、端口、从站 ID 与超时。网络传输还支持**SOCKS5 代理**,可填可选用户名/密码认证并走代理侧域名解析,让跳板机或工业网关后面的目标也能直连。

![新建连接对话框](docs/screenshots/tut-2-master-newconn.png)

**主站 · 扫描组填满数据表**

添加扫描组(如保持寄存器 0–59、线圈 0–31),每个组可独立设置轮询间隔与从站 ID 覆盖。连接树显示每个组的功能码与范围;表格随每次轮询刷新,展示从站返回的数据,按无符号 / 有符号 / 十六进制 / 二进制 / Float32 解码。

![主站轮询从站的数据表](docs/screenshots/tut-3-master-data.png)

**主站 · 解码后的 TX/RX 通信日志**

底部日志面板记录每一对请求/响应 —— 方向、功能码与可读详情(`R 0 x60`、`60 regs`)并排显示,支持按方向 / 功能码 / 文本过滤,可导出 CSV。

![主站通信日志:解码后的帧](docs/screenshots/tut-4-master-log.png)

## 功能特性

### 🛰️ ModbusSlave —— 从站模拟器

- **多传输模式** —— TCP、TCP+TLS、RTU(串口)、ASCII(串口)、RTU-over-TCP
- **Modbus TCP over TLS** —— TLS 1.2+ 加密传输,支持 PEM 和 PKCS#12 证书格式,可选 mTLS 双向认证(验证客户端证书)
- **多设备支持** —— 在任意端口创建连接,每个连接支持多个从站设备
- **四种寄存器类型** —— 线圈 (FC01)、离散输入 (FC02)、保持寄存器 (FC03)、输入寄存器 (FC04)
- **完整协议支持** —— 读取 (FC01–04)、单点写入 (FC05/06)、多点写入 (FC15/16),支持 Modbus 异常码
- **寄存器表格** —— 地址搜索/过滤、行内值编辑、Ctrl/Shift 多选、虚拟滚动(支持 20,000+ 寄存器),多格式显示(Auto / U16 / I16 / Hex / Bin / Float32 四种字节序)
- **默认初始化** —— 新建从站默认铺满地址 0~20,000(四种寄存器类型),批量添加单次最多 50,000 条
- **值解析面板** —— 有符号/无符号/十六进制/二进制 (16 位)、Long/Float (32 位)、Double (64 位),四种字节序 (AB CD / CD AB / BA DC / DC BA)
- **逐点数据源** —— 每个寄存器都能跑自己的仿真数据源:固定值、随机、正弦波、锯齿波、三角波、计数器、CSV 序列回放;每个点位独立设置更新间隔与波形参数(幅值 / 频率 / 偏移 / 相位 / 波周期),配置随工程保存/加载完整往返
- **逐点周期变位** —— 任意寄存器可配置独立的变位计划:离散点翻转,模拟量在上下限之间按步长爬升;最小周期 100 ms,所有计划在单一 100 ms 后端 tick 上并发独立调度,设置随工程持久化
- **通信日志** —— 实时 TX/RX 日志记录,支持搜索、方向/功能码过滤、CSV 导出
- **项目文件** —— 保存/加载完整配置为 `.modbusproj` 文件,方便多场景切换
- **串口支持** —— 自动检测系统串口,可配置波特率、数据位、停止位、校验

### 📡 ModbusMaster —— 主站工具

- **多传输模式** —— TCP、TCP+TLS、RTU(串口)、ASCII(串口)、RTU-over-TCP
- **Modbus TCP over TLS** —— TLS 1.2+ 加密传输,支持 PEM 和 PKCS#12 证书格式,支持自签名证书测试模式
- **SOCKS5 代理** —— TCP、TCP+TLS、RTU-over-TCP 连接可走 SOCKS5 代理(IPv4/IPv6),支持可选用户名/密码认证与代理侧域名解析;密码不会出现在 Rust Debug 输出中,界面明确提示 RFC 1929 凭据与工程文件的安全边界
- **扫描组** —— 按寄存器组配置周期性轮询,自定义轮询间隔,支持独立从站 ID 覆盖
- **设备发现** —— 从站 ID 扫描 (1–247)、寄存器地址扫描、发现设备后自动添加到扫描组
- **多格式数据视图** —— 无符号、有符号、十六进制、二进制、Float32 (AB CD / CD AB),虚拟滚动
- **写入操作** —— 支持写入单/多个线圈和寄存器 (FC05/06/15/16)
- **通信日志** —— TX/RX 日志,支持搜索/过滤(方向、功能码、文本),CSV 导出
- **自动重连** —— 可配置的指数退避重连策略(1s → 2s → 4s → … → 最大 30s)
- **项目文件** —— 保存/加载连接和扫描组配置
- **连接即扫描** —— 连接成功后自动提示扫描从站设备
- **应用内自动更新** —— 更新包先在后台静默下载并验签,准备完成后提示立即安装、跳过该版本或下次启动安装(6 小时检查节流、"稍后" 24 小时不重提)

### 🧩 共享架构

- **统一错误系统** —— 结构化 `ModbusError`,分类错误类型(连接/协议/应用),序列化为 JSON 供前端解析
- **共享 Vue 组件** —— 通用 composables、类型定义与 i18n 通过 `shared-frontend` npm workspace 在两个 Tauri 应用间共享

## 下载安装

各平台预编译安装包均在 **[Releases 页面](https://github.com/Karl-Dai/ModbusSim/releases)**。

| 平台 | 安装包 |
|------|--------|
| Windows | `.msi` / `.exe`(NSIS) |
| macOS   | `.dmg`(Apple Silicon 与 Intel) |
| Linux   | `.AppImage` / `.deb` / `.rpm` |

两个应用自 v0.16.0 起均支持从 GitHub Releases **自动更新**。自 v0.17.2 起,更新包先在后台下载并验签,准备完成后再提示立即安装 / 跳过该版本 / 下次启动安装。macOS 用户首次启动需要[多做一步](#macos-首次启动)。

> **提示**:自 v0.16 起,egui 原生版(文件名含 `-egui-` 后缀)停止维护与发布,请迁移到上表的 Tauri 版安装包。

### 国内镜像 (China mirror)

中国大陆用户访问 GitHub Releases 可能不稳定,推荐通过镜像直接下载安装包:

- <https://ghfast.top/https://github.com/Karl-Dai/ModbusSim/releases/latest>

自 v0.16.0 起,应用内更新会自动通过多个反代回退,无需手动处理。但**首次从 0.16 之前的旧版升级**时,旧版二进制根本没有应用内更新器,请按上面镜像链接手动下载新版安装一次,后续更新即可自动通过 proxy。

## 支持的功能码

| 功能码 | 功能 | 从站(服务器) | 主站(客户端) |
|--------|------|:-:|:-:|
| FC01 | 读线圈 | 读取 | 读取/轮询 |
| FC02 | 读离散输入 | 读取 | 读取/轮询 |
| FC03 | 读保持寄存器 | 读取 | 读取/轮询 |
| FC04 | 读输入寄存器 | 读取 | 读取/轮询 |
| FC05 | 写单个线圈 | 写入 | 写入 |
| FC06 | 写单个寄存器 | 写入 | 写入 |
| FC15 | 写多个线圈 | 写入 | 写入 |
| FC16 | 写多个寄存器 | 写入 | 写入 |

## 传输模式

| 模式 | 传输层 | 帧格式 | 使用场景 |
|------|--------|--------|----------|
| TCP | TCP/IP 套接字 | MBAP 头 | 标准 Modbus TCP |
| TCP+TLS | TLS over TCP | MBAP 头 | 安全 Modbus TCP(TLS 1.2+) |
| RTU | 串口 | 从站 ID + CRC-16 | RS-485/RS-232 设备 |
| ASCII | 串口 | `:` + 十六进制 + LRC + CRLF | 传统串口设备 |
| RTU-over-TCP | TCP/IP 套接字 | 从站 ID + CRC-16 | 工业网关 |

## 从源码构建

### 环境要求

- [Rust](https://rustup.rs/) 1.77+
- [Node.js](https://nodejs.org/) 18+
- [Tauri CLI](https://tauri.app/) —— `cargo install tauri-cli`

### 步骤

```bash
# 安装前端依赖(npm workspaces —— 在仓库根目录执行一次即可)
npm install

# 启动从站
cd crates/modbussim-app && cargo tauri dev

# 启动主站
cd crates/modbusmaster-app && cargo tauri dev
```

### 构建安装包

```bash
cd crates/modbussim-app && cargo tauri build
cd crates/modbusmaster-app && cargo tauri build
```

### 运行测试

```bash
cargo test --workspace
```

## 快速开始(教程)

六步跑通一次完整往返 —— 用主站驱动仿真从站,全程无需硬件。(截图为中文界面,随时可用 **中 / EN** 切换语言。)

> **先安装:** 从 [Releases 页面](#下载安装)下载对应平台安装包,或从源码运行(`cargo tauri dev`)。**同时**打开 `ModbusSlave` 与 `ModbusMaster` 两个应用。

### 第 1 步 · 从站 —— 新建服务器与寄存器

打开 **ModbusSlave**,点击**新建连接**:选 TCP、端口(如 `502`)与随机初始化 —— 服务器启动后自带一个设备,四种寄存器类型(线圈 / 离散输入 / 保持 / 输入寄存器)全部铺满。开启**随机变化**让数值动起来;点击任意一行,右侧值解析面板即时解码。

![从站:运行中的服务器、变位中的寄存器与值解析面板](docs/screenshots/tut-1-slave.png)

**小技巧 · 默认初始化与批量添加**:新建从站默认铺满地址 0~20,000(四种寄存器类型),批量添加单次最多 50,000 条。

### 第 2 步 · 主站 —— 新建连接

打开 **ModbusMaster**,点击**新建连接**。默认值已指向本地从站:目标地址 `127.0.0.1`、端口 `502`、从站 ID `1`。需要加密就勾选**启用 TLS**。点**创建**,再点**连接**。

![新建连接对话框](docs/screenshots/tut-2-master-newconn.png)

**小技巧 · SOCKS5 代理**:TCP、TCP+TLS、RTU-over-TCP 均可勾选**使用 SOCKS5 代理**,通过代理连接目标,支持可选用户名/密码认证与代理侧域名解析 —— 目标在跳板机或工业网关后面时尤其好用。

### 第 3 步 · 主站 —— 扫描组填满数据表

添加扫描组(如保持寄存器 0–59、线圈 0–31)并启动轮询。连接树显示每个组的功能码与范围;表格随每次轮询刷新,展示从站返回的真实数据。

![主站轮询从站的数据表](docs/screenshots/tut-3-master-data.png)

### 第 4 步 · 从站 —— 数据源与变位驱动数值

回到从站,让寄存器动起来,主站下一次轮询即刷新:

- **右键 → 数据源** —— 为任意寄存器选择固定值、随机、正弦波、锯齿波、三角波、计数器或 CSV 序列回放,各自独立设置更新间隔与波形参数。
- **右键 → 周期变位** —— 离散点翻转,模拟量在上下限之间按步长爬升;最小周期 100 ms,每个点位独立调度。
- 两项配置都随 `.modbusproj` 工程文件持久化,保存的场景重新加载后原样复现。

### 第 5 步 · 主站 —— 写回

在主站用**写入**下发 FC05/06/15/16 —— 单个/多个线圈与寄存器。在从站表格(和通信日志)里确认改动在下一次轮询前已落到从站。

### 第 6 步 · 看报文 —— 解码后的帧

展开底部**通信日志**:每一对 TX/RX 都被解码 —— 方向、功能码与可读详情(`R 0 x60`、`60 regs`),支持按方向 / 功能码 / 文本过滤,可导出 CSV;主站的**自动重连**过程也全程留痕。

![通信日志:解码后的帧](docs/screenshots/tut-4-master-log.png)

六步走完 —— 服务器、寄存器、轮询、仿真、写回与报文级检查,全程都在你的桌面上完成。

## 项目结构

```
ModbusSim/
├── crates/
│   ├── modbussim-core/            # 核心库:协议、传输、寄存器、日志
│   │   └── src/
│   │       ├── slave.rs / master.rs           # 从站/主站连接(TCP/RTU/ASCII/RtuOverTcp 派发)
│   │       ├── frame.rs / pdu.rs / parse.rs   # RTU/ASCII 帧编解码、Modbus PDU 解析
│   │       ├── mbap.rs                        # MBAP 帧编解码(TLS 模式使用)
│   │       ├── tls_slave.rs / tls_master.rs   # TLS 加密 Modbus TCP 从站服务器 / 主站客户端
│   │       ├── ascii_slave.rs / ascii_master.rs       # ASCII 串口传输
│   │       ├── rtu_slave.rs / rtu_master.rs           # RTU 串口传输
│   │       ├── rtu_tcp_slave.rs / rtu_tcp_master.rs   # RTU-over-TCP 传输
│   │       ├── socks5.rs                      # 主站网络传输的 SOCKS5 CONNECT 代理
│   │       ├── transport.rs                   # Transport 枚举、串口配置、TLS 配置、端口枚举
│   │       ├── register.rs                    # 寄存器类型、编码/解码(含 32 位宽值)
│   │       ├── data_source.rs                 # 逐点数据源(正弦波、计数器、CSV 回放等)
│   │       ├── mutation.rs                    # 逐点周期变位计划
│   │       ├── reconnect.rs                   # 重连策略(指数退避)
│   │       ├── error.rs                       # 统一 ModbusError 枚举
│   │       ├── project.rs                     # .modbusproj 文件保存/加载/迁移
│   │       ├── config.rs                      # 连接与扫描组配置类型
│   │       └── log_collector.rs / log_entry.rs / log_helpers.rs  # 线程安全日志环形缓冲区
│   ├── modbussim-app/             # 从站 Tauri 应用 —— ModbusSlave
│   └── modbusmaster-app/          # 主站 Tauri 应用 —— ModbusMaster
├── frontend/                      # 从站 Vue 3 前端
├── master-frontend/               # 主站 Vue 3 前端
└── shared-frontend/               # 共享 Vue 组件、composables、i18n
```

| 层 | 技术栈 |
|----|--------|
| 后端 | Rust、Tokio(异步运行时)、[tokio-modbus](https://github.com/slowtec/tokio-modbus)、[tokio-serial](https://github.com/berkowski/tokio-serial)、[native-tls](https://crates.io/crates/native-tls)(macOS Security.framework / Linux OpenSSL / Windows SChannel)、[serialport](https://crates.io/crates/serialport) |
| 前端 | Vue 3、TypeScript、Vite、[@tanstack/vue-virtual](https://tanstack.com/virtual) |
| 桌面端 | Tauri 2 |

## 参与贡献

欢迎提交 Issue 与 Pull Request。提交代码改动前,请确保 `cargo test --workspace` 通过(并完成前端类型检查)。

## 更新日志

最新变更请参见 [CHANGELOG.md](CHANGELOG.md) 或 [Releases 页面](https://github.com/Karl-Dai/ModbusSim/releases)。

从 v0.16.0 起,两个应用在启动时自动检测 GitHub Releases,发现新版本会弹窗提示安装。0.16 之前版本的用户需要手动升级一次。

## macOS 首次启动

应用未做 Apple 公证(Notarization)。首次双击 `.app` 时,macOS 会弹窗 *"未打开 ModbusSlave / ModbusMaster —— Apple 无法验证…"*,只提供 *完成* 与 *移到废纸篓* 两个按钮。这是 macOS 15 (Sequoia) 起对 ad-hoc 签名应用的标准拦截,**不是软件损坏**。

<details>
<summary><b>放行步骤(任选其一)</b></summary>

**1. 图形界面**

- 双击 `.app`,出现拦截弹窗,点 *完成*。
- 打开 *系统设置 → 隐私与安全性*,滚到底部。
- 看到 *"已阻止 ModbusSlave 的使用…"*,点 *仍要打开* 并输入密码。
- 弹窗变为 *打开*,点击即可,以后双击直接启动。

**2. 终端一行命令**

```bash
xattr -dr com.apple.quarantine "/Applications/ModbusSlave.app"
xattr -dr com.apple.quarantine "/Applications/ModbusMaster.app"
```

清掉隔离标记,macOS 不再拦截。

</details>

## 匿名使用统计

ModbusSim 启动时通过 [Aptabase](https://aptabase.com) 发送一个匿名的 `app_started` 事件,便于作者了解装机量、活跃度与版本/系统分布。它**不采集任何个人数据**——只有应用版本、操作系统、语言和由 IP 现场推算的大致国家(IP 本身从不存储)。可随时在工具栏的 ⓘ「关于」气泡里关闭。

## 许可证

[MIT](LICENSE)
