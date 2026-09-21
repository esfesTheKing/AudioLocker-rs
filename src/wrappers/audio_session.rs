use windows::Win32::{
    Foundation::S_OK,
    Media::Audio::{IAudioSessionControl, IAudioSessionControl2, IAudioSessionEvents},
};
use windows_core::Interface;

use crate::wrappers::{AudioSessionEventsHandler, SimpleAudioVolume, utils::RaiiPwstr};

#[derive(Debug)]
pub struct AudioSession {
    events_handler: Option<IAudioSessionEvents>,
    volume_control: SimpleAudioVolume,
    session: IAudioSessionControl2,
}

// Allow sending between threads.
// NOTE:
//  It's never explicitly mentioned in the docs that we can share IAudioSessionControl pointers between threads when using COM in MTA.
//  But we can assume that we can, as it's mentioned that we must call .Release() on the interface from the same thread that owned
//  the interface from the start.
//  https://learn.microsoft.com/en-us/windows/win32/api/audiopolicy/nn-audiopolicy-iaudiosessioncontrol

unsafe impl Send for AudioSession {}
unsafe impl Sync for AudioSession {}

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

    pub fn display_name(&self) -> windows_core::Result<String> {
        unsafe {
            let pwstr = RaiiPwstr(self.session.GetDisplayName()?);

            let return_value = pwstr.to_string()?;

            Ok(return_value)
        }
    }

    pub fn identifier(&self) -> windows_core::Result<String> {
        unsafe {
            let pwstr = RaiiPwstr(self.session.GetSessionIdentifier()?);

            let return_value = pwstr.to_string()?;

            Ok(return_value)
        }
    }

    pub fn set_volume(&self, volume: u8) -> windows_core::Result<()> {
        unsafe { self.volume_control.set_volume(volume as f32 / 100.0) }
    }

    pub fn get_volume(&self) -> windows_core::Result<u8> {
        unsafe { self.volume_control.get_volume().map(|volume| (volume * 100.) as u8) }
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
        if self.events_handler.is_none() {
            // TODO: instead of only using Self::display_name(), refactor to have the same logic for getting the session name as in AudioSessionManager::initailize_session
            log::debug!(
                "It's a little bit sus that we didn't register an events handler for `{}`",
                self.display_name().unwrap_or_else(|_| "unknown".to_string())
            );
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
