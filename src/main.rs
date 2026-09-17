#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod configuration;
mod constants;
mod logging;
mod registry;
mod utils;
mod wrappers;
// mod windows_bindings;

use std::{error::Error, sync::Arc};

use named_lock::NamedLock;
use notify::{Config, RecommendedWatcher, Watcher};
use rfd::MessageDialog;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::windows::WindowBuilderExtWindows,
    window::WindowBuilder,
};
use tray_icon::{
    TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

use crate::{
    configuration::Configuration,
    constants::{APPLICATION_NAME, menu_item_text, set_icon_on_theme_change},
    logging::{get_logging_directory, init_logging},
    registry::{is_enabled_on_startup, update_enabled_on_startup},
    utils::{get_configuration_file_path, get_icon_for_theme},
    wrappers::DeviceManager,
};

enum UserEvent {
    MenuEvent(tray_icon::menu::MenuEvent),
    ConfigurationReloaded(Result<notify::Event, notify::Error>),
}

// TODO:
//  3. [ ] clean up code
//  5. [ ] verify all debug statements + clean all logs

fn main() -> Result<(), Box<dyn Error>> {
    let _logging_guard = init_logging()?;

    let lock = NamedLock::create(APPLICATION_NAME)?;
    let _namedlock_guard = match lock.try_lock() {
        Ok(v) => v,
        Err(_) => {
            MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title("")
                .set_description(format!("Another instance of {APPLICATION_NAME} already running"))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();

            return Ok(());
        }
    };

    let configuration_file_path = get_configuration_file_path()?;
    let configuration = Arc::new(Configuration::from_file(configuration_file_path, true)?);

    // Note:
    //  We need to set mm_manager as Option to be able to drop it in event_loop.run
    //  Because event_loop.run uses FnMut, so we can't drop mm_manager directly.
    //  Rust's compiler thinks that FnMut can be called again, event though we drop on
    //  the 'Exit' event, rust can't know that at compile time.
    let mut mm_manager = {
        let manager = DeviceManager::new(configuration.clone())?;

        log::info!("Initializing devices");
        manager.initialize_devices()?;
        log::info!("Finished initializing devices");

        Some(manager)
    };

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::MenuEvent(event));
    }));

    let proxy = event_loop.create_proxy();
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = proxy.send_event(UserEvent::ConfigurationReloaded(event));
        },
        Config::default(),
    )?;
    watcher.watch(configuration.get_file_path(), notify::RecursiveMode::NonRecursive)?;

    let tray_menu = Menu::new();

    let settings_i = MenuItem::new(menu_item_text::SETTINGS, true, None);

    let startup_i = match is_enabled_on_startup()? {
        true => MenuItem::new(menu_item_text::DISABLE_STARTUP, true, None),
        false => MenuItem::new(menu_item_text::ENABLE_STARTUP, true, None),
    };

    let logs_i = MenuItem::new(menu_item_text::LOGS, true, None);
    let quit_i = MenuItem::new(menu_item_text::QUIT, true, None);
    tray_menu.append_items(&[
        &startup_i,
        &PredefinedMenuItem::separator(),
        &settings_i,
        &logs_i,
        &PredefinedMenuItem::separator(),
        &quit_i,
    ])?;

    // NOTE:
    //  we set `WindowBuilder.with_drag_and_drop(false)` to prevent the following error:
    //  OleInitialize failed! Result was: `RPC_E_CHANGED_MODE`. Make sure other crates are not using multithreaded COM library on the same thread or disable drag and drop support.
    let window = WindowBuilder::new()
        .with_drag_and_drop(false)
        .with_visible(false)
        .build(&event_loop)?;

    let icon = get_icon_for_theme(window.theme());
    let mut tray_icon = Some(icon.with(|icon| {
        TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu.clone()))
            .with_tooltip(APPLICATION_NAME)
            .with_icon(icon.clone())
            .build()
    })?);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::ThemeChanged(theme),
                ..
            } => {
                log::info!("Theme change was detected changed to {theme:#?}");

                tray_icon.as_ref().inspect(|tray_icon| {
                    for _ in 0..set_icon_on_theme_change::RETRY_COUNT {
                        std::thread::sleep(std::time::Duration::from_millis(
                            set_icon_on_theme_change::DELAY_MILLI_SECONDS,
                        ));

                        let icon = get_icon_for_theme(window.theme());

                        let result = icon.with(|icon| {
                            let result = match tray_icon.set_icon(Some(icon.clone())) {
                                Ok(_) => Some(()),
                                Err(error) => {
                                    log::warn!("Error occured while trying to update the icon: {:#?}", error);

                                    None
                                }
                            };

                            result
                        });

                        if result.is_some() {
                            break;
                        }
                    }
                });
            }

            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                if event.id == quit_i.id() {
                    tray_icon.take();
                    mm_manager.take();

                    *control_flow = ControlFlow::Exit;
                } else if event.id == settings_i.id() {
                    match open::that(configuration.get_file_path()) {
                        Ok(_) => log::info!("Opened configuration file"),
                        Err(error) => {
                            MessageDialog::new()
                                .set_level(rfd::MessageLevel::Error)
                                .set_title("Failed opening configuration file")
                                .set_description(format!("{:#?}", error))
                                .set_buttons(rfd::MessageButtons::Ok)
                                .show();
                        }
                    }
                } else if event.id == startup_i.id() {
                    match update_enabled_on_startup() {
                        Ok(true) => startup_i.set_text(menu_item_text::DISABLE_STARTUP),
                        Ok(false) => startup_i.set_text(menu_item_text::ENABLE_STARTUP),
                        Err(error) => {
                            MessageDialog::new()
                                .set_level(rfd::MessageLevel::Error)
                                .set_title("Error occured while trying to update registry")
                                .set_description(format!("{:#?}", error))
                                .set_buttons(rfd::MessageButtons::Ok)
                                .show();
                        }
                    }
                } else if event.id == logs_i.id()
                    && let Ok(logs_dir) = get_logging_directory()
                {
                    match open::that(logs_dir) {
                        Ok(_) => log::info!("Opened logs"),
                        Err(error) => {
                            MessageDialog::new()
                                .set_level(rfd::MessageLevel::Error)
                                .set_title("Unable to open logs directory")
                                .set_description(format!("{:#?}", error))
                                .set_buttons(rfd::MessageButtons::Ok)
                                .show();
                        }
                    }
                }
            }

            Event::UserEvent(UserEvent::ConfigurationReloaded(Ok(event))) => {
                if let notify::EventKind::Modify(_) = event.kind {
                    mm_manager.as_ref().inspect(|manager| {
                        manager.on_configuration_change();
                    });
                }
            }

            _ => {}
        }
    })
}
