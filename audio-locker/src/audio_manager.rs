use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
    thread,
};

use windows::Win32::Media::Audio::{IMMNotificationClient, eRender};
use windows_core::AsImpl;

use crate::configuration::Configuration;
use windows_core_audio::{
    AudioSession, AudioSessionEventsHandler, AudioSessionNotification, Device, DeviceEnumerator, DeviceSlim,
    MMNotificationClient, SameThreadUsed, WindowsCoreAudioEvent,
};

struct DeviceStore {
    _device: Device,
    sessions: HashMap<Arc<str>, AudioSession>,
}

pub struct AudioManager {
    config: Arc<Configuration>,

    sender: crossbeam_channel::Sender<WindowsCoreAudioEvent>,
    thread_handle: Option<thread::JoinHandle<()>>,

    notification_client: Option<IMMNotificationClient>,
    enumerator: DeviceEnumerator,
}

impl AudioManager {
    pub fn new(config: Arc<Configuration>) -> windows_core::Result<Self> {
        let (sender, receiver) = crossbeam_channel::unbounded::<WindowsCoreAudioEvent>();
        let enumerator = DeviceEnumerator::new()?;

        let notification_client = Some(IMMNotificationClient::from(MMNotificationClient::new(
            sender.clone(),
            AtomicBool::new(true),
        )));

        enumerator.register_client(notification_client.as_ref())?;

        let thread_handle = {
            let config = config.clone();
            let sender = sender.clone();

            thread::spawn(move || {
                let mut devices: HashMap<Arc<str>, DeviceStore> = HashMap::new();

                let enumerator: DeviceEnumerator = match DeviceEnumerator::new() {
                    Ok(v) => v,
                    Err(error) => {
                        log::error!("Unable to create a device enumerator in the background thread due to {error:#?}");
                        return;
                    }
                };

                while let Ok(event) = receiver.recv() {
                    match event {
                        WindowsCoreAudioEvent::DeviceAdded(device_id) => {
                            if devices.contains_key(device_id.as_str()) {
                                continue;
                            }

                            let mut device = match enumerator.get_device_by_id(device_id.as_str()) {
                                Ok(device) => device,
                                Err(error) => {
                                    log::error!("Unable to get device from `{device_id}` due to {error:#?}");
                                    continue;
                                }
                            };

                            let data_flow = match device.data_flow() {
                                Ok(data_flow) => data_flow,
                                Err(error) => {
                                    log::error!("Got error while retrieving data_flow for `{device_id}`: {error}");
                                    continue;
                                }
                            };
                            if data_flow != eRender {
                                continue;
                            }

                            let device_slim: &DeviceSlim = device.as_ref();

                            let device_name = device_slim.name.clone();
                            let device_id = device_slim.id.clone();

                            if device_name.is_empty() {
                                log::error!("Device `{device_id}` has empty name, stopping initializiation");
                                continue;
                            }

                            log::info!("[{device_name}] Initializing device");
                            let audio_session_manager = device.as_mut();

                            let notification = AudioSessionNotification::new(device_id.clone(), sender.clone());
                            // TODO:
                            let _ = audio_session_manager.register_notifications(notification);

                            // TODO:
                            //  1. unwrap
                            //  2. Verify that AudioSession is on the same thread (here is 100% correct, but for new session notifications we need to verify - maybe add a runtime verification step)
                            for session in audio_session_manager.get_sessions().unwrap().iter() {
                                let _ = sender.send(WindowsCoreAudioEvent::SessionAdded(
                                    device_id.clone(),
                                    SameThreadUsed::new(session),
                                ));
                            }

                            config.register_device(device_id.clone(), device_name.clone());
                            devices.insert(
                                device.as_ref().id.clone(),
                                DeviceStore {
                                    _device: device,
                                    sessions: HashMap::new(),
                                },
                            );
                            log::info!("[{device_name}] Finished initializing device");
                        }
                        WindowsCoreAudioEvent::DeviceRemoved(device_id) => {
                            let _ = devices.remove(device_id.as_str());

                            if let Some(device_name) = config.get_device_name(&device_id) {
                                log::info!("[{device_name}] Removed device");
                            }
                        }
                        WindowsCoreAudioEvent::SessionAdded(device_id, session) => {
                            let mut session = session.unlock().unwrap();

                            if let Some(store) = devices.get_mut(&device_id) {
                                let session_name: Arc<str> = Arc::from(session.name());

                                let event_handler = AudioSessionEventsHandler::new(
                                    device_id.clone(),
                                    session_name.clone(),
                                    sender.clone(),
                                );
                                // TODO: handle
                                let _ = session.register_event_handler(event_handler);

                                // TODO: unwrap
                                store.sessions.insert(session_name.clone(), session);

                                let entry = config.register_session(&device_id, session_name.to_string()).unwrap();
                                // TODO: unwrap
                                let _ = sender.send(WindowsCoreAudioEvent::SessionVolumeChanged(
                                    device_id.clone(),
                                    session_name.clone(),
                                    entry.volume_level,
                                ));
                            }
                        }
                        WindowsCoreAudioEvent::SessionClosed(device_id, session_name) => {
                            if let Some(store) = devices.get_mut(&device_id) {
                                store.sessions.remove(&session_name);
                            }
                        }
                        WindowsCoreAudioEvent::SessionVolumeChanged(device_id, session_name, volume) => {
                            if let Some(store) = devices.get(&device_id)
                                && let Some(session) = store.sessions.get(&session_name)
                                && let Some(config_entry) = config.get_session_config(&device_id, &session_name)
                                && volume != config_entry.volume_level
                            {
                                // TODO:
                                let _ = session.set_volume(config_entry.volume_level);
                            }
                        }
                        WindowsCoreAudioEvent::SessionMuteChanged(_, _, _) => {
                            // We do not care about the mute state of a session
                        }
                        WindowsCoreAudioEvent::Exit => return,
                    }
                }
            })
        };

        Ok(Self {
            config,
            sender,
            thread_handle: Some(thread_handle),
            notification_client,
            enumerator,
        })
    }

    pub fn initialize_devices(&self) -> windows_core::Result<()> {
        for device in self.enumerator.devices()?.iter() {
            // TODO
            let _ = self.sender.send(WindowsCoreAudioEvent::DeviceAdded(
                device.as_ref().id.clone().to_string(),
            ));
        }

        Ok(())
    }

    pub fn on_configuration_change(&self) {
        log::info!("[Configuration] Detected change from file");
        if let Err(error) = self.config.read_from_disk() {
            log::warn!("[Configuration] unable to read file: {error}");
            return;
        }
        // TODO:

        log::info!("[Configuration] propagating change to devices");
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        // Stop handling notifications during shutdown
        if let Some(client) = self.notification_client.as_ref() {
            unsafe { client.as_impl() }.disable();
        }

        if self.sender.send(WindowsCoreAudioEvent::Exit).is_err() {
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
        if let Some(client) = self.notification_client.take()
            && let Err(error) = self.enumerator.unregister_client(Some(&client))
        {
            log::error!("Error occurred when unregistering client due to {error:#?}");

            // The enumerator may still hold a raw pointer to the client; leak it rather than
            // free an object the OS could still call into.
            std::mem::forget(client);
        }
    }
}
