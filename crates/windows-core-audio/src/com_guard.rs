use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

pub struct ComInitializationGuard;

impl ComInitializationGuard {
    pub fn new() -> windows_core::Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }

        Ok(Self)
    }
}

impl Drop for ComInitializationGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
