use crate::bindings::audio::ISimpleAudioVolume;

#[derive(Debug)]
pub struct SimpleAudioVolume {
    audio_volume: ISimpleAudioVolume,
}

impl SimpleAudioVolume {
    pub fn new(audio_volume: ISimpleAudioVolume) -> Self {
        Self { audio_volume }
    }

    pub unsafe fn set_volume(&self, v: f32) -> windows_core::Result<()> {
        unsafe { self.audio_volume.SetMasterVolume(v, &windows_core::GUID::zeroed()).ok() }
    }

    #[allow(unused)]
    pub unsafe fn get_volume(&self) -> windows_core::Result<f32> {
        unsafe { self.audio_volume.GetMasterVolume() }
    }

    #[allow(unused)]
    pub unsafe fn is_muted(&self) -> windows_core::Result<bool> {
        unsafe { Ok(self.audio_volume.GetMute()?.as_bool()) }
    }

    #[allow(unused)]
    pub unsafe fn mute(&self) -> windows_core::Result<()> {
        unsafe { self.audio_volume.SetMute(true, &windows_core::GUID::zeroed()).ok() }
    }

    #[allow(unused)]
    pub unsafe fn unmute(&self) -> windows_core::Result<()> {
        unsafe { self.audio_volume.SetMute(false, &windows_core::GUID::zeroed()).ok() }
    }
}
