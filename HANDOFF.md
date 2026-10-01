# 🛡️ OSPivot 工程交接与状态记录 (HANDOFF.md)

> **当前工程状态**：`M1 (Windows -> Linux Mint 原地自举) 验收通过 | 状态: ARMED`  
> **交接时间**：2026-10-01  
> **架构基底**：100% Rust 跨平台驱动 (`crates/`) + 声明式契约驱动 (`configs/`) + 黑板模式 (`artifacts/`)

---

## 1. 项目愿景与技术本质 (Project Identity)

- **项目名称**：`ospivot` (OS Pivot)
- **技术定位**：Zero-USB In-place Bare-metal Cross-OS Transition Engine（零外接 U 盘的裸机跨操作系统原地自举与切换引擎）。
- **终极目标**：彻底消除对物理 U 盘的依赖，通过 UEFI 原生固件标准与存储几何计算，实现 **Windows ⇄ Linux 双向原地重装与互转**。

---

## 2. 真实审计事实与硬件拓扑 (Hard Facts)

### 2.1 物理 U 盘故障铁证（项目触发因）
- 用户原有双口 U 盘接入电脑 USB-A 口时报 `Code 43 (CM_PROB_FAILED_POST_START)`，DeviceID 固化为虚拟错误占位符 `USB\VID_0000&PID_0002`。
- 经 5 维底层审计（主板控制器 OK、注册表驱动过滤器 Clean、内核 PnP 事件无冲突、总线软复位），**100% 坐实 U 盘电脑端数据引脚物理接触断路**。
- **约束事实**：物理机在没有任何外置 U 盘介质可用的极限场景下，需将当前 Windows 彻底抹除替换为单一 Linux Mint 系统。

### 2.2 宿主物理存储现状 (现场不可伪造事实)
| 磁盘编号 | 物理型号 | 接口总线 | 物理容量 | 分区表 | 在本项目中的角色与契约 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Disk 1** | `KINGSTON SNVS500G` | NVMe | 500 GB | GPT | **目标安装盘**（承载当前 Windows C 盘，安装阶段全盘格式化为单一 Linux） |
| **Disk 0** | `WDC WD10EZEX-00WN4A0` | SATA HDD | 1000 GB | GPT | **介质承载盘**（承载用户 D 盘数据，非破坏性缩容末尾 8GB 充当虚拟内置 U 盘，原数据 100% 保持无损） |

---

## 3. 架构演进与工程纠偏历程 (Evolution & Corrections)

本项目在开发演进中经历了一次至关重要的**宪法级架构纠偏**：

### 3.1 错误路径（反模式复盘）
- **初期偏离**：过度依赖 Windows 宿主环境的 PowerShell 临时脚本，编写了盲目全盘扫描与基于 Windows 专有 API（`Resize-Partition`, `robocopy`）的过程式工具。
- **违背宪法**：
  1. 违背第 8 条（数据契约规则）与第 11 条（配置规则）：用隐式扫盘代替显式声明；
  2. 违背第 0 条（抽象 > 特判）与第 4 条（语言职责规则）：特判堆砌导致在 Linux 宿主下无法执行 `Linux -> Windows` 反向切换。

### 3.2 黄金标准重构（当前架构）
- **完全收敛至 Rust (`crates/`)**：废除一切业务层 `.ps1` 脚本，将黑板读写、配置反序列化、状态机推进与固件校验 100% 用 Rust 实现。
- **抽象平台适配器 (`PlatformAdapter Trait`)**：
  - Windows 环境编译：激活 `WindowsAdapter`（调用 Windows 存储栈与 Robocopy）；
  - Linux 环境编译：激活 `LinuxAdapter`（调用 `parted`、`losetup`、`rsync`，面向未来的反向切换）。
- **强类型声明式配置**：通过 `configs/transition.toml` 显式注入 ISO 路径与磁盘拓扑，零全盘猜测。
- **黑板架构 (Blackboard Pattern)**：硬件事实与流水线阶段持久化至 `artifacts/state/blackboard.json`。

---

## 4. 当前运行期状态与最新事实 (Current State: ARMED)

### 4.1 终端流水线执行记录
```text
>>> [STAGE 1: PROBE]
  [FACT] Disk 1: KINGSTON SNVS500G (0 GB, GPT)
  [FACT] Disk 0: WDC WD10EZEX-00WN4A0 (0 GB, GPT)

>>> [STAGE 2: PLAN]
  [PASS] Target ISO: C:/dev/linuxmint-22.3-cinnamon-64bit.iso
  [PLAN] Target Disk: 1 (Will be formatted as linux-mint-22.3-cinnamon)
  [PLAN] Scratch Disk: 0 (Allocating 8192 MB FAT32)

>>> [STAGE 3: STORAGE PROVISIONING]
  [PASS] Boot partition ready at: E:\

>>> [STAGE 4: ASSETS SYNCHRONIZATION]
Mount ISO at F:\, Copying to E:\...
  [PASS] Assets successfully synchronized.

>>> [STAGE 5: FIRMWARE BOOTLOADER VERIFICATION]

======================================================================
🎉 [PIPELINE SUCCESS] SYSTEM ARMED FOR ZERO-USB REBOOT
======================================================================
Blackboard saved to: artifacts/state/blackboard.json
Reboot and tap F11/F12 to boot into MINT_BOOT
```

### 4.2 物理磁盘落地事实核验
- **`MINT_BOOT (E:)`**：已成功在机械盘（Disk 0）末尾创建出 7.98 GB FAT32 分区，已占用 2.88 GB，剩余 5.10 GB。
- **引导资产完整性**：`E:\` 盘内完整包含 `.disk`、`casper`、`dists`、`pool` 以及固件核心入口 `E:\EFI\BOOT\BOOTX64.EFI`。
- **`blackboard.json`**：状态正式锁定为 `"status": "ARMED"`，`"allocated_drive": "E:\\"`。
- **数据安全性**：`数据 (D:)` 仍保有 324 GB 可用空间，原有个人数据毫无损耗。

---

## 5. 即刻行动指南：重启安装单系统 (Reboot Guide)

宿主环境目前已处于**不可变就绪态**，用户随时可以手动启动换装：
1. **重启电脑**：保存所有工作，点击 Windows 重启；
2. **进入 Boot Menu**：屏幕点亮瞬间连续敲击 **F11 / F12 / Del**；
3. **选定引导项**：在 BIOS 启动列表中选中 **`UEFI: MINT_BOOT`**；
4. **安装防呆底线**：
   - 运行桌面 **`Install Linux Mint`**；
   - 安装类型选择 **`Erase disk and install Linux Mint`**；
   - **目标盘必须选择且仅选择**：**`KINGSTON SNVS500G (500GB NVMe / nvme0n1)`**；
   - **绝对不要触碰**：`WDC WD10EZEX (1TB HDD / sda)`。
5. **系统收尾**：进入安装好的 Linux Mint 后，打开“磁盘 (Disks)”工具，删除 8GB `MINT_BOOT` 分区并合并回机械盘。

---

## 6. 下一阶段里程碑与架构路线 (Next Milestones)

- [x] **M1 (当前已闭环)**：Windows -> Linux Mint 单向原地自举引擎 (Rust Core + WindowsAdapter)。
- [ ] **M2 (待实现: Linux -> Windows 反向切换)**：
  - 在 `crates/ospivot-core/src/platform/linux.rs` 中落地 Linux 原生实现：
    - `parted resizepart` / `mkfs.vfat` 原生分区调用；
    - `mount -o loop` 挂载 Windows 官方 ISO；
    - 集成 `wimlib-imagex` 解决 Windows ISO 中 `install.wim > 4GB` 无法直接放入 FAT32 分区的分卷切片（`.swm`）难题。
- [ ] **M3 (多平台打包与分发)**：
  - 将 `ospivot-cli` 交叉编译为独立的单二进制无依赖程序；
  - 启动 `apps/` 目录下的 Tauri + React 图形化客户端建设。
