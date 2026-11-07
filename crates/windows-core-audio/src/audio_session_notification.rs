use std::sync::Arc;

use windows::Win32::Media::Audio::{IAudioSessionNotification, IAudioSessionNotification_Impl};
use windows_core::implement;

use crate::{AudioSession, WindowsCoreAudioEvent};

#[implement(IAudioSessionNotification)]
pub struct AudioSessionNotification {
    device_id: Arc<str>,

    sender: crossbeam_channel::Sender<WindowsCoreAudioEvent>,
}

impl AudioSessionNotification {
    pub fn new(device_id: Arc<str>, sender: crossbeam_channel::Sender<WindowsCoreAudioEvent>) -> Self {
        Self { device_id, sender }
    }
}

impl IAudioSessionNotification_Impl for AudioSessionNotification_Impl {
    fn OnSessionCreated(
        &self,
        newsession: windows_core::Ref<windows::Win32::Media::Audio::IAudioSessionControl>,
    ) -> windows_core::Result<()> {
        if let Some(interface) = newsession.cloned() {
            let session = AudioSession::new(interface)?;

            // This method only failes when the receiver is disconnected, this should not happen here :)
            let _ = self.sender.send(WindowsCoreAudioEvent::SessionAdded {
                device_id: self.device_id.clone(),
                session,
            });
        }

        Ok(())
    }
}
