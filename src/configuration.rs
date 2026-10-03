use std::{
    collections::{HashMap, hash_map::Entry},
    fs::OpenOptions,
    io::{self, BufReader, BufWriter},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::Arc,
};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use windows::Win32::{
    Storage::EnhancedStorage::PKEY_Software_ProductName,
    UI::Shell::{IShellItem2, SHCreateItemFromParsingName},
};
use windows_core::HSTRING;

use crate::constants::DEFAULT_VOLUME_LEVEL;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AudioSessionConfiguration {
    #[serde(rename = "VolumeLevel")]
    pub volume_level: u8,
    #[serde(rename = "IsManual")]
    pub is_manual: bool,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct DeviceConfiguration(HashMap<String, AudioSessionConfiguration>);

#[derive(Serialize, Deserialize, Debug)]
struct AudioConfiguration {
    default_volume_level: u8,
    config: HashMap<Arc<str>, DeviceConfiguration>,
    device_id_to_device_name: HashMap<Arc<str>, Arc<str>>,
}

impl Drop for AudioConfiguration {
    fn drop(&mut self) {
        log::debug!("Dropping audio configuration");
    }
}

#[derive(Debug)]
pub struct Configuration {
    data: RwLock<AudioConfiguration>,
    config_path: PathBuf,

    // TODO: is this really the best place to store this struct?
    system_information: RwLock<System>,
}

fn get_process_name_from_file_information(path: &str) -> Option<String> {
    unsafe {
        SHCreateItemFromParsingName(&HSTRING::from(path), None)
            .and_then(|shell_item: IShellItem2| shell_item.GetString(&PKEY_Software_ProductName))
            .and_then(|prop| prop.to_string().map_err(windows_core::Error::from))
            .ok()
    }
}

impl Configuration {
    pub fn from_file(path: PathBuf, create: bool) -> io::Result<Self> {
        let audio_configuration = match Self::factory_read_from_disk(&path) {
            Ok(v) => Ok(v),
            Err(error) => match error.kind() {
                io::ErrorKind::NotFound => {
                    if !create {
                        return Err(error);
                    }

                    Self::create_new(&path)
                }

                _ => Err(error),
            },
        }?;
        let system = System::new_with_specifics(RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()));

        Ok(Self {
            data: RwLock::new(audio_configuration),
            config_path: path,
            system_information: RwLock::new(system),
        })
    }

    pub fn get_process_name(&self, process_id: u32) -> Option<String> {
        let pid = Pid::from_u32(process_id);

        let mut system_information = self.system_information.write();
        system_information.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::OnlyIfNotSet),
        );

        let process = system_information.process(pid)?;

        process
            .exe()
            .and_then(Path::to_str)
            .and_then(get_process_name_from_file_information)
            .or_else(|| {
                let string = process.name().to_str()?;

                Some(string.strip_suffix(".exe").unwrap_or(string).to_owned())
            })
    }

    fn factory_read_from_disk(path: &Path) -> io::Result<AudioConfiguration> {
        let file: std::fs::File = OpenOptions::new().read(true).open(path)?;
        let reader = BufReader::new(file);

        Ok(serde_json::from_reader(reader)?)
    }

    fn create_new(path: &Path) -> io::Result<AudioConfiguration> {
        let config = AudioConfiguration {
            default_volume_level: DEFAULT_VOLUME_LEVEL,
            config: HashMap::new(),
            device_id_to_device_name: HashMap::new(),
        };

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .share_mode(0)
            .open(path)?;

        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &config)?;

        Ok(config)
    }

    pub fn get_file_path(&self) -> &Path {
        &self.config_path
    }

    pub fn get_device_name(&self, device_id: &str) -> Option<Arc<str>> {
        self.data.read().device_id_to_device_name.get(device_id).cloned()
    }

    pub fn get_session_config(&self, device_id: &str, session_id: &str) -> Option<AudioSessionConfiguration> {
        let device_name = &self.get_device_name(device_id)? as &str;

        self.data.read().config.get(device_name)?.0.get(session_id).cloned()
    }

    pub fn register_device(&self, device_id: Arc<str>, device_name: Arc<str>) {
        let mut data = self.data.write();

        match data.device_id_to_device_name.entry(device_id) {
            Entry::Occupied(mut entry) => {
                // Migrate device config from old name to new name.
                // If a device config with the new name already exists, override it with the old name's config.

                let old_device_name = entry.insert(device_name.clone());
                if let Some(device_config) = data.config.remove(&old_device_name as &str) {
                    data.config.insert(device_name, device_config);
                }
            }
            Entry::Vacant(entry) => {
                // Add entry to mapping, if device config exists use it.

                entry.insert_entry(device_name.clone());

                let device_config = data.config.remove(&device_name as &str).unwrap_or_default();
                data.config.insert(device_name, device_config);
            }
        }
    }

    pub fn register_session(&self, device_id: &str, session_id: String) -> Option<()> {
        let default_volume_level = self.data.read().default_volume_level;
        let device_name = &self.get_device_name(device_id)? as &str;

        let mut data = self.data.write();

        data.config
            .get_mut(device_name)?
            .0
            .entry(session_id)
            .or_insert_with(|| AudioSessionConfiguration {
                volume_level: default_volume_level,
                is_manual: false,
            });

        Some(())
    }

    pub fn read_from_disk(&self) -> io::Result<()> {
        let mut data = self.data.write();

        *data = Self::factory_read_from_disk(&self.config_path)?;

        Ok(())
    }

    pub fn write_to_disk(&self) -> io::Result<()> {
        // Acquire writing lock for writing to disk
        let data = self.data.write();

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .share_mode(0)
            .open(&self.config_path)?;

        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &*data)?;

        Ok(())
    }
}

impl Drop for Configuration {
    fn drop(&mut self) {
        if let Err(error) = self.write_to_disk() {
            log::warn!("Error while writing configuration to disk: {error:#?}");
        }
    }
}
