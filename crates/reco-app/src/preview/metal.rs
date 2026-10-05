//! Raw Metal handles, so a UI on the same `MTLDevice` can adopt the ring's
//! textures without a copy. `None` off macOS or on a non-Metal backend.

use reco_core::gpu::GpuContext;
use reco_core::wgpu;

/// The wgpu device's `MTLDevice`, as a pointer value.
pub fn device_ptr(gpu: &GpuContext) -> Option<usize> {
    #[cfg(target_os = "macos")]
    {
        use metal::foreign_types::ForeignType;
        // SAFETY: only reads the handle; the device outlives this call.
        unsafe { gpu.device().as_hal::<wgpu::hal::api::Metal>() }
            .map(|d| d.raw_device().as_ptr() as usize)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = gpu;
        None
    }
}

/// The texture's `MTLTexture`, as a pointer value. The texture must stay
/// alive until the UI has retained it (the ring's adoption handshake).
pub fn texture_ptr(texture: &wgpu::Texture) -> Option<usize> {
    #[cfg(target_os = "macos")]
    {
        use metal::foreign_types::ForeignType;
        // SAFETY: only reads the handle.
        unsafe { texture.as_hal::<wgpu::hal::api::Metal>() }
            .map(|t| unsafe { t.raw_handle().as_ptr() } as usize)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = texture;
        None
    }
}
