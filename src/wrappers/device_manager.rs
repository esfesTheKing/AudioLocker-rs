use std::{collections::HashMap, sync::Arc, thread};

use parking_lot::RwLock;
use windows::Win32::Media::Audio::{IMMNotificationClient, eRender};

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
        let notification_client = Some(IMMNotificationClient::from(MMNotificationClient::new(sender.clone())));

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
        // This will fail only if the receiver (in the thread) has been dropped, i.e. something paniced inside the thread :(
        let _ = self.sender.send(MMEvent::Exit);

        let thread_handle = self.thread_handle.take();
        // SAFETY: thread_handle is set always set in new()
        if let Err(error) = thread_handle.unwrap().join() {
            log::debug!("The thread paniced internaly for some reason, additional info: {error:#?}");
        }

        // SAFETY:
        //  both possible errors from this method call are unlikely to happen:
        //      1. E_POINTER - this will only happen if self.notification_client is set to None, it's value is set in the new() function to Some.
        //      2. E_NOTFOUND - this will only happen when trying to unregister a notification_client that was not registered with this IMMDeviceEnumerator.
        //
        //  links to docs: https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nf-mmdeviceapi-immdeviceenumerator-unregisterendpointnotificationcallback
        if let Err(error) = unsafe { self.enumerator.unregister_client(self.notification_client.as_ref()) } {
            log::error!("Error occured when unregistering client due to {error:#?}");
        }
    }
}
