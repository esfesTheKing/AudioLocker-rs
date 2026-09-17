use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{self, BufReader, BufWriter},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
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
    config: HashMap<String, DeviceConfiguration>,
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

    pub fn get_process_name(&self, process_id: u32) -> String {
        let mut system_information = self.system_information.write();
        system_information.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::OnlyIfNotSet),
        );

        let process = system_information.process(Pid::from_u32(process_id)).unwrap();

        process
            .exe()
            .and_then(Path::to_str)
            .and_then(get_process_name_from_file_information)
            .unwrap_or_else(|| {
                let string = process.name().to_str().unwrap();

                string
                    .strip_suffix(".exe")
                    .map(str::to_owned)
                    .unwrap_or(string.to_string())
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

    pub fn get_session_config(&self, device_id: &str, session_id: &str) -> Option<AudioSessionConfiguration> {
        self.data.read().config.get(device_id)?.0.get(session_id).cloned()
    }

    pub fn register_session(&self, device_id: String, session_id: String) -> io::Result<()> {
        let default_volume_level = self.data.read().default_volume_level;

        self.data
            .write()
            .config
            .entry(device_id)
            .or_default()
            .0
            .entry(session_id)
            .or_insert_with(|| AudioSessionConfiguration {
                volume_level: default_volume_level,
                is_manual: false,
            });

        Ok(())
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
