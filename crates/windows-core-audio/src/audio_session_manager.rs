use std::{
    collections::{HashMap, hash_map::Entry},
    sync::Arc,
    thread,
};

use parking_lot::RwLock;
use windows::Win32::Media::Audio::{IAudioSessionManager2, IAudioSessionNotification};

use crate::{
    configuration::Configuration,
    {
        AudioSession, AudioSessionCollection, AudioSessionEventsHandler, AudioSessionNotification,
        audio_session_events_handler::AudioSessionEvents,
    },
};

#[derive(Debug)]
pub struct AudioSessionManager {
    config: Arc<Configuration>,

    sender: crossbeam_channel::Sender<AudioSessionEvents>,
    thread_handle: Option<thread::JoinHandle<()>>,

    sessions: Arc<RwLock<HashMap<Arc<str>, AudioSession>>>,
    notification: Option<IAudioSessionNotification>,
    manager: IAudioSessionManager2,
    device_id: Arc<str>,
}

impl AudioSessionManager {
    pub unsafe fn new(
        device_id: Arc<str>,
        manager: IAudioSessionManager2,
        config: Arc<Configuration>,
    ) -> windows_core::Result<Self> {
        let sessions = Arc::new(RwLock::new(HashMap::new()));

        let (sender, receiver) = crossbeam_channel::unbounded::<AudioSessionEvents>();

        let thread_handle = {
            let device_name = config.get_device_name(&device_id).unwrap(); // TODO: fix unwrap
            let device_id = device_id.clone();
            let config = config.clone();
            let sender = sender.clone();
            let sessions = sessions.clone();

            thread::spawn(move || {
                while let Ok(event) = receiver.recv() {
                    match event {
                        AudioSessionEvents::NewSession(session) => {
                            if let Err(error) = Self::initailize_session(
                                &device_id,
                                &device_name as &str,
                                &config,
                                &sender,
                                &sessions,
                                session,
                            ) {
                                log::warn!("[{device_name}] Failed to initialize session due to {error:#?}");
                            }
                        }
                        AudioSessionEvents::VolumeChanged(session_name, new_volume) => {
                            if let Some(session) = sessions.read().get(session_name.as_ref()) {
                                let audio_config = match config.get_session_config(&device_id, &session_name) {
                                    Some(audio_config) => audio_config,
                                    None => {
                                        log::debug!(
                                            "AudioSessionManager: unable to retrieve config for session `{session_name}` in device `{device_name}`"
                                        );
                                        continue;
                                    }
                                };

                                if audio_config.is_manual {
                                    continue;
                                }

                                if new_volume == audio_config.volume_level {
                                    continue;
                                }

                                match session.set_volume(audio_config.volume_level) {
                                    Ok(_) => log::info!(
                                        "[{device_name}] [{session_name}] volume level was set from {new_volume} to {}",
                                        audio_config.volume_level
                                    ),
                                    Err(error) => log::info!(
                                        "[{device_name}] [{session_name}] failed to set volume level: {error:#?}"
                                    ),
                                }
                            }
                        }
                        AudioSessionEvents::MuteStateChanged(_, _) => {}
                        AudioSessionEvents::SessionClosed(session_name) => {
                            let _ = sessions.write().remove(session_name.as_ref());

                            log::info!("[{device_name}] [{session_name}] application closed");
                        }
                        AudioSessionEvents::Exit => return,
                    }
                }
            })
        };

        let mut manager = Self {
            config,
            sender,
            thread_handle: Some(thread_handle),
            sessions,
            manager,
            notification: None,
            device_id,
        };

        let notification = AudioSessionNotification::new(manager.sender.clone());

        unsafe {
            manager.register_notifications(notification)?;
        }

        Ok(manager)
    }

    pub unsafe fn initialize_sessions(&self) -> windows_core::Result<()> {
        unsafe {
            let device_name = &self.config.get_device_name(&self.device_id).unwrap() as &str;

            self.get_sessions()?.iter().try_for_each(|session| {
                Self::initailize_session(
                    &self.device_id,
                    device_name,
                    &self.config,
                    &self.sender,
                    &self.sessions,
                    session,
                )
            })
        }
    }

    pub unsafe fn get_sessions(&self) -> windows_core::Result<AudioSessionCollection> {
        unsafe { Ok(AudioSessionCollection::new(self.manager.GetSessionEnumerator()?)) }
    }

    pub fn on_configuration_change(&self) {
        self.sessions.read().iter().for_each(|(session_name, session)| {
            if let Ok(volume_level) = session.get_volume() {
                let _ = self
                    .sender
                    .send(AudioSessionEvents::VolumeChanged(session_name.clone(), volume_level));
            }
        });
    }

    unsafe fn register_notifications(&mut self, notification: AudioSessionNotification) -> windows_core::Result<()> {
        if self.notification.is_none() {
            let interface = Some(IAudioSessionNotification::from(notification));

            unsafe { self.manager.RegisterSessionNotification(interface.as_ref())? }

            self.notification = interface;
        }

        Ok(())
    }

    unsafe fn unregister_notifications(&mut self) -> windows_core::Result<()> {
        let device_name = self.config.get_device_name(&self.device_id).unwrap(); // TODO: unwrap
        let notification = self.notification.take();
        if notification.is_none() {
            log::debug!("[{device_name}] doesn't have a session notification handler, nothing to do...");

            return Ok(());
        }

        unsafe { self.manager.UnregisterSessionNotification(notification.as_ref())? }

        log::debug!("[{device_name}] Unregistering session notification handler");
        Ok(())
    }

    fn initailize_session(
        device_id: &str,
        device_name: &str,
        config: &Arc<Configuration>,
        sender: &crossbeam_channel::Sender<AudioSessionEvents>,
        sessions: &Arc<RwLock<HashMap<Arc<str>, AudioSession>>>,
        mut session: AudioSession,
    ) -> windows_core::Result<()> {
        let session_name = match session.is_system_sound() {
            true => String::from("System Sounds"),
            false => {
                let display_name = session.display_name()?;
                if display_name.is_empty() {
                    config.get_process_name(session.process_id()?).unwrap_or(display_name)
                } else {
                    display_name
                }
            }
        };

        let session_name = Arc::<str>::from(session_name);

        if let Entry::Vacant(entry) = sessions.write().entry(session_name.clone()) {
            log::info!("[{device_name}] [{session_name}] Initializing session");

            config
                .register_session(device_id, session_name.to_string())
                .ok_or_else(|| windows_core::Error::new(windows_core::HRESULT(-1), "Device not registered"))?;

            let events_handler = AudioSessionEventsHandler::new(session_name.clone(), sender.clone());
            session.register_event_handler(events_handler)?;

            let session_volume = session.get_volume()?;

            entry.insert_entry(session);

            // NOTE:
            //  1. We can only alert after the session is registered in the configratuion and added to `sessions`.
            //  2. This method only failes when the receiver is disconnected, this should not happen here :)
            let _ = sender.send(AudioSessionEvents::VolumeChanged(session_name.clone(), session_volume));
            log::info!("[{device_name}] [{session_name}] Finished initializing session");
        }

        Ok(())
    }
}

impl Drop for AudioSessionManager {
    fn drop(&mut self) {
        let device_name = self.config.get_device_name(&self.device_id).unwrap(); // TODO: unwrap

        if let Err(_) = self.sender.send(AudioSessionEvents::Exit) {
            log::error!("AudioSessionManager: receiver has been dropped inside internal thread for `{device_name}`");
        }

        if let Some(thread_handle) = self.thread_handle.take() {
            match thread_handle.join() {
                Ok(_) => log::debug!("AudioSessionManager: Thread exited cleanly"),
                Err(error) => log::error!(
                    "AudioSessionManager: internal thread has paniced for `{device_name}` with error {error:#?}"
                ),
            }
        } else {
            log::error!("AudioSessionManager: thread_handle was not set for `{device_name}`");
        }

        if let Err(error) = unsafe { self.unregister_notifications() } {
            log::error!(
                "AudioSessionManager: an error occured while unregistering notification client for `{device_name}` with error {error:#?}"
            );
        }
    }
}
