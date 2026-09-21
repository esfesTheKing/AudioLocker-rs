use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, DEVICE_STATE_NOTPRESENT, DEVICE_STATE_UNPLUGGED, IMMNotificationClient,
    IMMNotificationClient_Impl,
};
use windows_core::implement;

use crate::wrappers::MMEvent;

#[implement(IMMNotificationClient)]
pub struct MMNotificationClient {
    sender: crossbeam_channel::Sender<MMEvent>,
    enabled: AtomicBool,
}

impl MMNotificationClient {
    pub fn new(sender: crossbeam_channel::Sender<MMEvent>, enabled: AtomicBool) -> Self {
        Self { sender, enabled }
    }

    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }
}

impl IMMNotificationClient_Impl for MMNotificationClient_Impl {
    fn OnDeviceStateChanged(
        &self,
        pwstrdeviceid: &windows_core::PCWSTR,
        dwnewstate: windows::Win32::Media::Audio::DEVICE_STATE,
    ) -> windows_core::Result<()> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(());
        }

        match dwnewstate {
            DEVICE_STATE_UNPLUGGED | DEVICE_STATE_NOTPRESENT => self.OnDeviceRemoved(pwstrdeviceid),
            DEVICE_STATE_ACTIVE => self.OnDeviceAdded(pwstrdeviceid),
            _ => {
                log::warn!("Unknown state '{:#?}' for {}", dwnewstate, unsafe {
                    pwstrdeviceid.to_string()?
                });
                Ok(())
            }
        }
    }

    fn OnDeviceAdded(&self, pwstrdeviceid: &windows_core::PCWSTR) -> windows_core::Result<()> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(());
        }

        let event = MMEvent::DeviceAdded(unsafe { pwstrdeviceid.to_string()? });

        // This method only failes when the receiver is disconnected, this should not happen here :)
        if self.sender.send(event).is_err() {
            log::error!(
                "When trying to send `DeviceAdded` event the channel seems to be disconnected, this should never happen."
            )
        }

        Ok(())
    }

    fn OnDeviceRemoved(&self, pwstrdeviceid: &windows_core::PCWSTR) -> windows_core::Result<()> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(());
        }

        let event = MMEvent::DeviceRemoved(unsafe { pwstrdeviceid.to_string()? });

        // This method only failes when the receiver is disconnected, this should not happen here :)
        if self.sender.send(event).is_err() {
            log::error!(
                "When trying to send `DeviceRemoved` event the channel seems to be disconnected, this should never happen."
            )
        }

        Ok(())
    }

    #[allow(unused_variables)]
    fn OnDefaultDeviceChanged(
        &self,
        flow: windows::Win32::Media::Audio::EDataFlow,
        role: windows::Win32::Media::Audio::ERole,
        pwstrdefaultdeviceid: &windows_core::PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn OnPropertyValueChanged(
        &self,
        pwstrdeviceid: &windows_core::PCWSTR,
        key: &windows::Win32::Foundation::PROPERTYKEY,
    ) -> windows_core::Result<()> {
        Ok(())
    }
}
