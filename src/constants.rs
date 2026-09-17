pub const UTF16_NULL_TERMINATOR_LITTERAL: [u16; 1] = [0];

pub const APPLICATION_NAME: &str = cfg_select! {
    not(debug_assertions) => "AudioLocker",
    debug_assertions => "AudioLocker-Debug",
};

pub const CONFIGURATION_FILENAME: &str = "settings.json";
pub const DEFAULT_VOLUME_LEVEL: u8 = 10;

pub const WINDOWS_REGISTRY_AUTO_RUN_PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";

pub mod menu_item_text {
    pub const SETTINGS: &str = "Settings";
    pub const ENABLE_STARTUP: &str = "Enable AutoStart";
    pub const DISABLE_STARTUP: &str = "Disable AutoStart";
    pub const LOGS: &str = "Logs";
    pub const QUIT: &str = "Quit";
}

pub mod icon {
    thread_local! {
        pub static LIGHT: tray_icon::Icon =
            tray_icon::Icon::from_resource_name("light", None).unwrap();
        pub static DARK: tray_icon::Icon =
            tray_icon::Icon::from_resource_name("dark", None).unwrap();
    }
}

pub mod set_icon_on_theme_change {
    pub const RETRY_COUNT: u8 = 3;
    pub const DELAY_MILLI_SECONDS: u64 = 100;
}
