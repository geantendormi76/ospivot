use anyhow::Result;
use std::path::{Path, PathBuf};
use crate::blackboard::DiskFact;

pub trait PlatformAdapter: Send + Sync {
    /// 阶段 1: 探查硬件物理事实
    fn probe_disks(&self) -> Result<Vec<DiskFact>>;

    /// 阶段 3: 划分 FAT32 临时引导卷
    fn allocate_scratch_partition(&self, disk_id: u32, size_mb: u64, label: &str) -> Result<PathBuf>;

    /// 阶段 4: 挂载 ISO 并克隆系统文件到引导卷
    fn sync_boot_assets(&self, iso_path: &Path, target_drive: &Path) -> Result<()>;

    /// 阶段 5: 固件级 UEFI 引导契约终验
    fn verify_uefi_bootloader(&self, target_drive: &Path) -> Result<()>;

    /// 估算 ISO 解压后体积 (MB)。
    /// 用于 PLAN 阶段校验引导卷是否足以容纳目标系统 (Windows 的 install.wim 常 > 4GB)。
    /// 返回 0 表示"无法估算 / 跳过校验"。
    fn estimate_iso_size_mb(&self, iso_path: &Path) -> Result<u64> {
        let _ = iso_path;
        Ok(0)
    }

    /// 解析 ISO 实际源路径。
    ///
    /// 当 ISO 位于"将被缩容/卸载"的介质盘上时, 必须提前将其拷贝到持久化存储
    /// (如根盘), 否则卸载介质盘后会丢失源文件。
    /// `scratch_disk_id` 为本次将被缩容的磁盘号, 用于冲突检测。
    /// 默认实现认为 ISO 路径稳定, 直接返回原路径; Linux 覆写以处理介质盘冲突。
    fn resolve_iso_source(&self, iso_path: &Path, _scratch_disk_id: u32) -> Result<PathBuf> {
        Ok(iso_path.to_path_buf())
    }
}

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsAdapter as CurrentPlatformAdapter;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxAdapter as CurrentPlatformAdapter;
