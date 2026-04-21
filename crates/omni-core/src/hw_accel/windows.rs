//! Détection et initialisation des accélérateurs matériels Windows.

#[cfg(windows)]
pub mod win {
    use anyhow::Result;

    pub fn detect_available_hw_accels() -> Vec<&'static str> {
        let mut available = Vec::new();
        if is_d3d11va_available() { available.push("d3d11va"); }
        if is_dxva2_available()   { available.push("dxva2"); }
        available.push("none");
        available
    }

    pub fn is_d3d11va_available() -> bool {
        use windows::{
            Win32::Graphics::Direct3D::*,
            Win32::Graphics::Direct3D11::*,
            Win32::Graphics::Dxgi::IDXGIAdapter,
        };
        unsafe {
            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                None,
                None,
                None,
            ).is_ok()
        }
    }

    pub fn is_dxva2_available() -> bool {
        use windows::Win32::System::LibraryLoader::LoadLibraryA;
        use windows::Win32::Foundation::FreeLibrary;
        use windows::core::PCSTR;
        unsafe {
            match LoadLibraryA(PCSTR::from_raw(b"dxva2.dll\0".as_ptr())) {
                Ok(h) => { let _ = FreeLibrary(h); true }
                Err(_) => false,
            }
        }
    }

    pub fn primary_gpu_name() -> String {
        use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
        unsafe {
            let factory: IDXGIFactory1 = match CreateDXGIFactory1() {
                Ok(f)  => f,
                Err(_) => return "Inconnu".into(),
            };
            let adapter = match factory.EnumAdapters1(0) {
                Ok(a)  => a,
                Err(_) => return "Inconnu".into(),
            };
            let desc = match adapter.GetDesc1() {
                Ok(d)  => d,
                Err(_) => return "Inconnu".into(),
            };
            String::from_utf16_lossy(
                desc.Description.iter().take_while(|&&c| c != 0).cloned().collect::<Vec<_>>().as_slice(),
            )
        }
    }
}
