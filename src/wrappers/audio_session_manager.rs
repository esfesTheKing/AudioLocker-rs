use std::{
    collections::{HashMap, hash_map::Entry},
    sync::Arc,
    thread,
};

use parking_lot::RwLock;
use windows::Win32::Media::Audio::{IAudioSessionManager2, IAudioSessionNotification};

use crate::{
    configuration::Configuration,
    wrappers::{
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
    device_name: String,
}

impl AudioSessionManager {
    pub unsafe fn new(
        device_name: String,
        manager: IAudioSessionManager2,
        config: Arc<Configuration>,
    ) -> windows_core::Result<Self> {
        let sessions = Arc::new(RwLock::new(HashMap::new()));

        let (sender, receiver) = crossbeam_channel::unbounded::<AudioSessionEvents>();

        let thread_handle = {
            let device_name = device_name.clone();
            let config = config.clone();
            let sender = sender.clone();
            let sessions = sessions.clone();

            thread::spawn(move || {
                while let Ok(event) = receiver.recv() {
                    match event {
                        AudioSessionEvents::NewSession(session) => {
                            if let Err(error) =
                                Self::initailize_session(&device_name, &config, &sender, &sessions, session)
                            {
                                log::warn!("[{device_name}] Failed to initialize session due to {error:#?}");
                            }
                        }
                        AudioSessionEvents::VolumeChanged(session_name, new_volume) => {
                            if let Some(session) = sessions.read().get(session_name.as_ref()) {
                                let audio_config = config.get_session_config(&device_name, &session_name).unwrap();
                                if audio_config.is_manual {
                                    continue;
                                }

                                if new_volume != audio_config.volume_level {
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
            device_name,
        };

        let notification = AudioSessionNotification::new(manager.sender.clone());

        unsafe {
            manager.register_notifications(notification)?;
        }

        Ok(manager)
    }

    pub unsafe fn initialize_sessions(&self) -> windows_core::Result<()> {
        unsafe {
            self.get_sessions()?.iter().try_for_each(|session| {
                Self::initailize_session(&self.device_name, &self.config, &self.sender, &self.sessions, session)
            })
        }
    }

    pub unsafe fn get_sessions(&self) -> windows_core::Result<AudioSessionCollection> {
        unsafe { Ok(AudioSessionCollection::new(self.manager.GetSessionEnumerator()?)) }
    }

    pub fn on_configuration_change(&self) {
        self.sessions.read().iter().for_each(|(session_name, session)| {
            let volume_level = session.get_volume().unwrap();
            let _ = self
                .sender
                .send(AudioSessionEvents::VolumeChanged(session_name.clone(), volume_level));
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
        let notification = self.notification.take();
        if notification.is_none() {
            log::debug!(
                "[{}] doesn't have a session notification handler, nothing to do...",
                self.device_name
            );

            return Ok(());
        }

        unsafe { self.manager.UnregisterSessionNotification(notification.as_ref())? }

        log::debug!("[{}] Unregistering session notification handler", self.device_name);
        Ok(())
    }

    fn initailize_session(
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
            config.register_session(device_name.to_owned(), session_name.to_string())?;

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
        if let Err(_) = self.sender.send(AudioSessionEvents::Exit) {
            log::debug!(
                "AudioSessionManager: receiver has been dropped inside internal thread for `{}`",
                self.device_name
            );
        }

        if let Some(thread_handle) = self.thread_handle.take() {
            let result = thread_handle.join();
            log::debug!("AudioSessionManager: Thread exited cleanly: {}", result.is_ok());
        } else {
            log::debug!(
                "AudioSessionManager: thread_handle was not set for `{}`",
                self.device_name
            );
        };

        if let Err(error) = unsafe { self.unregister_notifications() } {
            log::error!(
                "AudioSessionManager: an error occured while unregistering notification client for `{}` with error {error:#?}",
                self.device_name
            );
        }
    }
}
