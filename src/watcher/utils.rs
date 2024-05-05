#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::{fs, hash::BuildHasher, io, path::PathBuf};

use super::MAIN_HASHER;

static MEGABYTE: u64 = 1000000;
static FILE_SIZE_THRESHOLD: u64 = MEGABYTE * 3;

fn hash_file(path: &PathBuf) -> u64 {
    let c = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    MAIN_HASHER.with(|m| m.hash_one(c))
}

fn get_hash(path: &PathBuf) -> u64 {
    let size = match fs::metadata(&path) {
        #[cfg(unix)]
        Ok(m) => m.size(),

        #[cfg(windows)]
        Ok(m) => m.file_size(),

        #[cfg(not(any(unix, windows)))]
        Ok(_) => return hash_file(path),

        Err(_) => return hash_file(path),
    };

    if size >= FILE_SIZE_THRESHOLD {
        size
    } else {
        hash_file(path)
    }
}

pub fn hash_dir(cwd: &PathBuf) -> io::Result<u64> {
    let dir = DirIterator::new(fs::read_dir(cwd)?);
    let mut total_hash: u64 = 0;

    for entry in dir.into_iter() {
        let entry = entry.path();

        if is_valid_filename(&entry) {
            let hash = get_hash(&entry.clone()) / 100;
            total_hash = total_hash.wrapping_add(hash);
        }
    }

    Ok(total_hash)
}

fn is_valid_filename(path: &PathBuf) -> bool {
    let is_nvim_cache = path.display().to_string().ends_with('~');
    let is_nvim_file = path.ends_with("4913");
    let is_git = path
        .components()
        .find(|c| c.as_os_str() == ".git")
        .is_some();

    !is_nvim_cache && !is_nvim_file && !is_git
}

pub struct DirIterator {
    stack: Vec<fs::DirEntry>,
    current: fs::ReadDir,
}

impl DirIterator {
    pub fn new(dir: fs::ReadDir) -> DirIterator {
        DirIterator {
            stack: Vec::new(),
            current: dir,
        }
    }
}

impl Iterator for DirIterator {
    type Item = fs::DirEntry;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(entry) = self.current.next().and_then(|f| f.ok()) {
                if entry.file_type().ok()?.is_dir() {
                    self.stack.push(entry);
                } else {
                    return Some(entry);
                }
            } else {
                let dir = self.stack.pop()?;
                self.current = fs::read_dir(dir.path()).ok()?;
            }
        }
    }
}
