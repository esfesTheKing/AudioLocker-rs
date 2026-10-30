use std::sync::Arc;

use windows::Win32::Media::Audio::{IAudioSessionManager2, IAudioSessionNotification};

use crate::{AudioSessionCollection, AudioSessionNotification};

#[derive(Debug)]
pub struct AudioSessionManager {
    notification: Option<IAudioSessionNotification>,
    manager: IAudioSessionManager2,
    device_id: Arc<str>,
    device_name: Arc<str>,
}

impl AudioSessionManager {
    pub fn new(
        device_id: Arc<str>,
        device_name: Arc<str>,
        manager: IAudioSessionManager2,
    ) -> windows_core::Result<Self> {
        let manager = Self {
            manager,
            notification: None,
            device_id,
            device_name,
        };

        Ok(manager)
    }

    pub fn get_sessions(&self) -> windows_core::Result<AudioSessionCollection> {
        unsafe { Ok(AudioSessionCollection::new(self.manager.GetSessionEnumerator()?)) }
    }

    fn register_notifications(&mut self, notification: AudioSessionNotification) -> windows_core::Result<()> {
        if self.notification.is_none() {
            let interface = Some(IAudioSessionNotification::from(notification));

            unsafe { self.manager.RegisterSessionNotification(interface.as_ref())? }

            self.notification = interface;
        }

        Ok(())
    }

    fn unregister_notifications(&mut self) -> windows_core::Result<()> {
        let device_name = &self.device_name;

        let notification = self.notification.take();
        if notification.is_some() {
            unsafe { self.manager.UnregisterSessionNotification(notification.as_ref())? }

            log::debug!("[{device_name}] Unregistering session notification handler");
        } else {
            log::debug!("[{device_name}] doesn't have a session notification handler, nothing to do...");
        }

        Ok(())
    }
}

impl Drop for AudioSessionManager {
    fn drop(&mut self) {
        if let Err(error) = self.unregister_notifications() {
            log::error!(
                "AudioSessionManager: an error occured while unregistering notification client for `{}` with error {error:#?}",
                self.device_name
            );
        }
    }
}
