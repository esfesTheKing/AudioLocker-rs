use std::sync::Arc;

use windows::Win32::{
    Media::Audio::{
        DEVICE_STATE_ACTIVE, IMMDeviceEnumerator, IMMNotificationClient, MMDeviceEnumerator as MMDeviceEnumeratorGuid,
        eRender,
    },
    System::Com::{CLSCTX_ALL, CoCreateInstance},
};
use windows_core::PCWSTR;

use crate::{
    configuration::Configuration,
    constants::UTF16_NULL_TERMINATOR_LITTERAL,
    wrappers::{ComInitializationGuard, Device, DeviceCollection, DeviceSlim},
};

pub struct DeviceEnumerator {
    config: Arc<Configuration>,
    enumerator: IMMDeviceEnumerator,
    _com_guard: ComInitializationGuard,
}

impl DeviceEnumerator {
    pub unsafe fn new(config: Arc<Configuration>) -> windows_core::Result<Self> {
        unsafe {
            let com_guard = ComInitializationGuard::new()?;
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumeratorGuid, None, CLSCTX_ALL)?;

            Ok(Self {
                config,
                enumerator,
                _com_guard: com_guard,
            })
        }
    }

    pub unsafe fn get_device_slim_by_id(&self, device_id: &str) -> windows_core::Result<DeviceSlim> {
        let temporary_buffer: Vec<u16> = device_id.encode_utf16().chain(UTF16_NULL_TERMINATOR_LITTERAL).collect();
        let pcwstr = PCWSTR(temporary_buffer.as_ptr());

        unsafe {
            let device = self.enumerator.GetDevice(pcwstr)?;

            DeviceSlim::new(device)
        }
    }

    pub unsafe fn get_device_by_id(&self, device_id: &str) -> windows_core::Result<Device> {
        let temporary_buffer: Vec<u16> = device_id.encode_utf16().chain(UTF16_NULL_TERMINATOR_LITTERAL).collect();
        let pcwstr = PCWSTR(temporary_buffer.as_ptr());

        unsafe {
            let device = self.enumerator.GetDevice(pcwstr)?;

            Device::new(device, self.config.clone())
        }
    }

    pub unsafe fn devices(&self) -> windows_core::Result<DeviceCollection> {
        unsafe {
            let collection = self.enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;

            Ok(DeviceCollection::new(collection, self.config.clone()))
        }
    }

    pub unsafe fn register_client(&self, client: Option<&IMMNotificationClient>) -> windows_core::Result<()> {
        unsafe {
            self.enumerator.RegisterEndpointNotificationCallback(client)?;
        }

        Ok(())
    }

    pub unsafe fn unregister_client(&self, client: Option<&IMMNotificationClient>) -> windows_core::Result<()> {
        unsafe {
            self.enumerator.UnregisterEndpointNotificationCallback(client)?;
        }

        Ok(())
    }
}
