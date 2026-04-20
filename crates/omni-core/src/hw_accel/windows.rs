//! Détection et initialisation des accélérateurs matériels Windows.
//! DXVA2, D3D11VA — interroge directement les APIs Windows pour vérifier
//! la disponibilité avant de les proposer à FFmpeg.

#[cfg(windows)]
pub mod win {
    use anyhow::{Context as _, Result};

    /// Retourne la liste des accélérateurs disponibles sur cette machine.
    pub fn detect_available_hw_accels() -> Vec<&'static str> {
        let mut available = Vec::new();

        if is_d3d11va_available() { available.push("d3d11va"); }
        if is_dxva2_available()   { available.push("dxva2"); }
        available.push("none");  // toujours disponible (soft decode)

        available
    }

    /// Vérifie la disponibilité de D3D11VA (Windows 8+).
    pub fn is_d3d11va_available() -> bool {
        // Tente de créer un Device D3D11 en mode test
        use windows::{
            Win32::Graphics::Direct3D11::*,
            Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0,
            Win32::Graphics::Dxgi::IDXGIAdapter,
        };

        unsafe {
            let mut device = None;
            let mut level  = D3D_FEATURE_LEVEL_11_0;
            let mut ctx    = None;

            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut level),
                Some(&mut ctx),
            )
            .is_ok()
        }
    }

    /// Vérifie la disponibilité de DXVA2 (Windows 7+).
    pub fn is_dxva2_available() -> bool {
        // DXVA2 est disponible sur tout Windows 7+
        // On vérifie juste si la DLL est présente
        unsafe {
            let lib = windows::core::PCSTR::from_raw(b"dxva2.dll\0".as_ptr());
            let handle = windows::Win32::Foundation::HMODULE(
                windows::Win32::System::LibraryLoader::LoadLibraryA(lib)
                    .unwrap_or_default()
                    .0,
            );
            let ok = !handle.is_invalid();
            if ok {
                let _ = windows::Win32::System::LibraryLoader::FreeLibrary(handle);
            }
            ok
        }
    }

    /// Retourne le GPU principal (nom du périphérique DXGI).
    pub fn primary_gpu_name() -> String {
        unsafe {
            let factory: windows::Win32::Graphics::Dxgi::IDXGIFactory1 =
                match windows::Win32::Graphics::Dxgi::CreateDXGIFactory1() {
                    Ok(f)  => f,
                    Err(_) => return "Inconnu".into(),
                };

            let adapter = match factory.EnumAdapters1(0) {
                Ok(a)  => a,
                Err(_) => return "Inconnu".into(),
            };

            let mut desc = windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_DESC1::default();
            let _ = adapter.GetDesc1(&mut desc);

            String::from_utf16_lossy(
                desc.Description
                    .iter()
                    .take_while(|&&c| c != 0)
                    .cloned()
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
        }
    }
}
