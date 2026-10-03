use std::sync::Arc;

use windows::Win32::{
    Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
    Media::Audio::{EDataFlow, IMMDevice, IMMEndpoint},
    System::Com::{CLSCTX_ALL, STGM_READ},
    UI::Shell::PropertiesSystem::IPropertyStore,
};
use windows_core::Interface;

use crate::{
    configuration::Configuration,
    wrappers::{AudioSessionManager, utils::RaiiPwstr},
};

#[derive(Debug)]
pub struct DeviceSlim {
    property_store: IPropertyStore,
    device: IMMDevice,
}

#[derive(Debug)]
pub struct Device {
    slim: DeviceSlim,
    session_manager: AudioSessionManager,
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl DeviceSlim {
    pub unsafe fn new(device: IMMDevice) -> windows_core::Result<Self> {
        unsafe {
            let property_store = device.OpenPropertyStore(STGM_READ)?;

            Ok(Self { property_store, device })
        }
    }

    pub unsafe fn name(&self) -> windows_core::Result<String> {
        let prop_variant = unsafe { self.property_store.GetValue(&PKEY_Device_FriendlyName)? };

        Ok(prop_variant.to_string())
    }

    pub fn id(&self) -> windows_core::Result<String> {
        unsafe {
            let result = RaiiPwstr(self.device.GetId()?);

            result.to_string()
        }
    }
}

impl Device {
    pub unsafe fn new(device: IMMDevice, config: Arc<Configuration>) -> windows_core::Result<Self> {
        unsafe {
            let slim = DeviceSlim::new(device)?;

            let device_id: Arc<str> = Arc::from(slim.id()?);
            let device_name = slim.name()?;
            config.register_device(device_id.clone(), device_name);

            let interface = slim.device.Activate(CLSCTX_ALL, None)?;
            let session_manager = AudioSessionManager::new(device_id, interface, config)?;

            Ok(Self { slim, session_manager })
        }
    }

    pub unsafe fn on_configuration_change(&self) {
        self.session_manager.on_configuration_change()
    }

    pub unsafe fn initialize_sessions(&self) -> windows_core::Result<()> {
        unsafe { self.session_manager.initialize_sessions() }
    }

    pub unsafe fn data_flow(&self) -> windows_core::Result<EDataFlow> {
        unsafe { self.slim.device.cast::<IMMEndpoint>()?.GetDataFlow() }
    }
}

// Allows calling DeviceSlim`s methods from Device automatically
impl std::ops::Deref for Device {
    type Target = DeviceSlim;

    fn deref(&self) -> &Self::Target {
        &self.slim
    }
}
