use std::path::Path;
use std::sync::OnceLock;
use std::thread::ThreadId;

use parking_lot::RwLock;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::{
    Storage::EnhancedStorage::PKEY_Software_ProductName,
    UI::Shell::{IShellItem2, SHCreateItemFromParsingName},
};
use windows_core::HSTRING;
use windows_core::PWSTR;

use crate::AudioSession;

pub struct RaiiPwstr(pub PWSTR);

impl RaiiPwstr {
    pub fn to_string(&self) -> windows_core::Result<String> {
        unsafe { self.0.to_string().map_err(windows_core::Error::from) }
    }
}

impl Drop for RaiiPwstr {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(Some(self.0.as_ptr().cast()));
        }
    }
}

static SYSTEM: OnceLock<RwLock<System>> = OnceLock::new();
fn get_system_information() -> &'static RwLock<System> {
    SYSTEM.get_or_init(|| {
        let system = System::new_with_specifics(RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()));

        RwLock::new(system)
    })
}

fn get_process_name_from_file_information(path: &str) -> Option<String> {
    unsafe {
        SHCreateItemFromParsingName(&HSTRING::from(path), None)
            .and_then(|shell_item: IShellItem2| shell_item.GetString(&PKEY_Software_ProductName))
            .and_then(|prop| prop.to_string().map_err(windows_core::Error::from))
            .ok()
    }
}

pub fn get_process_name(process_id: u32) -> Option<String> {
    let pid = Pid::from_u32(process_id);

    let mut system_information = get_system_information().write();
    system_information.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::OnlyIfNotSet),
    );

    let process = system_information.process(pid)?;

    process
        .exe()
        .and_then(Path::to_str)
        .and_then(get_process_name_from_file_information)
        .or_else(|| {
            let string = process.name().to_str()?;

            Some(string.strip_suffix(".exe").unwrap_or(string).to_owned())
        })
}

pub fn get_session_name(session: &AudioSession) -> windows_core::Result<String> {
    let session_name = match session.is_system_sound() {
        true => String::from("System Sounds"),
        false => {
            let display_name = session.display_name()?;
            if display_name.is_empty() {
                get_process_name(session.process_id()?).unwrap_or(display_name)
            } else {
                display_name
            }
        }
    };

    Ok(session_name)
}
