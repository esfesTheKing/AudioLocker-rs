mod audio_session;
mod audio_session_collection;
mod audio_session_events_handler;
mod audio_session_manager;
mod audio_session_notification;
mod com_guard;
mod device;
mod device_collection;
mod device_enumerator;
mod event;
mod mmnotification_client;
mod simple_audio_volume;
mod utils;

pub(crate) use audio_session_collection::AudioSessionCollection;
pub(crate) use audio_session_manager::AudioSessionManager;
pub(crate) use com_guard::ComInitializationGuard;
pub(crate) use device_collection::DeviceCollection;
pub(crate) use simple_audio_volume::SimpleAudioVolume;

pub use audio_session::AudioSession;
pub use audio_session_events_handler::AudioSessionEventsHandler;
pub use audio_session_notification::AudioSessionNotification;
pub use device::{Device, DeviceSlim};
pub use device_enumerator::DeviceEnumerator;
pub use event::WindowsCoreAudioEvent;
pub use mmnotification_client::MMNotificationClient;
