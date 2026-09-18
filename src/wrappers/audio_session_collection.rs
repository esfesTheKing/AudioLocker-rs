use crate::bindings::audio::IAudioSessionEnumerator;
use crate::wrappers::AudioSession;

pub struct AudioSessionCollection {
    enumerator: IAudioSessionEnumerator,
}

pub struct AudioSessionCollectionIterator<'a> {
    collection: &'a AudioSessionCollection,
    index: i32,
}

impl AudioSessionCollection {
    pub fn new(enumerator: IAudioSessionEnumerator) -> Self {
        Self { enumerator }
    }

    pub fn len(&self) -> windows_core::Result<i32> {
        unsafe { self.enumerator.GetCount() }
    }

    pub fn iter(&self) -> AudioSessionCollectionIterator<'_> {
        AudioSessionCollectionIterator {
            collection: self,
            index: 0,
        }
    }
}

impl Iterator for AudioSessionCollectionIterator<'_> {
    type Item = AudioSession;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.collection.len().ok()? {
            return None;
        }

        unsafe {
            let session = self
                .collection
                .enumerator
                .GetSession(self.index)
                .and_then(AudioSession::new)
                .ok();

            self.index += 1;

            session
        }
    }
}
