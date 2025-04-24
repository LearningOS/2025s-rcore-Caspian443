//! Process management syscalls
use alloc::{sync::Arc, vec::Vec};

use crate::{
    config::{PAGE_SIZE, TRAP_CONTEXT_BASE}, fs::{open_file, File, OpenFlags}, mm::{translated_refmut, translated_str, virt2phys_addr, MapPermission, MemorySet, VirtAddr, KERNEL_SPACE}, sync::UPSafeCell, task::{
        add_task, current_task, current_user_token, exit_current_and_run_next, kstack_alloc, pid_alloc, suspend_current_and_run_next, TaskContext, TaskControlBlock, TaskControlBlockInner, TaskStatus
    }, timer::get_time_us, trap::{trap_handler, TrapContext}

};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx: &mut crate::trap::TrapContext = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_get_time NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
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

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_mmap NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if start & (PAGE_SIZE - 1) != 0 {
            println!(
                "expect the start address to be aligned with a page, but get an invalid start: {:#x}",
                start
            );
            return -1;
        }
        let start_addr = VirtAddr::from(start);
    
        if port > 7usize || port == 0 {
            println!("invalid port: {:#b}", port);
            return -1;
        }
    
        let permission = MapPermission::from_bits((port as u8) << 1).unwrap() | MapPermission::U;

        let current_task = current_task().unwrap();

        if current_task.task_mmap(start_addr, len, permission).is_ok()
        {
            0
        }
        else
        {
            println!("mmap failed");
            -1
        }
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_munmap NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let current_task = current_task().unwrap();
    if current_task.task_munmap(start, len).is_ok()
    {
        0
    }
    else
    {
        println!("munmap  failed");
        -1
    }
    }

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // -1
    let current_task = current_task().unwrap();
    let mut parent_inner = current_task.inner_exclusive_access();


    let token =parent_inner.get_user_token();
    let path = translated_str(token, path);
    
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let elf_data = app_inode.read_all();
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data.as_slice());
        // memory_set with elf program headers/trampoline/trap context/user stack
        let trap_cx_ppn = memory_set
        .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
        .unwrap()
        .ppn();
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = kstack_alloc();
        let kernel_stack_top = kernel_stack.get_top();
        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent_inner.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }        
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: parent_inner.base_size,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(&current_task)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: new_fd_table,
                    heap_bottom: parent_inner.heap_bottom,
                    program_brk: parent_inner.program_brk,
                    priority: parent_inner.priority,
                })
            },
        });
        parent_inner.children.push(task_control_block.clone());
        // prepare TrapContext in user space
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        let pid = task_control_block.pid.0 as isize;
        add_task(task_control_block);
        pid
        
    } else {
        -1
    }

}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if prio >= 2 {
        let task = current_task().unwrap();
        let mut inner = task.inner_exclusive_access();
        inner.priority = prio as usize;
        prio
    } else {
        -1
    }
}
