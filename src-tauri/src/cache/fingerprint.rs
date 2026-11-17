//! The identity of a video's content, independent of its path: its size and a hash of its first
//! and last 64 KiB. Cheap enough for every new file, and stable across renames and moves.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const CHUNK: u64 = 64 * 1024;

pub fn fingerprint(path: &Path, size: u64) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; CHUNK as usize];

    let head = read_up_to(&mut file, &mut buffer)?;
    hasher.update(&buffer[..head]);

    if size > CHUNK {
        let tail_start = (size - CHUNK).max(CHUNK);
        file.seek(SeekFrom::Start(tail_start))?;
        let tail = read_up_to(&mut file, &mut buffer)?;
        hasher.update(&buffer[..tail]);
    }

    let hash = hasher.finalize().to_hex();
    Ok(format!("{}-{}", size, &hash[..32]))
}

fn read_up_to(file: &mut File, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        let n = file.read(&mut buffer[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn fp(dir: &TempDir, name: &str, content: &[u8]) -> String {
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        fingerprint(&path, content.len() as u64).unwrap()
    }

    #[test]
    fn test_same_content_same_fingerprint() {
        let dir = TempDir::new().unwrap();
        assert_eq!(fp(&dir, "a", b"hello"), fp(&dir, "b", b"hello"));
        assert_ne!(fp(&dir, "c", b"hello"), fp(&dir, "d", b"hellp"));
    }

    #[test]
    fn test_large_file_tail_counts() {
        let dir = TempDir::new().unwrap();
        let mut a = vec![7u8; 300_000];
        let first = fp(&dir, "a", &a);
        *a.last_mut().unwrap() = 8;
        assert_ne!(first, fp(&dir, "b", &a));
    }

    #[test]
    fn test_includes_size() {
        let dir = TempDir::new().unwrap();
        assert!(fp(&dir, "a", b"abc").starts_with("3-"));
    }
}
