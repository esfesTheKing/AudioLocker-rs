use std::{
    io,
    path::{self, PathBuf},
    thread::LocalKey,
};

use tao::window::Theme;

use crate::constants::{APPLICATION_NAME, CONFIGURATION_FILENAME, icon};

pub fn get_icon_for_theme(theme: Theme) -> &'static LocalKey<tray_icon::Icon> {
    match theme {
        Theme::Light => &icon::LIGHT,
        Theme::Dark => &icon::DARK,
        unknown_theme => {
            log::warn!("Got unknown theme: {unknown_theme:#?}");

            &icon::LIGHT
        }
    }
}

pub fn get_configuration_file_path() -> io::Result<PathBuf> {
    let mut near_exe = PathBuf::from(".");
    near_exe.push(CONFIGURATION_FILENAME);

    // Return config file near exe if exists
    if let Ok(absolute_path) = near_exe.canonicalize() {
        return Ok(absolute_path);
    }

    // Return config file in the configuration folder if folder exists
    if let Some(mut home) = std::env::var_os("APPDATA").map(PathBuf::from) {
        home.push(APPLICATION_NAME);
        home.push(CONFIGURATION_FILENAME);

        return path::absolute(home);
    }

    // Return config file near exe
    path::absolute(near_exe)
}
