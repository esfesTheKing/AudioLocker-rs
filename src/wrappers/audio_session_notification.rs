use windows::Win32::Media::Audio::{IAudioSessionNotification, IAudioSessionNotification_Impl};
use windows_core::implement;

use crate::wrappers::{AudioSession, audio_session_events_handler::AudioSessionEvents};

#[implement(IAudioSessionNotification)]
pub struct AudioSessionNotification {
    sender: crossbeam_channel::Sender<AudioSessionEvents>,
}

impl AudioSessionNotification {
    pub fn new(sender: crossbeam_channel::Sender<AudioSessionEvents>) -> Self {
        Self { sender }
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
            let _ = self.sender.send(AudioSessionEvents::NewSession(session));
        }

        Ok(())
    }
}
