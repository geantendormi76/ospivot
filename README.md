遵循《AI Engineering Constitution》的规范要求：
- **事实 > 猜测**：将本次真实实战验证闭环（U 盘硬件故障、无外接介质、NVMe + HDD 拓扑无损切分、UEFI 5 阶段状态机）固化为项目的核心背景与标准参考实现。
- **自动化优先**：提供 **PowerShell 一键写入命令**，直接将高质量、具备国际开源标准的 `README.md` 写入仓库根目录。

---

### 一、 交付文件规范说明

- **文件绝对路径**：`C:\dev\ospivot\README.md`
- **语言域**：Markdown
- **修改目的**：为项目构建全景开源自述文档，包含痛点背景、黑板架构、状态机拓扑、跨平台适配器原理与快速上手指南，便于直接发布至 GitHub。
- **影响范围**：覆盖根目录 `README.md`。

---

### 二、 PowerShell 一键写入命令

请在处于 `C:\dev\ospivot` 的终端中，直接复制并运行以下整段脚本：

```powershell
$ErrorActionPreference = "Stop"

@'
# 🛡️ OSPivot (`os-pivot`)

> **Zero-USB In-place Bare-metal Cross-OS Transition Engine.**  
> 无需外部 U 盘的裸机跨操作系统原地自举与切换引擎（Windows ⇄ Linux 双向支持）。

[![Rust](https://img.shields.io/badge/Language-Rust_2021-DEA584?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux-brightgreen?style=flat-square)](#)
[![Architecture](https://img.shields.io/badge/Architecture-Blackboard%20State%20Machine-purple?style=flat-square)](#)

---

## 📖 痛点与项目缘起 (Why OSPivot?)

在传统的系统重装与跨 OS 切换流程中，外部介质（如物理 U 盘、移动硬盘）往往被视为不可或缺的依赖。然而在真实工程实践中，用户经常遭遇：
1. **外部硬件失效**：U 盘物理金手指磨损、主控固件丢失（如常见的 `Code 43: Device Descriptor Request Failed`），导致无法制作物理启动盘；
2. **手头缺乏介质**：在远程物理机、机房独服或手头无可用外置 U 盘的紧急场景；
3. **跨系统单盘彻底置换**：想要彻底抹除当前宿主系统（如 Windows 单系统换为 Linux Mint / Ubuntu，或反向置换），但系统无法在运行状态下格式化自身根磁盘。

**OSPivot** 彻底颠覆了对外部介质的依赖。基于现代 **UEFI 原生引导规范** 与 **强类型存储几何计算**，OSPivot 直接在机箱内部现有的次级磁盘划分临时 FAT32 固件引导卷，在内存中完成自举接管，实现纯粹的**“裸机原地蜕变”**。

---

## ⚡ 核心工程哲学 (Core Philosophy)

OSPivot 严格遵循现代 AI 与系统软件工程宪法开发：

- **Zero-USB (零介质依赖)**：只利用物理机内置存储拓扑完成自举引导环境构建。
- **Declarative & Zero-Guesswork (声明式与零猜测)**：彻底废除盲目全盘扫描。所有环境意图由 `configs/transition.toml` 显式注入，严密契约校验。
- **Blackboard Architectural Pattern (黑板模式)**：探查到的不可伪造物理硬件事实与流水线阶段状态统一持久化于 `artifacts/state/blackboard.json`，支持单步调试与断点重试。
- **100% Pure Rust Core (内存安全与跨平台)**：底层调度、存储计算与固件验收全部收敛于 `crates/`，通过 `PlatformAdapter Trait` 抹平平台专有系统调用差异。

---

## 🏗️ 五阶段确定性状态机 (The 5-Stage Pipeline)

OSPivot 将跨系统切换抽象为严格的 5 个阶段状态机流转：

```
[ STAGE 1: PROBE ]
   │  通过系统总线底层接口提取磁盘物理事实（型号、容量、GPT/MBR 分区表），持久化写入黑板
   ▼
[ STAGE 2: PLAN ]
   │  加载 configs/transition.toml，比对硬件拓扑与 ISO 镜像物理存在，生成不可变执行计划
   ▼
[ STAGE 3: STORAGE PROVISIONING ]
   │  从非系统盘末尾安全执行非破坏性缩容（Non-destructive Shrink），分配 8GB FAT32 引导卷
   ▼
[ STAGE 4: ASSETS SYNCHRONIZATION ]
   │  内核级 Loopback 挂载 ISO 镜像，工业级流式克隆 Live 内核、SquashFS 与 EFI 引导树
   ▼
[ STAGE 5: FIRMWARE BOOTLOADER VERIFICATION ]
   │  固件钥匙校验：严格验证 \EFI\BOOT\BOOTX64.EFI 物理存在，锁定黑板状态为 "ARMED"
   ▼
[ 🎉 ARMED & REBOOT ]
   重启进入主板 BIOS 引导菜单 (F11/F12)，选择生成的本地 UEFI 启动介质，完成全盘格式化安装！
```

---

## 🚀 快速上手 (Quick Start)

### 1. 克隆仓库与准备
确保系统安装了 **Rust (1.75+)** 编译工具链：
```bash
git clone https://github.com/your-username/ospivot.git
cd ospivot
```

### 2. 声明转换意图 (`configs/transition.toml`)
编辑声明式配置文件，指定你的目标镜像与磁盘拓扑（以 Windows 转换为 Linux 为例）：

```toml
[source]
os = "windows"

[target]
os = "linux"
flavor = "linux-mint-22.3-cinnamon"
# 物理镜像绝对路径 (显式指定，拒绝盲目扫盘)
iso_path = "C:/dev/linuxmint-22.3-cinnamon-64bit.iso"

[storage]
# 最终要清空并安装新系统的磁盘编号 (如 500GB NVMe 固态)
target_disk_id = 1

# 借用划分临时 8GB 引导分区的物理磁盘编号 (如 1TB 机械硬盘，原数据 100% 保持无损)
scratch_disk_id = 0
scratch_size_mb = 8192
boot_label = "MINT_BOOT"
```

### 3. 一键编译并执行流水线
在任意终端（管理员权限）执行统一入口：

```bash
cargo run -p ospivot-cli
```

终端将清晰推进 5 个阶段，最终输出：
```text
======================================================================
🎉 [PIPELINE SUCCESS] SYSTEM ARMED FOR ZERO-USB REBOOT
======================================================================
Blackboard saved to: artifacts/state/blackboard.json
Reboot and tap F11/F12 to boot into MINT_BOOT
```

### 4. 重启安装
1. 重启电脑，屏幕亮起时连续按 **F11 / F12 / Del** 打开主板引导菜单；
2. 选择新建的 UEFI 卷标（如 `MINT_BOOT`）；
3. 进入桌面安装器，选择 **Erase disk** 并选中你的目标固态盘；
4. 安装完成后，在 Linux 中随时可将借用的 8GB 分区安全删除合并回原存储盘。

---

## 📂 仓库职责拓扑 (Repository Topology)

遵循 AI Engineering Monorepo 规范，严守领域职责边界：

```
ospivot/
├── crates/                    # 【Rust 系统内核层】负责底层能力与强类型跨平台抽象
│   ├── ospivot-core/          # 核心调度器、黑板模型与 PlatformAdapter 抽象 Trait
│   │   └── src/platform/
│   │       ├── mod.rs         # 跨平台接口 Trait (probe_disks, allocate, sync, verify)
│   │       ├── windows.rs     # #[cfg(target_os = "windows")] 实现 (Storage API / Robocopy)
│   │       └── linux.rs       # #[cfg(target_os = "linux")] 实现 (Parted / Rsync / Efibootmgr)
│   └── ospivot-cli/           # 统一跨平台命令行应用入口
│
├── configs/                   # 【声明式注入层】单向配置驱动
│   ├── transition.toml        # 用户转换声明
│   └── schemas/               # 严格的 JSON Schema 契约校验
│
├── artifacts/                 # 【产物与运行期黑板】
│   └── state/blackboard.json  # 运行时硬件与执行计划事实
│
├── apps/                      # 【未来 UI 壳工程】Tauri + React 桌面端骨架
├── docs/                      # 长期系统知识库与交接记录
└── tools/                     # 构建与开发自动化脚本
```

---

## 🗺️ 里程碑与路线图 (Roadmap)

- [x] **v0.1.0 (Dogfooding Milestone)**:
  - [x] 确立黑板持久化模型 (`artifacts/state/blackboard.json`)
  - [x] 实现 Windows 宿主环境下的安全非破坏性缩容与 FAT32 自动挂载
  - [x] 跨平台 `PlatformAdapter Trait` 接口抽象完成
  - [x] 实战验证通过：在双磁盘物理机无物理 U 盘状态下成功制备 Linux Mint 22.3 UEFI 引导环境
- [ ] **v0.2.0 (Bidirectional Linux -> Windows)**:
  - [ ] 完善 `crates/ospivot-core/src/platform/linux.rs` 原生系统调用
  - [ ] 集成 `wimlib` 处理 Windows 官方 ISO 中 `install.wim > 4GB` 的 FAT32 分卷自动切片
- [ ] **v0.3.0 (Tauri Desktop App)**:
  - [ ] 基于 `apps/` 打造开箱即用的跨平台一键切换图形桌面端

---

## 📄 开源许可证 (License)

本项目遵循 [MIT](LICENSE) OR [Apache-2.0](LICENSE) 双重开源许可证。
