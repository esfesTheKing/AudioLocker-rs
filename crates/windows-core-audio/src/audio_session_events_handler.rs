use std::sync::Arc;

use windows::{
    Win32::Media::Audio::{AudioSessionStateExpired, IAudioSessionEvents, IAudioSessionEvents_Impl},
    core::implement,
};

use crate::WindowsCoreAudioEvent;

#[implement(IAudioSessionEvents)]
pub struct AudioSessionEventsHandler {
    device_id: Arc<str>,
    session_name: Arc<str>,
    sender: crossbeam_channel::Sender<WindowsCoreAudioEvent>,
}

impl AudioSessionEventsHandler {
    pub fn new(
        device_id: Arc<str>,
        session_name: Arc<str>,
        sender: crossbeam_channel::Sender<WindowsCoreAudioEvent>,
    ) -> Self {
        Self {
            device_id,
            session_name,
            sender,
        }
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
        let _ = self.sender.send(WindowsCoreAudioEvent::SessionVolumeChanged {
            device_id: self.device_id.clone(),
            session_name: self.session_name.clone(),
            volume: (newvolume * 100.0).clamp(0.0, 100.0) as u8,
        });

        // This method only failes when the receiver is disconnected, this should not happen here :)
        let _ = self.sender.send(WindowsCoreAudioEvent::SessionMuteChanged {
            device_id: self.device_id.clone(),
            session_name: self.session_name.clone(),
            mute: newmute.as_bool(),
        });

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
            let _ = self.sender.send(WindowsCoreAudioEvent::SessionClosed {
                device_id: self.device_id.clone(),
                session_name: self.session_name.clone(),
            });
        }

        Ok(())
    }

    fn OnSessionDisconnected(
        &self,
        _disconnectreason: windows::Win32::Media::Audio::AudioSessionDisconnectReason,
    ) -> windows_core::Result<()> {
        // This method only failes when the receiver is disconnected, this should not happen here :)
        let _ = self.sender.send(WindowsCoreAudioEvent::SessionClosed {
            device_id: self.device_id.clone(),
            session_name: self.session_name.clone(),
        });

        Ok(())
    }
}
