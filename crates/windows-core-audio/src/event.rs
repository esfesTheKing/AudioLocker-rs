use std::sync::Arc;

use crate::{AudioSession, utils::SameThreadUsed};

pub enum WindowsCoreAudioEvent {
    DeviceAdded(String),
    DeviceRemoved(String),
    SessionAdded(Arc<str>, SameThreadUsed<AudioSession>),
    SessionClosed(Arc<str>, Arc<str>),
    SessionVolumeChanged(Arc<str>, Arc<str>, u8),
    SessionMuteChanged(Arc<str>, Arc<str>, bool),
    Exit,
}
