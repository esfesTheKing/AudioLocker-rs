use std::sync::Arc;

use crate::AudioSession;

pub enum WindowsCoreAudioEvent {
    DeviceAdded {
        device_id: String,
    },
    DeviceRemoved {
        device_id: String,
    },
    SessionAdded {
        device_id: Arc<str>,
        session: AudioSession,
    },
    SessionClosed {
        device_id: Arc<str>,
        session_name: Arc<str>,
    },
    SessionVolumeChanged {
        device_id: Arc<str>,
        session_name: Arc<str>,
        volume: u8,
    },
    SessionMuteChanged {
        device_id: Arc<str>,
        session_name: Arc<str>,
        mute: bool,
    },
    Exit,
}
