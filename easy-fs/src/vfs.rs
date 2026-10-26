use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};
/// Virtual filesystem layer over easy-fs
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    // AI assistance: OpenAI Codex. Reference: rCore Guide chapter 6 hard links.
    /// Register an open file description for this inode.
    pub fn open(&self) {
        let mut fs = self.fs.lock();
        let id = fs.get_disk_inode_id(self.block_id, self.block_offset);
        *fs.open_inodes.entry(id).or_insert(0) += 1;
    }
    /// Close a description and reclaim an unlinked inode on the final close.
    pub fn close(&self) {
        let mut fs = self.fs.lock();
        let id = fs.get_disk_inode_id(self.block_id, self.block_offset);
        if let Some(count) = fs.open_inodes.get_mut(&id) {
            *count -= 1;
            if *count != 0 {
                return;
            }
            fs.open_inodes.remove(&id);
            if fs.orphans.remove(&id) {
                self.reclaim(id, &mut fs);
            }
        }
    }
    fn reclaim(&self, id: u32, fs: &mut MutexGuard<EasyFileSystem>) {
        self.modify_disk_inode(|inode| {
            for block in inode.clear_size(&self.block_device) {
                fs.dealloc_data(block);
            }
        });
        fs.dealloc_inode(id);
        block_cache_sync_all();
    }
    /// Return the inode number, directory flag, and directory link count.
    pub fn stat(&self) -> (u32, bool, u32) {
        let fs = self.fs.lock();
        let id = fs.get_disk_inode_id(self.block_id, self.block_offset);
        let (block, offset) = fs.get_disk_inode_pos(0);
        let root = Self::new(block, offset, self.fs.clone(), self.block_device.clone());
        let links = root.read_disk_inode(|inode| {
            let mut count = 0;
            for index in 0..inode.size as usize / DIRENT_SZ {
                let mut entry = DirEntry::empty();
                inode.read_at(index * DIRENT_SZ, entry.as_bytes_mut(), &self.block_device);
                if !entry.name().is_empty() && entry.inode_id() == id {
                    count += 1;
                }
            }
            count
        });
        (id, self.read_disk_inode(|inode| inode.is_dir()), links)
    }
    /// Add a directory entry sharing the inode named by old.
    pub fn link(&self, old: &str, new: &str) -> bool {
        if old == new || new.is_empty() || new.len() > 27 || new.contains('/') {
            return false;
        }
        let mut fs = self.fs.lock();
        let Some(id) = self.read_disk_inode(|inode| self.find_inode_id(old, inode)) else {
            return false;
        };
        if self
            .read_disk_inode(|inode| self.find_inode_id(new, inode))
            .is_some()
        {
            return false;
        }
        self.modify_disk_inode(|inode| {
            let count = inode.size as usize / DIRENT_SZ;
            let mut slot = count;
            for index in 0..count {
                let mut entry = DirEntry::empty();
                inode.read_at(index * DIRENT_SZ, entry.as_bytes_mut(), &self.block_device);
                if entry.name().is_empty() {
                    slot = index;
                    break;
                }
            }
            self.increase_size(((slot + 1) * DIRENT_SZ) as u32, inode, &mut fs);
            inode.write_at(
                slot * DIRENT_SZ,
                DirEntry::new(new, id).as_bytes(),
                &self.block_device,
            );
        });
        block_cache_sync_all();
        true
    }
    /// Remove a directory entry and reclaim its inode when no references remain.
    pub fn unlink(&self, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let mut fs = self.fs.lock();
        let Some(id) = self.read_disk_inode(|inode| self.find_inode_id(name, inode)) else {
            return false;
        };
        let remaining = self.modify_disk_inode(|inode| {
            let mut remaining = 0;
            for index in 0..inode.size as usize / DIRENT_SZ {
                let mut entry = DirEntry::empty();
                inode.read_at(index * DIRENT_SZ, entry.as_bytes_mut(), &self.block_device);
                if entry.name() == name {
                    inode.write_at(
                        index * DIRENT_SZ,
                        DirEntry::empty().as_bytes(),
                        &self.block_device,
                    );
                } else if !entry.name().is_empty() && entry.inode_id() == id {
                    remaining += 1;
                }
            }
            remaining
        });
        if remaining == 0 {
            if fs.open_inodes.get(&id).copied().unwrap_or(0) != 0 {
                fs.orphans.insert(id);
            } else {
                let (block, offset) = fs.get_disk_inode_pos(id);
                let inode = Self::new(block, offset, self.fs.clone(), self.block_device.clone());
                inode.reclaim(id, &mut fs);
            }
        }
        block_cache_sync_all();
        true
    }

    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    /// Call a function over a disk inode to read it
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }
    /// Find inode under a disk inode by name
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if !name.is_empty() && dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }
    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }
    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }
    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        if name.is_empty() || name.len() > 27 || name.contains('/') {
            return None;
        }
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let mut slot = file_count;
            for index in 0..file_count {
                let mut entry = DirEntry::empty();
                root_inode.read_at(index * DIRENT_SZ, entry.as_bytes_mut(), &self.block_device);
                if entry.name().is_empty() {
                    slot = index;
                    break;
                }
            }
            let new_size = (root_inode.size as usize).max((slot + 1) * DIRENT_SZ);
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(slot * DIRENT_SZ, dirent.as_bytes(), &self.block_device);
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
        // release efs lock automatically by compiler
    }
    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                if !dirent.name().is_empty() {
                    v.push(String::from(dirent.name()));
                }
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        // Buffered writes become durable on cache eviction or inode close.
        // Avoid flushing unrelated metadata on every small userspace write.
        size
    }
    /// Clear the data in current inode
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
}

impl Drop for Inode {
    fn drop(&mut self) {
        // Includes the host filesystem packer, which writes through Inode directly.
        block_cache_sync_all();
    }
}
