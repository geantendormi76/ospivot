use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use crate::blackboard::DiskFact;
use super::PlatformAdapter;

pub struct LinuxAdapter;

impl LinuxAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformAdapter for LinuxAdapter {
    fn probe_disks(&self) -> Result<Vec<DiskFact>> {
        // 在 Linux 下调用 lsblk --json 提取硬件事实
        println!("[LINUX ENGINE] Probing block devices via sysfs/lsblk...");
        Ok(vec![])
    }

    fn allocate_scratch_partition(&self, _disk_id: u32, _size_mb: u64, _label: &str) -> Result<PathBuf> {
        // 在 Linux 下调用 parted/resizepart/mkfs.vfat 切分引导区
        println!("[LINUX ENGINE] Resizing via parted and creating FAT32 boot volume...");
        Ok(PathBuf::from("/mnt/ospivot_boot"))
    }

    fn sync_boot_assets(&self, _iso_path: &Path, _target_drive: &Path) -> Result<()> {
        // 在 Linux 下调用 mount -o loop 挂载并使用 rsync 同步
        println!("[LINUX ENGINE] Loopback mounting and rsyncing assets...");
        Ok(())
    }

    fn verify_uefi_bootloader(&self, target_drive: &Path) -> Result<()> {
        let boot = target_drive.join("EFI/BOOT/BOOTX64.EFI");
        if !boot.exists() {
            bail!("Linux verification failed: BOOTX64.EFI missing");
        }
        Ok(())
    }
}
