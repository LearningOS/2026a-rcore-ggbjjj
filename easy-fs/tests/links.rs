//! Regression coverage added with OpenAI Codex assistance.
use easy_fs::{BlockDevice, EasyFileSystem};
use std::sync::{Arc, Mutex};

struct MemoryDisk(Mutex<Vec<u8>>);
impl BlockDevice for MemoryDisk {
    fn read_block(&self, block_id: usize, buffer: &mut [u8]) {
        let data = self.0.lock().unwrap();
        buffer.copy_from_slice(&data[block_id * 512..block_id * 512 + buffer.len()]);
    }
    fn write_block(&self, block_id: usize, buffer: &[u8]) {
        let mut data = self.0.lock().unwrap();
        data[block_id * 512..block_id * 512 + buffer.len()].copy_from_slice(buffer);
    }
}

#[test]
fn links_preserve_data_and_unlinked_open_files_delay_inode_reuse() {
    let backing = Arc::new(MemoryDisk(Mutex::new(vec![0; 4096 * 512])));
    let disk: Arc<dyn BlockDevice> = backing.clone();
    let fs = EasyFileSystem::create(disk, 4096, 1);
    let root = EasyFileSystem::root_inode(&fs);
    let file = root.create("original").unwrap();
    file.open();
    let original_id = file.stat().0;
    file.write_at(0, b"retained data");
    assert!(root.link("original", "alias"));
    assert_eq!(file.stat().2, 2);
    assert_eq!(root.find("alias").unwrap().stat().0, original_id);
    assert!(!root.link("original", "original"));
    assert!(root.unlink("original"));
    assert_eq!(file.stat().2, 1);
    assert!(root.unlink("alias"));
    assert_eq!(file.stat().2, 0);
    assert!(root.find("original").is_none());
    let other = root.create("other").unwrap();
    assert_ne!(other.stat().0, original_id);
    let mut content = [0; 13];
    assert_eq!(file.read_at(0, &mut content), 13);
    assert_eq!(&content, b"retained data");
    file.close();
    let reused = root.create("reused").unwrap();
    assert_eq!(reused.stat().0, original_id);
    assert!(!root.unlink("missing"));
    assert!(!root.link("missing", "missing-link"));
    assert!(root.create("").is_none());
    assert!(root
        .create("a-name-longer-than-twenty-seven-bytes")
        .is_none());
    // Exercise direct, single-indirect, and double-indirect block reclamation.
    let large = vec![0x5a; 400_000];
    for _ in 0..5 {
        let tmp = root.create("temporary").unwrap();
        assert_eq!(tmp.write_at(0, &large), large.len());
        assert!(root.unlink("temporary"));
    }
    let mut names = root.ls();
    names.sort();
    assert_eq!(names, vec!["other", "reused"]);
    let payload = b"durability-after-final-inode-drop";
    let durable = root.create("durable").unwrap();
    assert_eq!(durable.write_at(0, payload), payload.len());
    drop(durable);
    assert!(backing.0.lock().unwrap().windows(payload.len()).any(|bytes| bytes == payload));
}
