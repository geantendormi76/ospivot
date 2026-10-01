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
        self.blackboard.status = "PLANNED".to_string();
        self.blackboard.save(&self.blackboard_path)?;

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

        // Stage 4: Sync
        println!("\n>>> [STAGE 4: ASSETS SYNCHRONIZATION]");
        self.adapter.sync_boot_assets(&self.config.target.iso_path, &drive)?;
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
