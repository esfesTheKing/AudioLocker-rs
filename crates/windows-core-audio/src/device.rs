use std::sync::Arc;

use windows::Win32::{
    Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
    Media::Audio::{EDataFlow, IMMDevice, IMMEndpoint},
    System::Com::{CLSCTX_ALL, STGM_READ},
    UI::Shell::PropertiesSystem::IPropertyStore,
};
use windows_core::Interface;

use crate::{AudioSessionManager, utils::RaiiPwstr};

#[derive(Debug)]
pub struct DeviceSlim {
    device: IMMDevice,

    // Cache
    pub id: Arc<str>,
    pub name: Arc<str>,
}

#[derive(Debug)]
pub struct Device {
    slim: DeviceSlim,
    session_manager: AudioSessionManager,
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl DeviceSlim {
    pub fn new(device: IMMDevice) -> windows_core::Result<Self> {
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
        let device_id = unsafe { device.GetId()? };
        let result = RaiiPwstr(device_id);

        result.to_string()
    }
}

impl Device {
    pub fn new(device: IMMDevice) -> windows_core::Result<Self> {
        let slim = DeviceSlim::new(device)?;

        let interface = unsafe { slim.device.Activate(CLSCTX_ALL, None)? };
        let session_manager = AudioSessionManager::new(slim.name.clone(), interface)?;

        Ok(Self { slim, session_manager })
    }

    pub fn data_flow(&self) -> windows_core::Result<EDataFlow> {
        unsafe { self.slim.device.cast::<IMMEndpoint>()?.GetDataFlow() }
    }
}

impl AsRef<DeviceSlim> for Device {
    fn as_ref(&self) -> &DeviceSlim {
        &self.slim
    }
}

impl AsMut<AudioSessionManager> for Device {
    fn as_mut(&mut self) -> &mut AudioSessionManager {
        &mut self.session_manager
    }
}
