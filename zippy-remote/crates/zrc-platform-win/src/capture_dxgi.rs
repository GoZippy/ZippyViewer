#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use thiserror::Error;
use windows::core::Interface;
use windows::Win32::{
    Foundation::*,
    Graphics::Direct3D11::*,
    Graphics::Direct3D::*,
    Graphics::Dxgi::*,
    Graphics::Dxgi::Common::*,
};

use crate::capture_gdi::BgraFrame;

#[derive(Debug, Error)]
pub enum DxgiError {
    #[error("DXGI not available")]
    NotAvailable,
    #[error("device creation failed")]
    DeviceCreation,
    #[error("output duplication failed")]
    DuplicationFailed,
    #[error("device lost")]
    DeviceLost,
    #[error("desktop switch (UAC/lock screen)")]
    DesktopSwitch,
    #[error("timeout")]
    Timeout,
    #[error("win32 error: {0}")]
    Win32(String),
}

/// DXGI Desktop Duplication capture (Windows 8+)
pub struct DxgiCapturer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    output_duplication: IDXGIOutputDuplication,
    staging_texture: ID3D11Texture2D,
    current_output: u32,
    width: u32,
    height: u32,
}

impl DxgiCapturer {
    /// Check if DXGI Desktop Duplication is available
    pub fn is_available() -> bool {
        unsafe {
            // Try to create a D3D11 device
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            let feature_levels = [
                D3D_FEATURE_LEVEL_11_1,
                D3D_FEATURE_LEVEL_11_0,
                D3D_FEATURE_LEVEL_10_1,
                D3D_FEATURE_LEVEL_10_0,
            ];

            let hr = D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG::default(),
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            );

            hr.is_ok()
        }
    }

    /// Create DXGI capturer for primary output
    pub fn new() -> Result<Self, DxgiError> {
        unsafe {
            // Create D3D11 device
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            let feature_levels = [
                D3D_FEATURE_LEVEL_11_1,
                D3D_FEATURE_LEVEL_11_0,
                D3D_FEATURE_LEVEL_10_1,
                D3D_FEATURE_LEVEL_10_0,
            ];

            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG::default(),
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            ).map_err(|_| DxgiError::DeviceCreation)?;

            let device = device.ok_or(DxgiError::DeviceCreation)?;
            let context = context.ok_or(DxgiError::DeviceCreation)?;

            // Get DXGI device
            let dxgi_device: IDXGIDevice = device.cast().map_err(|_| DxgiError::DeviceCreation)?;
            let adapter: IDXGIAdapter = dxgi_device
                .GetAdapter()
                .map_err(|e| DxgiError::Win32(format!("GetAdapter: {e:?}")))?;

            // Get primary output
            let output: IDXGIOutput = adapter
                .EnumOutputs(0)
                .map_err(|e| DxgiError::Win32(format!("EnumOutputs: {e:?}")))?;

            // Get output description
            let desc = output
                .GetDesc()
                .map_err(|e| DxgiError::Win32(format!("GetDesc: {e:?}")))?;

            // Duplicate output
            let output1: IDXGIOutput1 = output.cast().map_err(|_| DxgiError::DuplicationFailed)?;
            let output_duplication = output1
                .DuplicateOutput(&device)
                .map_err(|e| DxgiError::Win32(format!("DuplicateOutput: {e:?}")))?;

            // Create staging texture
            let width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left) as u32;
            let height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top) as u32;
            
            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };

            let mut staging_texture: Option<ID3D11Texture2D> = None;
            device
                .CreateTexture2D(&staging_desc, None, Some(&mut staging_texture))
                .map_err(|e| DxgiError::Win32(format!("CreateTexture2D: {e:?}")))?;

            let staging_texture = staging_texture.ok_or(DxgiError::DeviceCreation)?;

            Ok(Self {
                device,
                context,
                output_duplication,
                staging_texture,
                current_output: 0,
                width,
                height,
            })
        }
    }

    /// Capture next frame with dirty rectangles
    pub fn capture_frame(&mut self, timeout_ms: u32) -> Result<BgraFrame, DxgiError> {
        unsafe {
            let mut frame_info = Default::default();
            let mut desktop_resource: Option<IDXGIResource> = None;

            // Acquire next frame
            let result = self.output_duplication.AcquireNextFrame(
                timeout_ms,
                &mut frame_info,
                &mut desktop_resource,
            );

            if let Err(e) = &result {
                let code = e.code();
                if code == DXGI_ERROR_DEVICE_REMOVED {
                    return Err(DxgiError::DeviceLost);
                }
                if code == DXGI_ERROR_ACCESS_LOST {
                    return Err(DxgiError::DesktopSwitch);
                }
                if code == DXGI_ERROR_WAIT_TIMEOUT {
                    return Err(DxgiError::Timeout);
                }
                return Err(DxgiError::Win32(format!("AcquireNextFrame: {e:?}")));
            }

            let desktop_resource = desktop_resource.ok_or(DxgiError::DuplicationFailed)?;

            // Get texture from resource
            let desktop_texture: ID3D11Texture2D = desktop_resource
                .cast()
                .map_err(|_| DxgiError::DuplicationFailed)?;

            // Copy to staging texture
            self.context.CopyResource(&self.staging_texture, &desktop_texture);

            // Map staging texture for CPU access
            let mut mapped = Default::default();
            self.context.Map(
                &self.staging_texture,
                0,
                D3D11_MAP_READ,
                0,
                Some(&mut mapped),
            ).map_err(|e| {
                let _ = self.output_duplication.ReleaseFrame();
                DxgiError::Win32(format!("Map: {e:?}"))
            })?;

            // Copy pixel data
            let stride = mapped.RowPitch as usize;
            let height = self.height as usize;
            
            let src = std::slice::from_raw_parts(mapped.pData as *const u8, stride * height);

            // Convert to BGRA (DXGI format is already BGRA)
            let bgra_stride = (self.width * 4) as usize;
            let mut bgra = vec![0u8; bgra_stride * height];

            for y in 0..height {
                let src_row = &src[y * stride..y * stride + bgra_stride];
                let dst_row = &mut bgra[y * bgra_stride..y * bgra_stride + bgra_stride];
                dst_row.copy_from_slice(src_row);
            }

            self.context.Unmap(&self.staging_texture, 0);
            let _ = self.output_duplication.ReleaseFrame();

            Ok(BgraFrame {
                width: self.width,
                height: self.height,
                stride: bgra_stride as u32,
                bgra,
            })
        }
    }

    /// Handle device lost error
    pub fn handle_device_lost(&mut self) -> Result<(), DxgiError> {
        // Recreate device and duplication
        *self = Self::new()?;
        Ok(())
    }

    /// Handle desktop switch (UAC, lock screen)
    pub fn handle_desktop_switch(&mut self) -> Result<(), DxgiError> {
        // Try to recreate duplication
        unsafe {
            let dxgi_device: IDXGIDevice = self.device.cast().map_err(|_| DxgiError::DuplicationFailed)?;
            let adapter: IDXGIAdapter = dxgi_device.GetAdapter().map_err(|_| DxgiError::DuplicationFailed)?;
            let output: IDXGIOutput = adapter.EnumOutputs(self.current_output).map_err(|_| DxgiError::DuplicationFailed)?;
            let output1: IDXGIOutput1 = output.cast().map_err(|_| DxgiError::DuplicationFailed)?;

            self.output_duplication = output1
                .DuplicateOutput(&self.device)
                .map_err(|e| DxgiError::Win32(format!("DuplicateOutput: {e:?}")))?;
        }

        Ok(())
    }
}
