use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Qualcomm,
    Software,
    Other(u32),
}

impl GpuVendor {
    pub fn from_vendor_id(vendor_id: u32) -> Self {
        match vendor_id {
            0x10DE => Self::Nvidia,
            0x1002 => Self::Amd,
            0x8086 => Self::Intel,
            0x5143 | 0x4D4F => Self::Qualcomm,
            0x1414 => Self::Software, // Microsoft Basic Render Driver
            other => Self::Other(other),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Nvidia => "NVIDIA",
            Self::Amd => "AMD",
            Self::Intel => "Intel",
            Self::Qualcomm => "Qualcomm",
            Self::Software => "Software Rasterizer",
            Self::Other(_) => "Other Vendor",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: GpuVendor,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_vram_bytes: u64,
    pub shared_system_memory_bytes: u64,
    pub is_discrete: bool,
    pub is_software: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHardwareSummary {
    pub gpus: Vec<GpuInfo>,
    pub primary_gpu: Option<GpuInfo>,
    pub total_dedicated_vram_bytes: u64,
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub cpu_cores: usize,
    pub cpu_brand: String,
    pub os_arch: String,
    pub os_platform: String,
}

impl SystemHardwareSummary {
    pub fn has_discrete_gpu(&self) -> bool {
        self.gpus.iter().any(|g| g.is_discrete && !g.is_software)
    }

    pub fn primary_gpu_vendor(&self) -> Option<GpuVendor> {
        self.primary_gpu.as_ref().map(|g| g.vendor)
    }

    pub fn total_vram_gb(&self) -> f64 {
        self.total_dedicated_vram_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn total_ram_gb(&self) -> f64 {
        self.total_ram_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn available_ram_gb(&self) -> f64 {
        self.available_ram_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

pub struct HardwareDetector;

impl HardwareDetector {
    /// Detects all system hardware without assuming any specific vendor or device.
    pub fn detect() -> SystemHardwareSummary {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        sys.refresh_cpu_all();

        let total_ram = sys.total_memory();
        let available_ram = sys.available_memory();
        let cpu_cores = sys.cpus().len().max(1);
        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "Unknown CPU".to_string());

        let gpus = Self::detect_gpus();

        // Determine primary GPU (prefer discrete GPU with largest dedicated VRAM)
        let primary_gpu = gpus
            .iter()
            .filter(|g| !g.is_software)
            .max_by_key(|g| g.dedicated_vram_bytes)
            .cloned()
            .or_else(|| gpus.first().cloned());

        let total_vram = primary_gpu
            .as_ref()
            .map(|g| g.dedicated_vram_bytes)
            .unwrap_or(0);

        SystemHardwareSummary {
            gpus,
            primary_gpu,
            total_dedicated_vram_bytes: total_vram,
            total_ram_bytes: total_ram,
            available_ram_bytes: available_ram,
            cpu_cores,
            cpu_brand,
            os_arch: std::env::consts::ARCH.to_string(),
            os_platform: std::env::consts::OS.to_string(),
        }
    }

    #[cfg(target_os = "windows")]
    fn detect_gpus() -> Vec<GpuInfo> {
        let mut detected = Vec::new();

        unsafe {
            use windows::Win32::Graphics::Dxgi::{
                CreateDXGIFactory1, IDXGIFactory1,
            };

            if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
                let mut adapter_index = 0;
                while let Ok(adapter) = factory.EnumAdapters1(adapter_index) {
                    adapter_index += 1;

                    if let Ok(desc) = adapter.GetDesc1() {
                        let name_len = desc
                            .Description
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(desc.Description.len());
                        let name = String::from_utf16_lossy(&desc.Description[..name_len])
                            .trim()
                            .to_string();

                        let vendor = GpuVendor::from_vendor_id(desc.VendorId);
                        let is_software = (desc.Flags & 1 != 0) || vendor == GpuVendor::Software;
                        let dedicated_vram = desc.DedicatedVideoMemory as u64;
                        let shared_memory = desc.SharedSystemMemory as u64;

                        // Discrete GPUs typically have DedicatedVideoMemory > 256MB and aren't software
                        let is_discrete = !is_software && dedicated_vram > 256 * 1024 * 1024;

                        detected.push(GpuInfo {
                            name,
                            vendor,
                            vendor_id: desc.VendorId,
                            device_id: desc.DeviceId,
                            dedicated_vram_bytes: dedicated_vram,
                            shared_system_memory_bytes: shared_memory,
                            is_discrete,
                            is_software,
                        });
                    }
                }
            }
        }

        detected
    }

    #[cfg(not(target_os = "windows"))]
    fn detect_gpus() -> Vec<GpuInfo> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vendor_id_mapping() {
        assert_eq!(GpuVendor::from_vendor_id(0x10DE), GpuVendor::Nvidia);
        assert_eq!(GpuVendor::from_vendor_id(0x1002), GpuVendor::Amd);
        assert_eq!(GpuVendor::from_vendor_id(0x8086), GpuVendor::Intel);
        assert_eq!(GpuVendor::from_vendor_id(0x5143), GpuVendor::Qualcomm);
        assert_eq!(GpuVendor::from_vendor_id(0x1414), GpuVendor::Software);
        assert_eq!(GpuVendor::from_vendor_id(0x9999), GpuVendor::Other(0x9999));
    }

    #[test]
    fn test_hardware_detector_runs() {
        let summary = HardwareDetector::detect();
        assert!(summary.cpu_cores >= 1);
        assert!(summary.total_ram_bytes > 0);
        assert!(!summary.cpu_brand.is_empty());
        // Should detect system memory in GB
        assert!(summary.total_ram_gb() > 0.1);
    }
}
