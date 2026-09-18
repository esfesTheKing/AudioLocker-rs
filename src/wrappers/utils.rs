use crate::bindings::com::CoTaskMemFree;
use windows_core::PWSTR;

pub struct RaiiPwstr(pub PWSTR);

impl RaiiPwstr {
    pub unsafe fn to_string(&self) -> windows_core::Result<String> {
        unsafe { self.0.to_string().map_err(windows_core::Error::from) }
    }
}

impl Drop for RaiiPwstr {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(self.0.as_ptr().cast());
        }
    }
}
