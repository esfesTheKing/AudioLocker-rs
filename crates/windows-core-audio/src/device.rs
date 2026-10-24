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
struct DeviceSlim {
    device: IMMDevice,

    // Cache
    id: Arc<str>,
    name: Arc<str>,
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

            let id = Self::id(&device)?;
            let name = Self::name(&property_store)?;

            Ok(Self {
                device,
                id: Arc::<str>::from(id),
                name: Arc::<str>::from(name),
            })
        }
    }

    fn name(property_store: &IPropertyStore) -> windows_core::Result<String> {
        let prop_variant = unsafe { property_store.GetValue(&PKEY_Device_FriendlyName)? };

        Ok(prop_variant.to_string())
    }

    fn id(device: &IMMDevice) -> windows_core::Result<String> {
        unsafe {
            let result = RaiiPwstr(device.GetId()?);

            result.to_string()
        }
    }
}

impl Device {
    pub unsafe fn new(device: IMMDevice, config: Arc<Configuration>) -> windows_core::Result<Self> {
        unsafe {
            let slim = DeviceSlim::new(device)?;

            config.register_device(slim.id.clone(), slim.name.clone());

            let interface = slim.device.Activate(CLSCTX_ALL, None)?;
            let session_manager = AudioSessionManager::new(slim.id.clone(), interface, config)?;

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

    pub fn id(&self) -> Arc<str> {
        self.slim.id.clone()
    }

    pub fn name(&self) -> Arc<str> {
        self.slim.name.clone()
    }
}
