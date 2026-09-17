use std::sync::Arc;

use windows::{
    Win32::Media::Audio::{AudioSessionStateExpired, IAudioSessionEvents, IAudioSessionEvents_Impl},
    core::implement,
};

use crate::wrappers::AudioSession;

pub enum AudioSessionEvents {
    NewSession(AudioSession),
    VolumeChanged(Arc<str>, u8),
    MuteStateChanged(Arc<str>, bool),
    SessionClosed(Arc<str>),
    Exit,
}

#[implement(IAudioSessionEvents)]
pub struct AudioSessionEventsHandler {
    name: Arc<str>,
    sender: crossbeam_channel::Sender<AudioSessionEvents>,
}

impl AudioSessionEventsHandler {
    pub fn new(name: Arc<str>, sender: crossbeam_channel::Sender<AudioSessionEvents>) -> Self {
        Self { name, sender }
    }
}

impl IAudioSessionEvents_Impl for AudioSessionEventsHandler_Impl {
    #[allow(unused)]
    fn OnDisplayNameChanged(
        &self,
        newdisplayname: &windows_core::PCWSTR,
        eventcontext: *const windows_core::GUID,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    #[allow(unused)]
    fn OnIconPathChanged(
        &self,
        newiconpath: &windows_core::PCWSTR,
        eventcontext: *const windows_core::GUID,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    fn OnSimpleVolumeChanged(
        &self,
        newvolume: f32,
        newmute: windows_core::BOOL,
        _eventcontext: *const windows_core::GUID,
    ) -> windows_core::Result<()> {
        // This method only failes when the receiver is disconnected, this should not happen here :)
        let _ = self.sender.send(AudioSessionEvents::VolumeChanged(
            self.name.clone(),
            (newvolume * 100.0).clamp(0.0, 100.0) as u8,
        ));

        // This method only failes when the receiver is disconnected, this should not happen here :)
        let _ = self.sender.send(AudioSessionEvents::MuteStateChanged(
            self.name.clone(),
            newmute.as_bool(),
        ));

        Ok(())
    }

    #[allow(unused)]
    fn OnChannelVolumeChanged(
        &self,
        channelcount: u32,
        newchannelvolumearray: *const f32,
        changedchannel: u32,
        eventcontext: *const windows_core::GUID,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    #[allow(unused)]
    fn OnGroupingParamChanged(
        &self,
        newgroupingparam: *const windows_core::GUID,
        eventcontext: *const windows_core::GUID,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    fn OnStateChanged(&self, newstate: windows::Win32::Media::Audio::AudioSessionState) -> windows_core::Result<()> {
        if newstate == AudioSessionStateExpired {
            // This method only failes when the receiver is disconnected, this should not happen here :)
            let _ = self.sender.send(AudioSessionEvents::SessionClosed(self.name.clone()));
        }

        Ok(())
    }

    fn OnSessionDisconnected(
        &self,
        _disconnectreason: windows::Win32::Media::Audio::AudioSessionDisconnectReason,
    ) -> windows_core::Result<()> {
        // This method only failes when the receiver is disconnected, this should not happen here :)
        let _ = self.sender.send(AudioSessionEvents::SessionClosed(self.name.clone()));

        Ok(())
    }
}
