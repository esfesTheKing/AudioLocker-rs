use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
    thread,
};

use parking_lot::RwLock;
use windows::Win32::Media::Audio::{IMMNotificationClient, eRender};
use windows_core::AsImpl;

use crate::{
    configuration::Configuration,
    wrappers::{Device, DeviceEnumerator, MMEvent, MMNotificationClient},
};

pub struct DeviceManager {
    config: Arc<Configuration>,

    sender: crossbeam_channel::Sender<MMEvent>,
    thread_handle: Option<thread::JoinHandle<()>>,

    devices: Arc<RwLock<HashMap<String, Device>>>,
    notification_client: Option<IMMNotificationClient>,
    enumerator: DeviceEnumerator,
}

impl DeviceManager {
    pub fn new(config: Arc<Configuration>) -> windows_core::Result<Self> {
        let devices = Arc::new(RwLock::new(HashMap::new()));
        let enumerator = unsafe { DeviceEnumerator::new(config.clone())? };

        let (sender, receiver) = crossbeam_channel::unbounded::<MMEvent>();
        let notification_client = Some(IMMNotificationClient::from(MMNotificationClient::new(
            sender.clone(),
            AtomicBool::new(true),
        )));

        let thread_handle = {
            let config = config.clone();
            let devices = devices.clone();

            thread::spawn(move || {
                let enumerator: DeviceEnumerator = match unsafe { DeviceEnumerator::new(config.clone()) } {
                    Ok(v) => v,
                    Err(error) => {
                        log::error!("Unable to create a device enumerator in the background thread due to {error:#?}");
                        return;
                    }
                };

                while let Ok(event) = receiver.recv() {
                    match event {
                        MMEvent::DeviceAdded(device_id) => {
                            let device = match unsafe { enumerator.get_device_by_id(&device_id) } {
                                Ok(device) => device,
                                Err(error) => {
                                    log::error!("Unable to get device from `{device_id}` due to {error:#?}");
                                    continue;
                                }
                            };

                            if unsafe { device.data_flow().unwrap() } != eRender {
                                continue;
                            }

                            let device_name = unsafe { device.name().unwrap() };
                            log::info!("[{device_name}] Initializing device");

                            if let Err(error) = unsafe { device.initialize_sessions() } {
                                log::error!("[{device_name}] Failed to initialize sessions {error:#?}");
                                continue;
                            }

                            if let Err(error) = config.write_to_disk() {
                                log::error!("Failed to write configuration to disk due to {error:#?}");
                                continue;
                            }

                            log::info!("[{device_name}] Finished initializing device");

                            devices.write().insert(device_name, device);
                        }
                        MMEvent::DeviceRemoved(device_id) => {
                            // It is cheap the get the a "new" MMDevice as we didn't call initialize_sessions() to make it expensive
                            let device = match unsafe { enumerator.get_device_slim_by_id(&device_id) } {
                                Ok(v) => v,
                                Err(error) => {
                                    log::error!("Unable to get device from `{device_id}` due to {error:#?}");
                                    continue;
                                }
                            };

                            let device_name = match unsafe { device.name() } {
                                Ok(device_name) => device_name,
                                Err(error) => {
                                    log::error!("Unable to get {device_id}'s name due to {error:#?}");
                                    continue;
                                }
                            };

                            let _ = devices.write().remove(&device_name);

                            log::info!("[{device_name}] Removed device");
                        }
                        MMEvent::Exit => return,
                    }
                }
            })
        };

        unsafe { enumerator.register_client(notification_client.as_ref())? }

        Ok(Self {
            config,
            sender,
            thread_handle: Some(thread_handle),
            devices,
            notification_client,
            enumerator,
        })
    }

    pub fn initialize_devices(&self) -> windows_core::Result<()> {
        unsafe {
            let mut devices = self.devices.write();

            for device in self.enumerator.devices()?.iter() {
                let device_name = device.name()?;

                log::info!("[{device_name}] Initializing device");
                device.initialize_sessions()?;
                log::info!("[{device_name}] Finished initializing device");

                devices.insert(device_name, device);
            }

            self.config.write_to_disk()?;
        }

        Ok(())
    }

    pub fn on_configuration_change(&self) {
        log::info!("[Configuration] Detected change from file");
        if let Err(error) = self.config.read_from_disk() {
            log::warn!("[Configuration] unable to read file: {error}");
            return;
        }

        log::info!("[Configuration] propagating change to devices");
        self.devices.read().iter().for_each(|(_, device)| {
            unsafe { device.on_configuration_change() };
        });
    }
}

impl Drop for DeviceManager {
    fn drop(&mut self) {
        // Stop handling notifications during shutdown
        if let Some(client) = self.notification_client.as_ref() {
            unsafe { client.as_impl() }.disable();
        }

        if self.sender.send(MMEvent::Exit).is_err() {
            log::error!("Receiver has been dropped inside internal thread");
        }

        if let Some(thread_handle) = self.thread_handle.take() {
            match thread_handle.join() {
                Ok(_) => log::debug!("Thread exited cleanly"),
                Err(error) => log::error!("Internal thread has panicked with error {error:#?}"),
            }
        } else {
            log::error!("thread_handle was not set");
        }

        // SAFETY:
        //  - `self.notification_client` is the same interface pointer registered on this exact
        //    `self.enumerator` in `new()`, so E_POINTER / E_NOTFOUND cannot occur.
        //  - Register/UnregisterEndpointNotificationCallback do NOT AddRef/Release the client,
        //    `self.notification_client` must outlive the `self.enumerator.unregister_client` call.
        //    Owning it via `take()` guarantees that.
        //
        //  links to docs: https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nf-mmdeviceapi-immdeviceenumerator-unregisterendpointnotificationcallback
        if let Some(client) = self.notification_client.take() {
            if let Err(error) = unsafe { self.enumerator.unregister_client(Some(&client)) } {
                log::error!("Error occurred when unregistering client due to {error:#?}");

                // The enumerator may still hold a raw pointer to the client; leak it rather than
                // free an object the OS could still call into.
                std::mem::forget(client);
            }
        }
    }
}
