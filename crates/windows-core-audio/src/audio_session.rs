use windows::Win32::{
    Foundation::S_OK,
    Media::Audio::{IAudioSessionControl, IAudioSessionControl2, IAudioSessionEvents},
};
use windows_core::Interface;

use crate::{
    AudioSessionEventsHandler, SimpleAudioVolume,
    utils::{RaiiPwstr, get_session_name},
};

#[derive(Debug)]
pub struct AudioSession {
    events_handler: Option<IAudioSessionEvents>,
    volume_control: SimpleAudioVolume,
    session: IAudioSessionControl2,
}

impl AudioSession {
    pub fn new(audio_session: IAudioSessionControl) -> windows_core::Result<Self> {
        unsafe { Self::internal_new(audio_session.cast()?) }
    }

    unsafe fn internal_new(session: IAudioSessionControl2) -> windows_core::Result<Self> {
        let volume_control = SimpleAudioVolume::new(session.cast()?);

        Ok(Self {
            session,
            volume_control,
            events_handler: None,
        })
    }

    pub fn is_system_sound(&self) -> bool {
        unsafe { self.session.IsSystemSoundsSession() == S_OK }
    }

    pub fn name(&self) -> String {
        get_session_name(self).unwrap_or_else(|_| "unknown".to_string())
    }

    pub fn display_name(&self) -> windows_core::Result<String> {
        let display_name = unsafe { self.session.GetDisplayName()? };
        let pwstr = RaiiPwstr(display_name);

        pwstr.to_string()
    }

    pub fn identifier(&self) -> windows_core::Result<String> {
        let session_id = unsafe { self.session.GetSessionIdentifier()? };
        let pwstr = RaiiPwstr(session_id);

        pwstr.to_string()
    }

    pub fn set_volume(&self, volume: u8) -> windows_core::Result<()> {
        self.volume_control.set_volume(f32::from(volume) / 100.0)
    }

    pub fn get_volume(&self) -> windows_core::Result<u8> {
        let volume = self.volume_control.get_volume()?;

        // https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-isimpleaudiovolume-getmastervolume
        if (0.0..1.0).contains(&volume) {
            unreachable!("Volume range is promised to be between 0.0 and 1.0");
        }

        Ok((volume * 100.) as u8)
    }

    pub fn process_id(&self) -> windows_core::Result<u32> {
        unsafe { self.session.GetProcessId() }
    }

    pub fn register_event_handler(&mut self, events_handler: AudioSessionEventsHandler) -> windows_core::Result<()> {
        if self.events_handler.is_none() {
            let events_handler = Some(IAudioSessionEvents::from(events_handler));

            unsafe { self.session.RegisterAudioSessionNotification(events_handler.as_ref())? }

            self.events_handler = events_handler;
        }
        Ok(())
    }

    fn unregister_event_handler(&mut self) {
        let name = self.name();

        if self.events_handler.is_none() {
            log::debug!("It's a little bit sus that we didn't register an events handler for `{name}`");
            return;
        }

        unsafe {
            let _ = self
                .session
                .UnregisterAudioSessionNotification(self.events_handler.as_ref());
        }

        self.events_handler.take();
    }
}

impl Drop for AudioSession {
    fn drop(&mut self) {
        log::debug!("Dropping audio sesion");

        self.unregister_event_handler()
    }
}
