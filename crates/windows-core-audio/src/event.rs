use std::sync::Arc;

use crate::{AudioSession};

pub enum WindowsCoreAudioEvent {
    DeviceAdded(String),
    DeviceRemoved(String),
    SessionAdded(Arc<str>, AudioSession),
    SessionClosed(Arc<str>, Arc<str>),
    SessionVolumeChanged(Arc<str>, Arc<str>, u8),
    SessionMuteChanged(Arc<str>, Arc<str>, bool),
    Exit,
}
