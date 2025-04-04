//! Process management syscalls
use crate::{
    config::PAGE_SIZE, mm::{is_readable, is_writable, virt2phys_addr, MapPermission, VirtAddr}, task::{change_program_brk, exit_current_and_run_next, get_syscall_num, suspend_current_and_run_next, TASK_MANAGER}, timer::get_time_us
};


#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");

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

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        0 => {
            // Check if address is valid in SV39 mode
            // In SV39, bits 39-63 must be sign extensions of bit 38
            let bit38 = ((id >> 38) & 1) != 0;
            let high_bits = id >> 39;
            let expected_high_bits = if bit38 { 0x1ffffff } else { 0 };
            if high_bits != expected_high_bits {
                return -1;
            }
            
            // Read byte at address id
            let virt_addr = VirtAddr::from(id);
            if !is_readable(virt_addr) {
                return -1;
            }
            
            // Only perform the read if the address is valid and readable
            if let Some(phys_addr) = virt2phys_addr(virt_addr) {
                unsafe {
                    if id == isize::MAX as usize {
                        println!("uuuuu")                   
                    }
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
            // Write data (as u8) to address id
            let virt_addr = VirtAddr::from(id);
            if !is_writable(virt_addr) {
                return -1;
            }
            
            // Only perform the write if the address is valid and writable
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
            // Query syscall count for id
            // Get current task's syscall statistics
            get_syscall_num(id) as isize
        }
        _ => -1,
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
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

    if TASK_MANAGER.task_mmap(start_addr, len, permission).is_ok()
    {
        0
    }
    else
    {
        println!("mmap failed");
        -1
    }
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");

    if TASK_MANAGER.task_munmap(start, len).is_ok()
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
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
