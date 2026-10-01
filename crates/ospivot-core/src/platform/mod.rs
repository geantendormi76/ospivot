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
}

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsAdapter as CurrentPlatformAdapter;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxAdapter as CurrentPlatformAdapter;
