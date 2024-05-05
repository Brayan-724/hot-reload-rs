use std::io;
use std::path::PathBuf;

use super::utils::hash_dir;

#[derive(Debug)]
pub struct PollWatcher {
    will_drop: bool,
    folder: PathBuf,
    hashed_files: u64,
}

impl PollWatcher {
    pub fn new(folder: PathBuf) -> io::Result<PollWatcher> {
        let hashed_files = hash_dir(&folder)?;

        Ok(PollWatcher {
            will_drop: false,
            folder,
            hashed_files,
        })
    }

    pub fn poll(&mut self) -> bool {
        let Ok(new_hash) = hash_dir(&self.folder) else {
            eprintln!("[WATCHER] Error polling changes on {:#?}", self.folder);
            return false;
        };

        let changed = self.hashed_files != new_hash;

        self.hashed_files = new_hash;

        changed
    }
}

impl Drop for PollWatcher {
    fn drop(&mut self) {
        self.will_drop = true;
    }
}
