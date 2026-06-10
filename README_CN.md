<div align="center">

# 🔌 ModbusSim

**跨平台 Modbus 仿真工具 —— 从站与主站,一套桌面工具全包。**

[![Release](https://img.shields.io/github/v/release/Karl-Dai/ModbusSim?label=release&color=2ea043)](https://github.com/Karl-Dai/ModbusSim/releases)
[![Downloads](https://img.shields.io/github/downloads/Karl-Dai/ModbusSim/total?color=1f6feb)](https://github.com/Karl-Dai/ModbusSim/releases)
[![Stars](https://img.shields.io/github/stars/Karl-Dai/ModbusSim?color=e3b341)](https://github.com/Karl-Dai/ModbusSim/stargazers)
[![License: MIT](https://img.shields.io/badge/License-MIT-lightgrey.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20·%20macOS%20·%20Linux-informational)]()

基于 **Rust** · **Tauri 2** · **Vue 3** 构建

[English](README.md) · **中文**

![ModbusMaster 通过 TCP 轮询仿真从站](docs/screenshots/tut-3-master-data.png)

</div>

---

## 项目简介

测试 Modbus 集成往往需要接一台真实 PLC 或借一台主站设备。本项目把**通信两端都搬到你的桌面**:

- 🛰️ **从站与主站同仓** —— 模拟一台现场设备,或去驱动一台,无需任何外部硬件。
- 🔌 **五种传输,同一内核** —— TCP、TCP+TLS、RTU、ASCII、RTU-over-TCP,覆盖功能码 FC01–FC06 / FC15 / FC16。
- 📈 **内置数据仿真** —— 用固定值 / 随机 / 正弦 / 锯齿 / 三角 / 计数器 / CSV 回放驱动寄存器;支持 20,000+ 寄存器虚拟滚动。
- 🖥️ **原生桌面应用** —— Rust + Tauri 的小体积安装包,覆盖 Windows / macOS / Linux,内置应用内自动更新。
- 🌏 **中英双语界面** —— 完整 English / 简体中文,运行时即时切换。

## 目录

- [应用截图](#应用截图)
- [功能特性](#功能特性)
- [下载安装](#下载安装)
- [支持的功能码](#支持的功能码)
- [传输模式](#传输模式)
- [从源码构建](#从源码构建)
- [快速开始](#快速开始)
- [项目结构](#项目结构)
- [参与贡献](#参与贡献)
- [更新日志](#更新日志)
- [macOS 首次启动](#macos-首次启动)
- [许可证](#许可证)

## 应用截图

**从站 · 20,000 寄存器 + 实时随机变位**

ModbusSlave 在 `0.0.0.0:502` 上跑一个真实 Modbus TCP 服务器,挂两个从站设备。寄存器表格虚拟滚动浏览 20,000+ 保持寄存器;开启**随机变位**后数值原地跳动(橙色闪烁),右侧值解析面板同时按有符号 / 无符号 / 十六进制 / 二进制解码选中寄存器。

![从站:运行中的服务器、变位中的寄存器与值解析面板](docs/screenshots/tut-1-slave.png)

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
- **动态数据源** —— 模拟寄存器值变化:固定值、随机、正弦波、锯齿波、三角波、计数器、CSV 回放
- **通信日志** —— 实时 TX/RX 日志记录,支持搜索、方向/功能码过滤、CSV 导出
- **项目文件** —— 保存/加载完整配置为 `.modbusproj` 文件,方便多场景切换
- **串口支持** —— 自动检测系统串口,可配置波特率、数据位、停止位、校验

### 📡 ModbusMaster —— 主站工具

- **多传输模式** —— TCP、TCP+TLS、RTU(串口)、ASCII(串口)、RTU-over-TCP
- **Modbus TCP over TLS** —— TLS 1.2+ 加密传输,支持 PEM 和 PKCS#12 证书格式,支持自签名证书测试模式
- **扫描组** —— 按寄存器组配置周期性轮询,自定义轮询间隔,支持独立从站 ID 覆盖
- **设备发现** —— 从站 ID 扫描 (1–247)、寄存器地址扫描、发现设备后自动添加到扫描组
- **多格式数据视图** —— 无符号、有符号、十六进制、二进制、Float32 (AB CD / CD AB),虚拟滚动
- **写入操作** —— 支持写入单/多个线圈和寄存器 (FC05/06/15/16)
- **通信日志** —— TX/RX 日志,支持搜索/过滤(方向、功能码、文本),CSV 导出
- **自动重连** —— 可配置的指数退避重连策略(1s → 2s → 4s → … → 最大 30s)
- **项目文件** —— 保存/加载连接和扫描组配置
- **连接即扫描** —— 连接成功后自动提示扫描从站设备
- **应用内自动更新** —— 从 GitHub Releases 推送(签名验证、6 小时检查节流、"稍后" 24 小时不重提)

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

两个应用自 v0.16.0 起均支持从 GitHub Releases **自动更新**。macOS 用户首次启动需要[多做一步](#macos-首次启动)。

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

## 快速开始

四步跑通一次完整往返 —— 用主站驱动仿真从站,全程无需硬件。(截图为中文界面,随时可用 **中 / EN** 切换语言。)

### 1 · 从站 —— 新建服务器与寄存器

打开 **ModbusSlave**,点击**新建连接**:选 TCP、端口(如 `502`)与随机初始化 —— 服务器启动后自带一个设备,四种寄存器类型(线圈 / 离散输入 / 保持 / 输入寄存器)全部铺满。开启**随机变化**让数值动起来;点击任意一行,右侧值解析面板即时解码。

![从站:运行中的服务器、变位中的寄存器与值解析面板](docs/screenshots/tut-1-slave.png)

### 2 · 主站 —— 新建连接

打开 **ModbusMaster**,点击**新建连接**。默认值已指向本地从站:目标地址 `127.0.0.1`、端口 `502`、从站 ID `1`。需要加密就勾选**启用 TLS**。点**创建**,再点**连接**。

![新建连接对话框](docs/screenshots/tut-2-master-newconn.png)

### 3 · 主站 —— 扫描组填满数据表

添加扫描组(如保持寄存器 0–59、线圈 0–31)并启动轮询。连接树显示每个组的功能码与范围;表格随每次轮询刷新,展示从站返回的真实数据。

![主站轮询从站的数据表](docs/screenshots/tut-3-master-data.png)

### 4 · 看报文

展开底部**通信日志**:每一对 TX/RX 都被解码 —— 方向、功能码与可读详情。回到从站,变位后的数值在主站下一次轮询即刷新。也可从主站写回(FC05/06/15/16),确认改动落到从站。

![通信日志:解码后的帧](docs/screenshots/tut-4-master-log.png)

## 项目结构

```
ModbusSim/
├── crates/
│   ├── modbussim-core/        # 核心库:协议、传输、寄存器、日志
│   │   └── src/
│   │       ├── slave.rs       # 从站连接(TCP/RTU/ASCII/RtuOverTcp 派发)
│   │       ├── master.rs      # 主站连接,多传输模式支持
│   │       ├── frame.rs       # RTU/ASCII 帧编解码
│   │       ├── pdu.rs         # Modbus PDU 请求/响应解析
│   │       ├── transport.rs   # Transport 枚举、串口配置、TLS 配置、端口枚举
│   │       ├── mbap.rs        # MBAP 帧编解码(TLS 模式使用)
│   │       ├── tls_slave.rs   # TLS 加密 Modbus TCP 从站服务器
│   │       ├── tls_master.rs  # TLS 加密 Modbus TCP 主站客户端
│   │       ├── register.rs    # 寄存器类型、编码/解码
│   │       ├── data_source.rs # 动态数据源(正弦波、计数器等)
│   │       ├── reconnect.rs   # 重连策略(指数退避)
│   │       ├── error.rs       # 统一 ModbusError 枚举
│   │       ├── project.rs     # .modbusproj 文件保存/加载/迁移
│   │       └── log_collector.rs # 线程安全日志环形缓冲区
│   ├── modbussim-app/         # 从站 Tauri 应用 —— ModbusSlave
│   └── modbusmaster-app/      # 主站 Tauri 应用 —— ModbusMaster
├── frontend/                  # 从站 Vue 3 前端
├── master-frontend/           # 主站 Vue 3 前端
└── shared-frontend/           # 共享 Vue 组件、composables、i18n
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

## 许可证

[MIT](LICENSE)
