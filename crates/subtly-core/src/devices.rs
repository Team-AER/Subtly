//! GPU device enumeration via wgpu, plus the "best device" auto-select heuristic
//! lifted from `src/renderer/App.jsx`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DeviceInfo {
    pub name: String,
    pub vendor: u32,
    pub device: u32,
    pub device_type: String,
    pub backend: String,
    pub driver: String,
    pub driver_info: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PingResult {
    pub message: String,
    pub gpu_enabled: bool,
    pub gpu_name: Option<String>,
    pub gpu_backend: String,
    pub gpu_type: String,
}

/// Probe the first available GPU adapter via wgpu and return a status payload.
/// Wrapped in `catch_unwind` so a flaky Vulkan ICD can't take down the UI.
pub fn ping() -> PingResult {
    let info = std::panic::catch_unwind(|| {
        let instance = wgpu::Instance::default();
        instance
            .enumerate_adapters(wgpu::Backends::all())
            .into_iter()
            .next()
            .map(|adapter| adapter.get_info())
    })
    .unwrap_or(None);

    match info {
        Some(info) => PingResult {
            message: "Runtime ready".to_string(),
            gpu_enabled: true,
            gpu_name: Some(info.name),
            gpu_backend: format!("{:?}", info.backend),
            gpu_type: format!("{:?}", info.device_type),
        },
        None => PingResult {
            message: "Runtime ready (CPU fallback)".to_string(),
            gpu_enabled: false,
            gpu_name: None,
            gpu_backend: "CPU".to_string(),
            gpu_type: "Cpu".to_string(),
        },
    }
}

pub fn list_devices() -> Vec<DeviceInfo> {
    let infos = std::panic::catch_unwind(|| {
        let instance = wgpu::Instance::default();
        instance
            .enumerate_adapters(wgpu::Backends::all())
            .into_iter()
            .map(|adapter| adapter.get_info())
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();

    infos
        .into_iter()
        .map(|info| DeviceInfo {
            name: info.name,
            vendor: info.vendor,
            device: info.device,
            device_type: format!("{:?}", info.device_type),
            backend: format!("{:?}", info.backend),
            driver: info.driver,
            driver_info: info.driver_info,
        })
        .collect()
}

/// Run a tiny wgpu buffer creation to verify the device is usable.
/// Lifted from the sidecar `smoke_test`.
pub fn smoke_test() -> anyhow::Result<String> {
    use anyhow::anyhow;
    let result = std::panic::catch_unwind(|| -> anyhow::Result<String> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .enumerate_adapters(wgpu::Backends::all())
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No compatible GPU adapters found"))?;
        let info = adapter.get_info();
        let (device, _queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("subtly-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))?;
        let _ = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("smoke-buffer"),
            size: 1024,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        Ok(format!(
            "Smoke test ok on {} ({:?})",
            info.name, info.backend
        ))
    });
    match result {
        Ok(inner) => inner,
        Err(_) => Err(anyhow!("smoke test panicked (likely a flaky GPU driver)")),
    }
}

/// Heuristic: prefer discrete > integrated > any GPU-backed > CPU.
/// Ported from `selectBestDevice` in `src/renderer/App.jsx`.
pub fn select_best_device(devices: &[DeviceInfo]) -> Option<&DeviceInfo> {
    if devices.is_empty() {
        return None;
    }
    let gpu_backends = ["vulkan", "metal", "wgpu"];
    let is_gpu_backend = |d: &DeviceInfo| {
        let b = d.backend.to_lowercase();
        gpu_backends.iter().any(|x| b.contains(x))
    };

    for ty in ["DiscreteGpu", "IntegratedGpu"] {
        if let Some(d) = devices.iter().find(|d| d.device_type == ty && is_gpu_backend(d)) {
            return Some(d);
        }
    }
    if let Some(d) = devices.iter().find(|d| is_gpu_backend(d)) {
        return Some(d);
    }
    devices
        .iter()
        .find(|d| d.device_type == "Cpu")
        .or_else(|| devices.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, backend: &str, ty: &str) -> DeviceInfo {
        DeviceInfo {
            name: name.into(),
            vendor: 0,
            device: 0,
            device_type: ty.into(),
            backend: backend.into(),
            driver: String::new(),
            driver_info: String::new(),
        }
    }

    #[test]
    fn empty_returns_none() {
        assert!(select_best_device(&[]).is_none());
    }

    #[test]
    fn prefers_discrete_gpu() {
        let devs = vec![
            dev("integrated", "Vulkan", "IntegratedGpu"),
            dev("discrete", "Vulkan", "DiscreteGpu"),
            dev("cpu", "CPU", "Cpu"),
        ];
        assert_eq!(select_best_device(&devs).unwrap().name, "discrete");
    }

    #[test]
    fn falls_back_to_integrated() {
        let devs = vec![
            dev("integrated", "Metal", "IntegratedGpu"),
            dev("cpu", "CPU", "Cpu"),
        ];
        assert_eq!(select_best_device(&devs).unwrap().name, "integrated");
    }

    #[test]
    fn falls_back_to_cpu_last() {
        let devs = vec![dev("cpu", "CPU", "Cpu")];
        assert_eq!(select_best_device(&devs).unwrap().name, "cpu");
    }
}
