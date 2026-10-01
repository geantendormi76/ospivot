use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::blackboard::DiskFact;
use super::PlatformAdapter;

pub struct WindowsAdapter;

impl WindowsAdapter {
    pub fn new() -> Self {
        Self
    }

    fn run_powershell_raw(&self, cmd: &str) -> Result<String> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", cmd])
            .output()
            .context("Failed to spawn powershell host")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            bail!("Host command failed: {}", err);
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

impl PlatformAdapter for WindowsAdapter {
    fn probe_disks(&self) -> Result<Vec<DiskFact>> {
        let cmd = "Get-Disk | Select-Object Number, FriendlyName, TotalSize, PartitionStyle | ConvertTo-Json -Depth 3";
        let raw_json = self.run_powershell_raw(cmd)?;

        if raw_json.is_empty() {
            return Ok(vec![]);
        }

        let parsed: serde_json::Value = serde_json::from_str(&raw_json)
            .with_context(|| format!("Failed to parse disk JSON: {}", raw_json))?;

        let items = match parsed {
            serde_json::Value::Array(arr) => arr,
            serde_json::Value::Object(_) => vec![parsed],
            _ => bail!("Unexpected JSON root structure from Get-Disk"),
        };

        let mut facts = Vec::new();
        for item in items {
            let number = match &item["Number"] {
                serde_json::Value::Number(n) => n.as_u64().unwrap_or(0) as u32,
                serde_json::Value::String(s) => s.parse::<u32>().unwrap_or(0),
                serde_json::Value::Object(map) => {
                    map.get("value").and_then(|v| v.as_u64()).unwrap_or(0) as u32
                }
                _ => 0,
            };

            let name = item["FriendlyName"]
                .as_str()
                .unwrap_or("Unknown Disk")
                .to_string();

            // 健壮解析 TotalSize (支持数字、字符串或嵌套 map)
            let total_bytes = match &item["TotalSize"] {
                serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
                serde_json::Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
                serde_json::Value::Object(map) => {
                    map.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0)
                }
                _ => 0.0,
            };
            let total_size_gb = (total_bytes / 1_073_741_824.0 * 100.0).round() / 100.0;

            let style = match &item["PartitionStyle"] {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => match n.as_u64() {
                    Some(1) => "MBR".to_string(),
                    Some(2) => "GPT".to_string(),
                    _ => "RAW".to_string(),
                },
                _ => "UNKNOWN".to_string(),
            };

            facts.push(DiskFact {
                number,
                name,
                total_size_gb,
                partition_style: style,
            });
        }

        Ok(facts)
    }

    fn allocate_scratch_partition(&self, disk_id: u32, size_mb: u64, label: &str) -> Result<PathBuf> {
        // PowerShell 脚本：自动查找并分配可用盘符，确保绝对不返回空盘符
        let cmd = format!(r#"
            $ErrorActionPreference = 'Stop'
            $v = Get-Volume -FileSystemLabel "{label}" -ErrorAction SilentlyContinue
            if ($v -and $v.DriveLetter) {{
                return ($v.DriveLetter + ":\")
            }}

            # 如果卷存在但没盘符，分配一个
            if ($v -and (-not $v.DriveLetter)) {{
                $part = Get-Partition -DiskNumber {disk_id} | Where-Object {{ (Get-Volume -Partition $_ -ErrorAction SilentlyContinue).FileSystemLabel -eq "{label}" }} | Select-Object -First 1
                $letter = (Get-ChildItem function:[d-z]: -n | Where-Object {{ -not (Test-Path $_) }})[0].TrimEnd(':')
                Set-Partition -DiskNumber {disk_id} -PartitionNumber $part.PartitionNumber -NewDriveLetter $letter
                Start-Sleep -Seconds 1
                return ($letter + ":\")
            }}

            # 不存在则创建
            $p = Get-Partition -DiskNumber {disk_id} | Where-Object {{ $_.Type -eq 'Basic' -and $_.DriveLetter }} | Sort-Object Size -Descending | Select-Object -First 1
            $bytes = [int64]{size_mb} * 1MB
            Resize-Partition -DriveLetter $p.DriveLetter -Size ($p.Size - $bytes)

            # 寻找空闲盘符
            $freeLetter = (Get-ChildItem function:[d-z]: -n | Where-Object {{ -not (Test-Path $_) }})[0].TrimEnd(':')
            $np = New-Partition -DiskNumber {disk_id} -Size $bytes -DriveLetter $freeLetter
            Format-Volume -Partition $np -FileSystem FAT32 -NewFileSystemLabel "{label}" -Confirm:$false | Out-Null
            Start-Sleep -Seconds 1
            return ($freeLetter + ":\")
        "#);

        let drive = self.run_powershell_raw(&cmd)?;
        let trimmed = drive.lines().last().unwrap_or("").trim().to_string();

        if trimmed.len() < 3 || !trimmed.ends_with(":\\") {
            bail!("Invalid drive letter assigned by host: '{}'", trimmed);
        }

        Ok(PathBuf::from(trimmed))
    }

    fn sync_boot_assets(&self, iso_path: &Path, target_drive: &Path) -> Result<()> {
        let iso_str = iso_path.to_str().unwrap();
        let dest = target_drive.to_str().unwrap();
        let cmd = format!(r#"
            $ErrorActionPreference = 'Stop'
            $m = Mount-DiskImage -ImagePath "{iso_str}" -StorageType ISO -PassThru
            $isoDrive = (Get-Volume -DiskImage $m).DriveLetter + ":\"
            Write-Host "Mount ISO at $isoDrive, Copying to {dest}..."
            robocopy $isoDrive "{dest}" /E /XD "System Volume Information" /NFL /NDL /NJH /NJS /nc /ns /np | Out-Null
            Dismount-DiskImage -ImagePath "{iso_str}" | Out-Null
        "#);

        let output = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &cmd])
            .status()?;
        if !output.success() {
            bail!("Asset sync returned non-zero code");
        }
        Ok(())
    }

    fn verify_uefi_bootloader(&self, target_drive: &Path) -> Result<()> {
        let boot = target_drive.join("EFI").join("BOOT").join("BOOTX64.EFI");
        if !boot.exists() {
            bail!("UEFI bootloader not found at {}", boot.display());
        }
        Ok(())
    }
}
