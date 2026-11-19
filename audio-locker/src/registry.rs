use std::error::Error;

use windows_registry::{CURRENT_USER, Transaction};

use crate::constants::{APPLICATION_NAME, WINDOWS_REGISTRY_AUTO_RUN_PATH};

pub fn is_enabled_on_startup() -> Result<bool, Box<dyn Error>> {
    let key = CURRENT_USER.options().read().open(WINDOWS_REGISTRY_AUTO_RUN_PATH)?;

    Ok(key.get_type(APPLICATION_NAME).is_ok())
}

pub(crate) enum StartupStatus {
    Enabled,
    Disabled,
}

pub fn update_enabled_on_startup() -> Result<StartupStatus, Box<dyn Error>> {
    let tx = Transaction::new()?;
    let key = CURRENT_USER
        .options()
        .read()
        .write()
        .transaction(&tx)
        .open(WINDOWS_REGISTRY_AUTO_RUN_PATH)?;

    let startup_status = if key.get_string(APPLICATION_NAME).is_ok() {
        key.remove_value(APPLICATION_NAME)?;

        StartupStatus::Disabled
    } else {
        key.set_hstring(
            APPLICATION_NAME,
            &windows_core::HSTRING::from(std::env::current_exe()?.as_path()),
        )?;

        StartupStatus::Enabled
    };

    tx.commit()?;

    Ok(startup_status)
}
