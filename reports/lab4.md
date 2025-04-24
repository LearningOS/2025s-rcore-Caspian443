# 操作系统实验四报告

## 实验概述

本实验主要完成了文件系统相关功能的扩展实现，包括硬链接、取消链接和文件状态获取等功能。这些功能丰富了操作系统的文件系统接口，提高了文件管理的灵活性和功能性。

## 实现细节

### 1. 硬链接功能 (sys_linkat)

实现了 `sys_linkat` 系统调用，允许创建指向已存在文件的硬链接：

```rust
pub fn sys_linkat(old_name: *const u8, new_name: *const u8) -> isize {
    let token = current_user_token();
    let old = translated_str(token, old_name);
    let new = translated_str(token, new_name);
    println!("link {} to {}", new , old);
    if old.as_str() != new.as_str() {
        if let Some(_) = ROOT_INODE.link(old.as_str(), new.as_str()) {
            return 0;
        }
    }
    -1
}
```

在 `easy-fs` 中添加了 `link` 方法实现硬链接创建：

```rust
pub fn link(&self, old: &str, new: &str) -> Option<Arc<Inode>> {
    let mut fs: MutexGuard<'_, EasyFileSystem> = self.fs.lock();
    // 确认旧文件存在并获取其inode_id
    let op = |root_inode: &DiskInode| {
        assert!(root_inode.is_dir());
        self.find_inode_id(old, root_inode)
    };
    if let Some(old_inode_id) = self.read_disk_inode(op) {
        // 创建指向同一inode的新目录项
        let new_inode_id = old_inode_id;
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        self.modify_disk_inode(|root_inode| {
            // 在目录中添加新条目
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            self.increase_size(new_size as u32, root_inode, &mut fs);
            let dirent = DirEntry::new(new, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });
        
        // 返回新创建的Inode对象
        Some(Arc::new(Self::new(
            new_inode_block_id,
            new_inode_block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
    } else {
        None
    }
}
```

### 2. 取消链接功能 (sys_unlinkat)

实现了 `sys_unlinkat` 系统调用，用于删除文件链接：

```rust
pub fn sys_unlinkat(name: *const u8) -> isize {
    let token = current_user_token();
    let name = translated_str(token, name);
    if let Some(inode) = ROOT_INODE.find(name.as_str()) {
        if ROOT_INODE.get_link_num(inode.block_id, inode.block_offset) == 1 {
            // 如果只有一个链接，则清除数据
            inode.clear();
        }
        return ROOT_INODE.unlink(name.as_str());
    }
    -1
}
```

在 `easy-fs` 中添加了 `unlink` 方法实现链接删除：

```rust
pub fn unlink(&self, name: &str) -> isize {
    let _fs = self.fs.lock();
    let op = |root_inode: &DiskInode| {
        assert!(root_inode.is_dir());
        self.find_inode_id(name, root_inode)
    };
    
    if let Some(_) = self.read_disk_inode(op) {
        self.modify_disk_inode(|root_inode| {
            let mut buf = DirEntry::empty();
            let mut swap = DirEntry::empty();
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            for i in 0..file_count {
                if root_inode.read_at(DIRENT_SZ * i, buf.as_bytes_mut(), &self.block_device) == DIRENT_SZ {
                    if buf.name() == name {
                        // 通过用最后一个目录项覆盖当前目录项来实现删除
                        root_inode.read_at(DIRENT_SZ *(file_count - 1), swap.as_bytes_mut(), &self.block_device);
                        root_inode.write_at(DIRENT_SZ * i, swap.as_bytes_mut(), &self.block_device);
                        root_inode.size -= DIRENT_SZ as u32;
                        break;
                    }
                }
            }
        });
        0
    } else {
        -1
    }
}
```

### 3. 获取文件信息 (sys_fstat)

实现了 `sys_fstat` 系统调用，用于获取文件状态信息：

```rust
pub fn sys_fstat(fd: usize, st: *mut Stat) -> isize {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();

    // 检查文件描述符有效性
    if fd >= inner.fd_table.len() || inner.fd_table[fd].is_none() {
        return -1;
    }

    // 获取文件inode信息
    let ino: u64;
    let nlink: u32;
    if let Some(file_node) = &inner.fd_table[fd] {
        let any: &dyn Any = file_node.as_any();
        let os_node = any.downcast_ref::<OSInode>().unwrap();
        ino = os_node.get_inode_id();
        let (block_id, block_offset) = os_node.get_inode_pos();
        nlink = ROOT_INODE.get_link_num(block_id, block_offset);
    } else {
        return -1;
    }

    // 构造Stat结构并复制到用户空间
    let stat = &Stat {
        dev: 0,
        ino: ino,
        mode: StatMode::FILE,
        nlink: nlink,
        pad: [0;7],
    };

    let token = inner.get_user_token();
    let st = translated_byte_buffer(token, st as *const u8, core::mem::size_of::<Stat>());
    let stat_ptr = stat as *const _ as *const u8;
    for (idx, byte) in st.into_iter().enumerate() {
        unsafe {
            byte.copy_from_slice(core::slice::from_raw_parts(
                stat_ptr.wrapping_byte_add(idx), byte.len())
            );
        }
    }
    0
}
```

### 4. 获取链接数量函数

为支持文件链接计数，实现了 `get_link_num` 方法：

```rust
pub fn get_link_num(&self, block_id: usize, block_offset: usize) -> u32 {
    let fs = self.fs.lock();
    let mut count = 0;
    self.read_disk_inode(|root_inode| {
        let mut buf = DirEntry::empty();
        let file_count = (root_inode.size as usize) / DIRENT_SZ;
        for i in 0..file_count {
            assert_eq!(
                root_inode.read_at(DIRENT_SZ * i, buf.as_bytes_mut(), &self.block_device),
                DIRENT_SZ,
            );
            let (this_inode_block_id, this_inode_block_offset) = fs.get_disk_inode_pos(buf.inode_id());
            if this_inode_block_id as usize == block_id && this_inode_block_offset == block_offset {
                count += 1;
            }
        }
    });
    count
}
```

## 文件系统架构增强

为了支持上述功能，对文件系统结构进行了一系列增强：

1. 扩展了 `Inode` 结构，公开了 `block_id` 和 `block_offset` 字段
2. 为 `OSInode` 添加了额外方法获取 inode 信息
3. 增加了 `AnyConvertor` trait 以支持运行时类型转换
4. 公开了 `ROOT_INODE` 使其可以直接被系统调用使用

```rust
pub trait AnyConvertor {
    fn as_any(&self) -> &dyn Any;
}

impl<T: 'static> AnyConvertor for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
```

## 挑战与解决方案

1. **文件系统结构理解**：一开始对文件系统的整体架构不够熟悉，特别是 `easy-fs` 与内核接口的关系。通过阅读源码并绘制结构图解决了这个问题。

2. **硬链接实现**：硬链接需要维护对相同 inode 的多个引用，而不是复制文件内容。通过确保新的链接指向相同的 inode 实现了这一点。

3. **文件状态获取**：`sys_fstat` 需要处理文件描述符到文件信息的映射，并安全地将数据传递回用户空间。通过使用 `as_any` 和类型转换解决了这一问题。

## 实验总结

本次实验成功实现了文件系统的高级功能，包括硬链接、文件链接删除和文件状态获取。这些功能极大地增强了文件系统的实用性，使其更接近现代操作系统的标准功能集。通过本次实验，我深入理解了文件系统的内部结构，特别是文件管理和链接机制的实现原理。

实验中还实现了一些辅助功能，如安全的类型转换和文件信息获取，这些功能对于构建健壮的文件系统至关重要。总体而言，本次实验加深了我对操作系统文件系统层的理解，也增强了实现复杂系统功能的能力。