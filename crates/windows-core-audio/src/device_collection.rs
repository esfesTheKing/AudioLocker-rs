use windows::Win32::Media::Audio::IMMDeviceCollection;

use crate::Device;

pub struct DeviceCollection {
    collection: IMMDeviceCollection,
}

pub struct DeviceCollectionIterator<'a> {
    collection: &'a DeviceCollection,
    index: u32,
}

impl DeviceCollection {
    pub fn new(collection: IMMDeviceCollection) -> Self {
        Self { collection }
    }

    pub fn len(&self) -> windows_core::Result<u32> {
        unsafe { self.collection.GetCount() }
    }

    pub fn iter(&self) -> DeviceCollectionIterator<'_> {
        DeviceCollectionIterator {
            collection: self,
            index: 0,
        }
    }
}

impl Iterator for DeviceCollectionIterator<'_> {
    type Item = Device;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.collection.len().ok()? {
            return None;
        }

        unsafe {
            let session = self
                .collection
                .collection
                .Item(self.index)
                .and_then(|interface| Device::new(interface))
                .ok();

            self.index += 1;

            session
        }
    }
}
