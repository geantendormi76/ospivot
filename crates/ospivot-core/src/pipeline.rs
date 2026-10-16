use anyhow::{bail, Result};
use std::path::Path;
use crate::config::TransitionConfig;
use crate::blackboard::Blackboard;
use crate::platform::{CurrentPlatformAdapter, PlatformAdapter};

pub struct PipelineRunner {
    config: TransitionConfig,
    blackboard: Blackboard,
    blackboard_path: std::path::PathBuf,
    adapter: Box<dyn PlatformAdapter>,
}

impl PipelineRunner {
    pub fn new(config_path: &Path, blackboard_path: &Path) -> Result<Self> {
        let config = TransitionConfig::load_from_file(config_path)?;
        let blackboard = Blackboard::load(blackboard_path);
        let adapter: Box<dyn PlatformAdapter> = Box::new(CurrentPlatformAdapter::new());

        Ok(Self {
            config,
            blackboard,
            blackboard_path: blackboard_path.to_path_buf(),
            adapter,
        })
    }

    pub fn run_all(&mut self) -> Result<()> {
        println!("======================================================================");
        println!("🛡️ OSPIVOT UNIFIED ENGINE (RUST CROSS-PLATFORM PIPELINE)");
        println!("======================================================================");

        // Stage 1: Probe
        println!("\n>>> [STAGE 1: PROBE]");
        let disks = self.adapter.probe_disks()?;
        for d in &disks {
            println!("  [FACT] Disk {}: {} ({} GB, {})", d.number, d.name, d.total_size_gb, d.partition_style);
        }
        self.blackboard.disks = disks;
        self.blackboard.status = "PROBED".to_string();
        self.blackboard.save(&self.blackboard_path)?;

        // Stage 2: Plan
        println!("\n>>> [STAGE 2: PLAN]");
        if !self.config.target.iso_path.exists() {
            bail!("Configured ISO does not exist: {}", self.config.target.iso_path.display());
        }
        println!("  [PASS] Target ISO: {}", self.config.target.iso_path.display());
        println!("  [PLAN] Target Disk: {} (Will be formatted as {})", self.config.storage.target_disk_id, self.config.target.flavor);
        println!("  [PLAN] Scratch Disk: {} (Allocating {} MB FAT32)", self.config.storage.scratch_disk_id, self.config.storage.scratch_size_mb);

        // 目标卷大小校验: Windows 的 install.wim 常 > 4GB (FAT32 单文件上限)，
        // 必须在破坏性缩容前确认引导卷足以容纳解压后的全部资产。
        if self.config.target.os.eq_ignore_ascii_case("windows") {
            match self.adapter.estimate_iso_size_mb(&self.config.target.iso_path) {
                Ok(required) if required > 0 => {
                    println!("  [SIZE] Windows ISO 解压体积约 {} MB", required);
                    if self.config.storage.scratch_size_mb + 512 < required {
                        bail!("引导卷过小: Windows ISO 需要至少 {} MB, 而 scratch_size_mb = {}。请修改 configs/transition.toml 的 scratch_size_mb >= {} 后重跑。", required + 512, self.config.storage.scratch_size_mb, required + 512);
                    }
                    println!("  [PASS] 引导卷大小充足 ({} MB >= {} MB)", self.config.storage.scratch_size_mb, required);
                }
                Ok(_) => println!("  [SIZE] 无法估算 ISO 体积, 跳过大小校验"),
                Err(e) => println!("  [SIZE] 估算 ISO 体积失败 (跳过): {}", e),
            }
        }

        self.blackboard.status = "PLANNED".to_string();
        self.blackboard.save(&self.blackboard_path)?;

        // 阶段 2.5: 解析 ISO 实际源路径 (若 ISO 在将被缩容的介质盘上, 先预热拷贝到根盘)。
        let iso_source = self
            .adapter
            .resolve_iso_source(&self.config.target.iso_path, self.config.storage.scratch_disk_id)?;

        // Stage 3: Storage
        println!("\n>>> [STAGE 3: STORAGE PROVISIONING]");
        let drive = self.adapter.allocate_scratch_partition(
            self.config.storage.scratch_disk_id,
            self.config.storage.scratch_size_mb,
            &self.config.storage.boot_label,
        )?;
        println!("  [PASS] Boot partition ready at: {}", drive.display());
        self.blackboard.allocated_drive = Some(drive.to_string_lossy().to_string());
        self.blackboard.status = "STORAGE_PROVISIONED".to_string();
        self.blackboard.save(&self.blackboard_path)?;

        // Stage 4: Sync (使用解析后的 ISO 源路径, 避免介质盘卸载后找不到源)。
        println!("\n>>> [STAGE 4: ASSETS SYNCHRONIZATION]");
        self.adapter.sync_boot_assets(&iso_source, &drive)?;
        println!("  [PASS] Assets successfully synchronized.");
        self.blackboard.status = "ASSETS_SYNCED".to_string();
        self.blackboard.save(&self.blackboard_path)?;

        // Stage 5: Arm
        println!("\n>>> [STAGE 5: FIRMWARE BOOTLOADER VERIFICATION]");
        self.adapter.verify_uefi_bootloader(&drive)?;
        self.blackboard.status = "ARMED".to_string();
        self.blackboard.save(&self.blackboard_path)?;

        println!("\n======================================================================");
        println!("🎉 [PIPELINE SUCCESS] SYSTEM ARMED FOR ZERO-USB REBOOT");
        println!("======================================================================");
        println!("Blackboard saved to: {}", self.blackboard_path.display());
        println!("Reboot and tap F11/F12 to boot into {}", self.config.storage.boot_label);
        Ok(())
    }
}
