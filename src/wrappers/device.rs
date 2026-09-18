use std::sync::Arc;

use windows_core::Interface;

use crate::{
    bindings::{
        audio::{
            EDataFlow, IMMDevice, IMMEndpoint, IPropertyStore, PKEY_Device_FriendlyName, PropVariantClear,
            PropVariantToStringAlloc,
        },
        com::{CLSCTX_ALL, STGM_READ},
    },
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
            let property_store = device.OpenPropertyStore(STGM_READ as u32)?;

            Ok(Self { property_store, device })
        }
    }

    pub unsafe fn name(&self) -> windows_core::Result<String> {
        unsafe {
            let mut prop_variant = self.property_store.GetValue(&PKEY_Device_FriendlyName)?;
            let pwstr = RaiiPwstr(PropVariantToStringAlloc(&prop_variant)?);

            PropVariantClear(&mut prop_variant).ok()?;

            pwstr.to_string()
        }
    }
}

impl Device {
    pub unsafe fn new(device: IMMDevice, config: Arc<Configuration>) -> windows_core::Result<Self> {
        unsafe {
            let slim = DeviceSlim::new(device)?;

            let interface = slim.device.Activate(CLSCTX_ALL as u32, None)?;
            let session_manager = AudioSessionManager::new(slim.name()?, interface, config)?;

            Ok(Self { slim, session_manager })
        }
    }

    pub unsafe fn on_configuration_change(&self) {
        self.session_manager.on_configuration_change()
    }

    pub unsafe fn initialize_sessions(&self) -> windows_core::Result<()> {
        unsafe { self.session_manager.initialize_sessions() }
    }

    pub unsafe fn name(&self) -> windows_core::Result<String> {
        unsafe { self.slim.name() }
    }

    pub unsafe fn data_flow(&self) -> windows_core::Result<EDataFlow> {
        unsafe { self.slim.device.cast::<IMMEndpoint>()?.GetDataFlow() }
    }
}
