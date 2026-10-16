//! =============================================================================
//! LinuxAdapter —— Linux → Windows 反向原地自举引擎 (M2 里程碑)
//! =============================================================================
//!
//! 运行平台: Linux (当前宿主为 Linux Mint 22.3 Cinnamon)
//! 目标系统: Windows 11 (25H2 官方 ISO)
//!
//! 与 WindowsAdapter (Windows→Linux) 对称。核心差异与 Windows 专属约束:
//!   1. install.wim 通常 > 4GB，超出 FAT32 单文件上限，必须用 wimlib-imagex
//!      拆分为 install.swm1/2/3… (Windows Setup 会自动识别 .swm 分卷)。
//!   2. Windows ISO 解压后约 8GB+，引导卷必须足够大 (默认放大至 16GB)。
//!   3. 引导契约仍是 `\EFI\BOOT\BOOTX64.EFI` (UEFI 无名字回弹加载项)。
//!
//! 安全前提 (必须满足，否则流程会提前 bail):
//!   - 以 root 运行 (`sudo cargo run -p ospivot-cli`) 或具备免密 sudo。
//!   - 介质盘 (scratch_disk) 上的目标分区须为非 EFI/非根的数据分区。
//!   - ntfsresize/resize2fs 会非破坏性缩容，但会先校验文件系统干净。
//! =============================================================================

use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::blackboard::DiskFact;
use super::PlatformAdapter;

/// FAT32 单文件硬上限 (4 GiB - 1 byte)。超过此大小的文件无法放入 FAT32。
#[allow(dead_code)]
const FAT32_MAX_FILE_BYTES: u64 = 4_294_967_295;

/// 固定引导卷挂载点。
const BOOT_MOUNT: &str = "/mnt/ospivot_boot";
/// 固定 ISO loopback 挂载点。
const ISO_MOUNT: &str = "/mnt/ospivot_iso";

pub struct LinuxAdapter;

// -----------------------------------------------------------------------------
// 底层命令执行与硬件探查辅助 (纯实现细节，不属于对外 trait)
// -----------------------------------------------------------------------------
impl LinuxAdapter {
    pub fn new() -> Self {
        Self
    }

    /// 执行只读命令 (lsblk / 7z / parted print 等), 无需 root。
    fn run(args: &[&str]) -> Result<String> {
        let output = Command::new(args[0])
            .args(&args[1..])
            .output()
            .with_context(|| format!("Failed to spawn: {}", args.join(" ")))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            bail!("Command failed ({}): {}", args.join(" "), err.trim());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// 执行需 root 的设备级写命令 (mount/ntfsresize/mkfs/parted mkpart/efibootmgr 等)。
    ///
    /// 认证模式 (优先级从高到低):
    ///   1. 已设置环境变量 `OSPIVOT_SUDO_PASS` → 以 `-S` 方式从 stdin 非交互读取 (自动化/CI)。
    ///   2. 当前已是 root → 直接执行。
    ///   3. 否则前缀 `sudo` (依赖免密 sudo 配置或交互式输入密码)。
    fn run_root(args: &[&str]) -> Result<String> {
        // 自动化模式: 通过 OSPIVOT_SUDO_PASS 以 -S 从 stdin 读取密码。
        if let Ok(pass) = std::env::var("OSPIVOT_SUDO_PASS") {
            let mut child = Command::new("sudo")
                .arg("-S")
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .context("Failed to spawn sudo -S")?;
            {
                let mut stdin = child.stdin.take().context("no stdin")?;
                use std::io::Write;
                stdin.write_all(format!("{}\n", pass).as_bytes())?;
            }
            let output = child.wait_with_output().context("sudo -S wait failed")?;
            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                bail!("Command failed: {}", err.trim());
            }
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }

        // 默认模式: 已是 root 或配置了免密 sudo。
        let mut full: Vec<&str> = Vec::with_capacity(args.len() + 1);
        if std::process::id() != 0 {
            full.push("sudo");
        }
        full.extend_from_slice(args);
        let output = Command::new(&full[0])
            .args(&full[1..])
            .output()
            .with_context(|| format!("Failed to spawn: {}", full.join(" ")))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            bail!("Command failed: {}", err.trim());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// 解析 `lsblk --json`，返回所有物理磁盘 (TYPE=="disk)。
    fn lsblk_disks() -> Result<Vec<Value>> {
        let raw = Self::run(&[
            "lsblk", "--json", "--bytes",
            "-o", "NAME,SIZE,TYPE,PARTTYPE,FSTYPE,MOUNTPOINT",
        ])
        .context("lsblk failed")?;
        let json: Value = serde_json::from_str(&raw)
            .with_context(|| format!("Failed to parse lsblk JSON: {}", &raw[..200.min(raw.len())]))?;
        let mut arr: Vec<Value> = json["blockdevices"]
            .as_array()
            .cloned()
            .context("lsblk --json returned no blockdevices array")?;
        // 严格白名单：只保留物理磁盘，彻底排除 loop 设备及内存盘
        arr.retain(|d| {
            let t = d["type"].as_str().unwrap_or("");
            let name = d["name"].as_str().unwrap_or("");
            t == "disk" && !name.starts_with("loop") && !name.starts_with("zram")
        });

        // 按设备名升序编号, 保证 0/1/2… 在不同启动间稳定可复现。
        arr.sort_by(|a, b| {
            a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or(""))
        });
        Ok(arr)
    }

    /// 判断 GUID 是否为 EFI System Partition (c12a7328-...)。
    fn is_esp(parttype: &str) -> bool {
        parttype == "c12a7328-f81f-11d2-ba4b-00a0c93ec93b"
    }

    /// 判断 GUID 是否为 BIOS GRUB 保留区。
    fn is_bios_grub(parttype: &str) -> bool {
        parttype == "21686148-6449-6e63-74-64-6576-362633616265"
    }

    /// 从 lsblk 磁盘数组提取 DiskFact (阶段 1)。
    fn disk_facts_from_lsblk(disks: &[Value]) -> Vec<DiskFact> {
        let mut facts = Vec::new();
        for (idx, d) in disks.iter().enumerate() {
            let name = d["name"].as_str().unwrap_or("unknown").to_string();
            let total_bytes = d["size"].as_u64().unwrap_or(0);
            let total_size_gb = (total_bytes as f64 / 1_073_741_824.0 * 100.0).round() / 100.0;
            facts.push(DiskFact {
                number: idx as u32,
                name: format!("{} ({} GB)", name, total_size_gb),
                total_size_gb,
                partition_style: Self::infer_table_style(d),
            });
        }
        facts
    }

    /// 启发式推断分区表风格 (GPT / MBR / UNKNOWN)。
    fn infer_table_style(d: &Value) -> String {
        if let Some(parts) = d["children"].as_array() {
            for p in parts {
                let pt = p["parttype"].as_str().unwrap_or("");
                if pt.contains('-') && pt.len() >= 32 {
                    return "GPT".to_string();
                }
                if pt.starts_with("0x") {
                    return "MBR".to_string();
                }
            }
        }
        "GPT".to_string()
    }

    /// 获取某物理磁盘上所有分区几何 (来自 `parted unit MiB print free`)。
    /// 返回 (分区号, 起始MiB, 体积MiB, flags 字符串)。
    fn parted_geometry(dev: &str) -> Result<Vec<(u32, u64, u64, String)>> {
        // parted 读取/写入原始设备节点, 需 root。
        let out = Self::run_root(&["parted", "-s", dev, "unit", "MiB", "print", "free"])
            .context("parted print free failed")?;
        let mut parts = Vec::new();
        for line in out.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 3 {
                continue;
            }
            // 分区行以编号开头；Free Space 行以 "Free" 开头。
            let num: u32 = match f[0].parse() {
                Ok(n) => n,
                Err(_) => continue,
            };
            let start: u64 = f[1].trim_end_matches("MiB").parse().unwrap_or(0);
            let size: u64 = f[2].trim_end_matches("MiB").parse().unwrap_or(0);
            let flags = if f.len() >= 6 { f[5..].join(" ") } else { String::new() };
            parts.push((num, start, size, flags));
        }
        Ok(parts)
    }

    /// 从设备名推导分区号 (sda3 -> 3; nvme0n1p2 -> 2)。
    fn part_number(disk_name: &str, part_name: &str) -> Option<u32> {
        let suffix = if part_name.starts_with(disk_name) {
            &part_name[disk_name.len()..]
        } else {
            part_name
        };
        suffix.trim_start_matches('p').parse::<u32>().ok()
    }

    /// 从 lsblk 分区数组按分区号建立 fstype / mountpoint / parttype 索引。
    fn partition_meta(dev_name: &str, children: &[Value]) -> std::collections::HashMap<u32, Value> {
        let mut map = std::collections::HashMap::new();
        for c in children {
            if c["type"].as_str() == Some("part") {
                if let Some(num) = Self::part_number(dev_name, c["name"].as_str().unwrap_or("")) {
                    map.insert(num, c.clone());
                }
            }
        }
        map
    }

    /// 估算 ISO 解压后体积 (MB)，用于引导卷大小校验。返回 0 表示"无法估算"。
    fn iso_extracted_size_mb(iso: &Path) -> Result<u64> {
        let iso_str = iso.to_str().context("invalid iso path")?;
        // 汇总 7z -slt 中每个文件的解压后 Size, 得到引导卷所需最小容量。
        let out = Self::run(&["7z", "l", "-slt", iso_str]).context("7z 读取 ISO 清单失败")?;
        let mut total: u64 = 0;
        for line in out.lines() {
            if let Some(v) = line.trim().strip_prefix("Size =") {
                let v = v.trim();
                if let Ok(n) = v.parse::<u64>() {
                    total += n;
                }
            }
        }
        Ok(total / 1024 / 1024)
    }

    /// 递归扫描 lsblk 树, 返回与路径前缀匹配的最长挂载点。
    fn find_mp(node: &Value, path_str: &str, best: &mut String) {
        if let Some(mp) = node["mountpoint"].as_str() {
            if !mp.is_empty() && path_str.starts_with(mp) && mp.len() > best.len() {
                *best = mp.to_string();
            }
        }
        if let Some(children) = node["children"].as_array() {
            for c in children {
                Self::find_mp(c, path_str, best);
            }
        }
    }

    /// 查找给定路径所在挂载点 (所有 lsblk 挂载点中最长的前匹配)。
    fn longest_mountpoint(path: &Path) -> String {
        let path_str = path.to_string_lossy();
        let disks = match Self::lsblk_disks() {
            Ok(d) => d,
            Err(_) => return String::new(),
        };
        let mut best = String::new();
        for d in &disks {
            Self::find_mp(d, &path_str, &mut best);
        }
        best
    }

    /// 阶段 2.5: 若 ISO 位于将被缩容的介质盘上, 先拷贝到根盘临时文件,
    /// 确保后续卸载介质盘后仍能回环挂载。
    fn resolve_iso_source(&self, iso_path: &Path, scratch_disk_id: u32) -> Result<PathBuf> {
        let disks = Self::lsblk_disks()?;
        if (scratch_disk_id as usize) >= disks.len() {
            return Ok(iso_path.to_path_buf());
        }

        // 介质盘上的所有挂载点。
        let scratch_mounts: Vec<String> = disks[scratch_disk_id as usize]
            .get("children")
            .and_then(|c| c.as_array())
            .map(|cs| {
                cs.iter()
                    .filter_map(|p| p["mountpoint"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // ISO 所在挂载点。
        let iso_mount = Self::longest_mountpoint(iso_path);
        if iso_mount.is_empty() || !scratch_mounts.contains(&iso_mount) {
            return Ok(iso_path.to_path_buf());
        }

        // 冲突: ISO 在介质盘上。拷贝到根盘临时文件。
        let tmp = PathBuf::from("/tmp/ospivot_staged.iso");
        println!(
            "  [PRE-STAGE] ISO 位于将被缩容的介质盘 (挂载点 {}), 先拷贝到 {} 以防卸载后丢失",
            iso_mount,
            tmp.display()
        );
        Self::run_root(&[
            "cp", "-a", iso_path.to_str().unwrap(), tmp.to_str().unwrap(),
        ])?;
        Ok(tmp)
    }

    /// best-effort: 将引导卷注册为 UEFI NVRAM 启动项。
    fn register_efi_boot_entry(drive: &Path) {
        if !Path::new("/sys/firmware/efi").exists() {
            println!("  [EFIBOOTMGR] 非 UEFI 启动环境, 跳过 NVRAM 注册");
            return;
        }
        // 定位挂载了引导卷的分区设备名。
        let dev = Self::run(&["lsblk", "-no", "NAME", "-o", "MOUNTPOINT", drive.to_str().unwrap()])
            .unwrap_or_default();
        let dev_name = dev.lines().nth(1).unwrap_or("").trim();
        if dev_name.is_empty() {
            println!("  [EFIBOOTMGR] 无法识别引导分区设备, 跳过 (可在 F12 菜单手动选择)");
            return;
        }
        // 分区号 = 设备名末尾连续数字 (nvme0n1p4 -> 4, sda4 -> 4)。
        let part_num: String = dev_name
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        let dev_path = format!("/dev/{}", dev_name);
        let (label, path) = ("WIN_BOOT", r#"\EFI\BOOT\BOOTX64.EFI"#);
        println!(
            "  [EFIBOOTMGR] 尝试注册启动项: {} {} ({} -p {})",
            label, dev_path, dev_name, part_num
        );
        // 失败不影响结果 (用户仍可手动在启动菜单选择)。
        let _ = Self::run_root(&[
            "efibootmgr", "-c", "-L", label, "-l", path, "-d", &dev_path, "-p", &part_num,
        ]);
    }
}

// -----------------------------------------------------------------------------
// 对外 trait 实现 (流水线按此契约调度)
// -----------------------------------------------------------------------------
impl PlatformAdapter for LinuxAdapter {
    fn probe_disks(&self) -> Result<Vec<DiskFact>> {
        println!("[LINUX ENGINE] Probing block devices via lsblk...");
        let disks = Self::lsblk_disks()?;
        let facts = Self::disk_facts_from_lsblk(&disks);
        if facts.is_empty() {
            bail!("No physical disks detected by lsblk");
        }
        Ok(facts)
    }

    fn allocate_scratch_partition(
        &self,
        disk_id: u32,
        scratch_size_mb: u64,
        label: &str,
    ) -> Result<PathBuf> {

        // [幂等性保护] 如果引导卷已经存在，直接挂载并返回，跳过重复缩容
        if let Ok(blkid_out) = Self::run(&["blkid", "-L", label]) {
            let blkid_out = blkid_out.trim();
            if !blkid_out.is_empty() && blkid_out.starts_with("/dev/") {
                println!("  [SKIP] 引导卷 {} 已存在于 {}, 跳过缩容阶段", label, blkid_out);
                let mount_point = PathBuf::from(BOOT_MOUNT);
                std::fs::create_dir_all(&mount_point).unwrap_or_default();
                let _ = Self::run_root(&["mount", blkid_out, mount_point.to_str().unwrap()]);
                return Ok(mount_point);
            }
        }

        let disks = Self::lsblk_disks()?;
        if (disk_id as usize) >= disks.len() {
            bail!(
                "scratch_disk_id {} out of range (detected {} disk(s)). Re-check probe output.",
                disk_id,
                disks.len()
            );
        }
        let disk = &disks[disk_id as usize];
        let dev_name = disk["name"].as_str().context("disk missing name")?.to_string();
        let dev = format!("/dev/{}", dev_name);
        let _part_size_mb = disk["size"].as_u64().unwrap_or(0) / 1024;

        // 安全闸: 介质盘绝不能是承载根或 EFI 的磁盘。
        let mountpoints: Vec<String> = disk
            .get("children")
            .and_then(|c| c.as_array())
            .map(|cs| {
                cs.iter()
                    .filter_map(|p| p["mountpoint"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        for m in &mountpoints {
            if m == "/" || m == "/boot" || m == "/boot/efi" {
                bail!(
                    "介质盘 /dev/{} 承载系统分区 (mountpoint={})，缩容会导致宿主崩溃。请改用其他磁盘作为介质盘。",
                    dev_name, m
                );
            }
        }

        // 选择候选缩容分区: 跳过 ESP / bios_grub / 系统挂载点，取最大者。
        let geom = Self::parted_geometry(&dev)?;
        let children = disk.get("children").and_then(|c| c.as_array()).cloned().unwrap_or_default();
        let meta = Self::partition_meta(&dev_name, &children);

        let mut candidates: Vec<(u32, u64, u64, String)> = geom
            .into_iter()
            .filter(|(num, _, _, _)| {
                let m = meta.get(num);
                let esp = m.map(|x| Self::is_esp(&x["parttype"].as_str().unwrap_or(""))).unwrap_or(false);
                let bg = m.map(|x| Self::is_bios_grub(&x["parttype"].as_str().unwrap_or(""))).unwrap_or(false);
                let mp = m.and_then(|x| x["mountpoint"].as_str()).unwrap_or("");
                !esp && !bg && mp != "/" && mp != "/boot/efi" && mp != "/boot"
            })
            .collect();
        candidates.sort_by(|a, b| b.2.cmp(&a.2)); // 按体积降序

        let (part_num_ref, _part_start_ref, part_size_ref, _flags) = candidates.first()
            .context("未找到可缩容的数据分区。请确认介质盘含非 EFI 数据分区。")?;
        let part_num: u32 = *part_num_ref;
        let current_mb: u64 = *part_size_ref;
        let chosen = meta.get(&part_num).context("分区元数据缺失")?;
        let fstype = chosen["fstype"].as_str().unwrap_or("").to_string();
        let part_dev = format!("/dev/{}{}", dev_name, part_num);

        // 安全闸: 申请体积不得超出介质分区当前体积。
        if scratch_size_mb >= current_mb {
            bail!(
                "scratch_size_mb ({}) 不得大于介质分区当前体积 ({})。",
                scratch_size_mb,
                current_mb
            );
        }

        println!(
            "  [SHRINK] 目标分区: /dev/{}{} ({} {} 当前 {} MB)",
            dev_name, part_num, fstype, chosen["parttype"].as_str().unwrap_or("?"), current_mb
        );

        let new_part_mb = current_mb.saturating_sub(scratch_size_mb);
        if new_part_mb < 1024 {
            bail!("缩容后分区过小 (<1GB)，scratch_size_mb={} 过大。", scratch_size_mb);
        }
        let free_tail_mb = current_mb - new_part_mb;
        println!("  [SHRINK] 尾部将释放 {} MB 空间用于引导卷", free_tail_mb);

        // 1) 若分区已挂载，先卸载 (ntfsresize/resize2fs 要求未挂载)。
        let mp = chosen["mountpoint"].as_str().unwrap_or("");
        if !mp.is_empty() {
            println!("  [UMOUNT] 卸载已挂载的分区 {} -> {}", part_dev, mp);
            Self::run_root(&["umount", mp])
                .with_context(|| format!("无法卸载 {} (可能有进程占用)", mp))?;
        }

        // 2) 按文件系统类型缩容文件系统。
        match fstype.as_str() {
            "ntfs" => {
                let _new_bytes = new_part_mb * 1024 * 1024;
                println!(
                    "  [NTFSRESIZE] 非破坏性缩容 /dev/{}{} -> {} MB",
                    dev_name, part_num, new_part_mb
                );
                // ntfsresize -s 默认单位为字节, 须加 M 后缀。
                Self::run_root(&["ntfsresize", "-f", "-s", &format!("{}M", new_part_mb), &part_dev])
                    .context("ntfsresize 失败 (文件系统可能脏或有不可移动簇，已中止, 数据安全)")?;
            }
            "ext4" | "ext3" | "ext2" => {
                let new_blocks = (new_part_mb * 1024 * 1024 / 4096) as u64;
                println!(
                    "  [RESIZE2FS] 非破坏性缩容 /dev/{}{} -> {} 块",
                    dev_name, part_num, new_blocks
                );
                Self::run_root(&["resize2fs", "-f", &part_dev, &new_blocks.to_string()])
                    .context("resize2fs 失败, 已中止, 数据安全")?;
            }
            other => bail!(
                "介质分区文件系统 {} 暂不支持自动缩容 (仅支持 ntfs/ext*)。",
                other
            ),
        }

        // 3) 缩容分区边界，释放尾部空间 (parted resizepart)。
        println!(
            "  [PARTED] 重定义 /dev/{}{} 边界至 {} MB",
            dev_name, part_num, new_part_mb
        );
        // parted 在缩容时会强制警告，-s 模式下会直接报错退出。必须用 yes 管道强答。
        let parted_cmd = format!(
            "yes Yes | parted ---pretend-input-tty {} unit MiB resizepart {} {}",
            dev, part_num, new_part_mb
        );
        Self::run_root(&["sh", "-c", &parted_cmd])
            .context("parted resizepart 失败")?;

        // 4) 在释放的尾部新建 FAT32 引导分区，并打上 ESP 标志 (便于固件识别 + efibootmgr)。
        let new_part_num = part_num + 1;
        println!(
            "  [PARTED] 新建分区 /dev/{}{} (尾部 {} MB)",
            dev_name, new_part_num, free_tail_mb
        );
        Self::run_root(&[
            "parted", "-s", &dev, "unit", "MiB", "mkpart",
            "primary", "fat32",
            &new_part_mb.to_string(), "100%",
        ])
        .context("parted mkpart 失败")?;
        Self::run_root(&[
            "parted", "-s", &dev, "unit", "MiB", "set",
            &new_part_num.to_string(), "msftdata", "on",
        ])
        .context("parted set esp 失败")?;

        // 5) 刷新内核分区表 (避免设备节点过期)。
        let _ = Self::run_root(&["partprobe", &dev]);

        // 6) 格式化 FAT32。
        println!("  [MKFS.VFAT] 卷标 {}", label);
        Self::run_root(&[
            "mkfs.vfat", "-F", "32", "-n", label,
            &format!("/dev/{}{}", dev_name, new_part_num),
        ])
        .context("mkfs.vfat 失败")?;

        // 7) 挂载到固定引导点。
        let mount_point = PathBuf::from(BOOT_MOUNT);
        std::fs::create_dir_all(&mount_point)?;
        println!(
            "  [MOUNT] /dev/{}{} -> {}",
            dev_name, new_part_num, mount_point.display()
        );
        Self::run_root(&["mount", &format!("/dev/{}{}", dev_name, new_part_num), mount_point.to_str().unwrap()])
            .context("挂载引导分区失败")?;

        Ok(mount_point)
    }

    fn sync_boot_assets(&self, iso_path: &Path, target_drive: &Path) -> Result<()> {
        let iso = iso_path.to_str().context("invalid iso path")?;
        let dest = target_drive.to_str().context("invalid target drive path")?;
        std::fs::create_dir_all(ISO_MOUNT)?;
        
        println!("[LINUX ENGINE] Loopback mounting ISO (ro)...");
        Self::run_root(&["mount", "-o", "loop,ro", iso, ISO_MOUNT])
            .with_context(|| format!("ISO 挂载失败: {}", iso))?;

        let iso_wim = Path::new(ISO_MOUNT).join("sources").join("install.wim");
        if iso_wim.exists() {
            println!("[LINUX ENGINE] 识别为 Windows 镜像，执行分层克隆流水线...");
            
            println!("  [RSYNC] 克隆引导框架 (排除 install.wim)...");
            Self::run_root(&[
                "rsync", "-rt", "--delete", "--no-owner", "--no-group", "--no-perms",
                "--exclude=sources/install.wim",
                "--exclude=sources/install.esd",
                &format!("{}/", ISO_MOUNT),
                dest,
            ]).context("rsync 基础引导文件失败")?;

            let meta = std::fs::metadata(&iso_wim).context("读取 ISO 中 install.wim 元数据失败")?;
            let size = meta.len();
            let dest_sources = Path::new(dest).join("sources");
            std::fs::create_dir_all(&dest_sources)?;

            if Command::new("wimlib-imagex").arg("--version").output().is_err() {
                println!("  [WIM] 正在安装 wimtools...");
                let _ = Self::run_root(&["apt-get", "update"]);
                let _ = Self::run_root(&["apt-get", "install", "-y", "wimtools"]);
            }

            let chunk = "3800";
            let swm_target = dest_sources.join("install.swm");
            if size > FAT32_MAX_FILE_BYTES {
                println!(
                    "  [WIM] install.wim ({} MB) > 4GB (FAT32单文件硬上限)，正在源端切片分卷写入 {} (chunk {} MB)...",
                    size / 1024 / 1024,
                    swm_target.display(),
                    chunk
                );
            }

            Self::run_root(&[
                "wimlib-imagex",
                "split",
                iso_wim.to_str().unwrap(),
                swm_target.to_str().unwrap(),
                chunk,
                "--check",
            ]).context("wimlib-imagex split 切片执行失败")?;

            if !swm_target.exists() {
                bail!("分卷切片断言失败：未能检测到关键引导资产 {}", swm_target.display());
            }
            println!("  [WIM] ✅ 微软标准分卷镜像 install.swm 切片校验通过！");
        } else {
            println!("[LINUX ENGINE] 克隆全量资产到引导卷...");
            Self::run_root(&["rsync", "-rt", "--delete", "--no-owner", "--no-group", "--no-perms", &format!("{}/", ISO_MOUNT), dest])
                .context("rsync 资产同步失败")?;
        }

        let _ = Self::run_root(&["umount", ISO_MOUNT]);
        Ok(())
    }

    fn verify_uefi_bootloader(&self, target_drive: &Path) -> Result<()> {
        let boot = target_drive.join("EFI").join("BOOT").join("BOOTX64.EFI");
        if !boot.exists() {
            bail!("UEFI bootloader not found at {}", boot.display());
        }
        println!("[LINUX ENGINE] BOOTX64.EFI verified at {}", boot.display());

        // 可选: 注册 NVRAM 引导项，使其出现在 F12 菜单 (best-effort, 失败不中断)。
        Self::register_efi_boot_entry(target_drive);
        Ok(())
    }

    fn estimate_iso_size_mb(&self, iso_path: &Path) -> Result<u64> {
        LinuxAdapter::iso_extracted_size_mb(iso_path)
    }

    fn resolve_iso_source(&self, iso_path: &Path, scratch_disk_id: u32) -> Result<PathBuf> {
        LinuxAdapter::resolve_iso_source(self, iso_path, scratch_disk_id)
    }
}
