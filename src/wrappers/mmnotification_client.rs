use windows_core::implement;

use crate::{
    bindings::audio::{
        DEVICE_STATE_ACTIVE, DEVICE_STATE_NOTPRESENT, DEVICE_STATE_UNPLUGGED, EDataFlow, ERole, IMMNotificationClient,
        IMMNotificationClient_Impl, PROPERTYKEY,
    },
    wrappers::MMEvent,
};

#[implement(IMMNotificationClient)]
pub struct MMNotificationClient {
    sender: crossbeam_channel::Sender<MMEvent>,
}

impl MMNotificationClient {
    pub fn new(sender: crossbeam_channel::Sender<MMEvent>) -> Self {
        Self { sender }
    }
}

impl IMMNotificationClient_Impl for MMNotificationClient_Impl {
    fn OnDeviceStateChanged(&self, pwstrdeviceid: &windows_core::PCWSTR, dwnewstate: u32) -> windows_core::Result<()> {
        match dwnewstate as i32 {
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
        flow: EDataFlow,
        role: ERole,
        pwstrdefaultdeviceid: &windows_core::PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn OnPropertyValueChanged(
        &self,
        pwstrdeviceid: &windows_core::PCWSTR,
        key: &PROPERTYKEY,
    ) -> windows_core::Result<()> {
        Ok(())
    }
}
