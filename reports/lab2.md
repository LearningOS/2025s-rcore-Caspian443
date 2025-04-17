# 实验二：内存管理与系统调用

## 实验概述

本次实验主要围绕内存管理与系统调用展开，通过实现多个系统调用增强了操作系统的功能。主要完成了：

1. 虚拟内存管理：实现mmap和munmap系统调用
2. 系统调用跟踪：实现trace系统调用
3. 时间获取：实现get_time系统调用

## 实现细节

### 1. 虚拟内存管理

#### mmap系统调用

`mmap`系统调用允许用户程序创建新的内存映射区域。实现逻辑如下：

```rust
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    // 检查起始地址是否页面对齐
    if start & (PAGE_SIZE - 1) != 0 {
        println!(
            "expect the start address to be aligned with a page, but get an invalid start: {:#x}",
            start
        );
        return -1;
    }
    let start_addr = VirtAddr::from(start);

    // 检查权限位
    if port > 7usize || port == 0 {
        println!("invalid port: {:#b}", port);
        return -1;
    }

    // 构建映射权限
    let permission = MapPermission::from_bits((port as u8) << 1).unwrap() | MapPermission::U;

    // 在任务管理器中执行映射操作
    if TASK_MANAGER.task_mmap(start_addr, len, permission).is_ok() {
        0
    } else {
        println!("mmap failed");
        -1
    }
}
```

#### munmap系统调用

`munmap`系统调用用于取消之前创建的内存映射：

```rust
pub fn sys_munmap(start: usize, len: usize) -> isize {
    if TASK_MANAGER.task_munmap(start, len).is_ok() {
        0
    } else {
        println!("munmap failed");
        -1
    }
}
```

在`TaskManager`中增加了对应的内存管理方法：

```rust
pub fn task_mmap(&self, virt_addr: VirtAddr, len: usize, permission: MapPermission) -> Result<VirtAddr, ()> {
    let mut inner = self.inner.exclusive_access();
    let current = inner.current_task;
    let memory_set = &mut inner.tasks[current].memory_set;

    // 创建新的映射区域
    let start_va = virt_addr;
    let end_va = VirtAddr::from(virt_addr.0 + len);

    // 检查虚拟页号是否已被占用
    let start_vpn = VirtPageNum::from(start_va);
    let end_vpn = VirtPageNum::from(end_va.ceil());
    for vpn in start_vpn.0..end_vpn.0 {
        if let Some(pte) = memory_set.translate(VirtPageNum(vpn)) {
            if pte.is_valid() {
                println!("vpn {} has been occupied!", vpn);
                return Err(());
            }
        }
    }

    // 插入新的映射区域
    memory_set.insert_framed_area(start_va, end_va, permission);
    Ok(start_va)
}

pub fn task_munmap(&self, start: usize, len: usize) -> Result<(), ()> {
    let mut inner = self.inner.exclusive_access();
    let current = inner.current_task;
    let memory_set = &mut inner.tasks[current].memory_set;

    memory_set.unmmap(start, len)
}
```

在`MemorySet`中实现了`unmmap`方法，用于取消映射关系：

```rust
pub fn unmmap(&mut self, start: usize, len: usize) -> Result<(), ()> {
    let va_start: VirtAddr = start.into();
    if !va_start.aligned() {
        debug!("unmap fail don't aligned");
        return Err(());
    }
    let mut va_start: VirtPageNum = va_start.into();

    let va_end: VirtAddr = (start + len).into();
    let va_end: VirtPageNum = va_end.ceil();

    while va_start != va_end {
        if let Some(item) = self.page_table.translate(va_start) {
            if !item.is_valid() {
                debug!("unmap on no map vpn");
                return Err(());
            }
        } else {
            return Err(());
        }
        self.page_table.unmap(va_start);
        va_start.step();
    }
    return Ok(());
}
```

### 2. 系统调用跟踪

实现了`sys_trace`系统调用，用于跟踪系统调用情况：

```rust
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    match trace_request {
        0 => {
            // 检查地址是否在SV39模式下有效
            let bit38 = ((id >> 38) & 1) != 0;
            let high_bits = id >> 39;
            let expected_high_bits = if bit38 { 0x1ffffff } else { 0 };
            if high_bits != expected_high_bits {
                return -1;
            }
            
            // 读取地址id处的字节
            let virt_addr = VirtAddr::from(id);
            if !is_readable(virt_addr) {
                return -1;
            }
            
            // 只在地址有效且可读的情况下执行读取
            if let Some(phys_addr) = virt2phys_addr(virt_addr) {
                unsafe {
                    let addr = phys_addr.0 as *const u8;
                    match core::ptr::read_volatile(addr) {
                        val => val as isize,
                    }
                }
            } else {
                -1
            }
        }
        1 => {
            // 将数据(u8)写入地址id
            let virt_addr = VirtAddr::from(id);
            if !is_writable(virt_addr) {
                return -1;
            }
            
            // 只在地址有效且可写的情况下执行写入
            if let Some(phys_addr) = virt2phys_addr(virt_addr) {
                unsafe {
                    let addr = phys_addr.0 as *mut u8;
                    core::ptr::write_volatile(addr, data as u8);
                }
                0
            } else {
                -1
            }
        }
        2 => {
            // 查询syscall ID的调用次数
            get_syscall_num(id) as isize
        }
        _ => -1,
    }
}
```

并在`TaskControlBlock`中添加了系统调用的计数器：

```rust
pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub call: [SyscallInfo; MAX_SYSCALL_NUM],
    pub memory_set: MemorySet,
    pub trap_cx_ppn: PhysPageNum,
    pub base_size: usize,
    pub task_cx: TaskContext,
}
```

### 3. 时间获取系统调用

实现了`sys_get_time`系统调用，用于获取当前时间：

```rust
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let vir_addr  = VirtAddr::from(ts as usize);
    if let Some(phys_addr) = virt2phys_addr(vir_addr) {
        let us = get_time_us();
        let kernel_ts = phys_addr.0 as *mut TimeVal;
        unsafe {
            *kernel_ts = TimeVal {
                sec: us / 1_000_000,
                usec: us % 1_000_000,
            };
        }
        0
    } else {
        -1
    }
}
```

### 4. 辅助函数实现

为支持上述系统调用，实现了一系列辅助函数：

```rust
// 虚拟地址转物理地址
pub fn virt2phys_addr(virt_addr: VirtAddr) -> Option<PhysAddr> {
    let offset = virt_addr.page_offset();
    let vpn = virt_addr.floor();
    let ppn = PageTable::from_token(current_user_token())
        .translate(vpn)
        .and_then(|pte| {
            if pte.is_valid() {
                Some(pte.ppn())
            } else {
                None
            }
        });

    if let Some(ppn) = ppn {
        Some(PhysAddr::combine(ppn, offset))
    } else {
        println!("virt2phys_addr() fail");
        None
    }
}

// 检查虚拟地址是否可读
pub fn is_readable(virt_addr: VirtAddr) -> bool {
    let vpn = virt_addr.floor();
    PageTable::from_token(current_user_token())
        .translate(vpn)
        .map_or(false, |pte| pte.is_valid() && pte.readable())
}

// 检查虚拟地址是否可写
pub fn is_writable(virt_addr: VirtAddr) -> bool {
    let vpn = virt_addr.floor();
    PageTable::from_token(current_user_token())
        .translate(vpn)
        .map_or(false, |pte| pte.is_valid() && pte.writable())
}
```

## 实验总结

通过本次实验，我深入理解了操作系统中内存管理的机制，特别是虚拟内存和物理内存的映射关系，以及如何实现各种系统调用来支持用户程序的需求。主要收获包括：

1. 掌握了虚拟内存映射的实现方式，包括内存的分配和释放
2. 理解了系统调用的处理流程，以及如何在内核态安全地访问用户态内存
3. 学习了如何跟踪和统计系统调用的使用情况
4. 加深了对页表管理和地址转换过程的理解

这些知识和经验对于理解现代操作系统的内存管理机制非常有帮助，也为后续实验打下了坚实的基础。

## 遇到的问题与解决方案

1. **问题**：在实现`mmap`系统调用时，需要检查虚拟地址是否已被占用
   **解决方案**：通过遍历所有可能被占用的虚拟页号，检查对应的页表项是否有效

2. **问题**：在实现`sys_trace`系统调用时，需要安全地读写用户空间内存
   **解决方案**：首先检查地址的有效性和权限，然后通过虚拟地址转换为物理地址进行操作

3. **问题**：系统调用计数器的实现
   **解决方案**：在`TaskControlBlock`中添加系统调用信息数组，并在每次系统调用时更新计数

通过解决这些问题，不仅完成了实验要求，也加深了对操作系统内存管理和系统调用机制的理解。