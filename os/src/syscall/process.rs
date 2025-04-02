//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, get_syscall_num, suspend_current_and_run_next},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

// TODO: implement the syscall
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        0 => {
            // Read byte at address id
            let addr = id as *const u8;
            unsafe {
                match core::ptr::read_volatile(addr) {
                    val => val as isize,
                }
            }
        }
        1 => {
            // Write data (as u8) to address id
            let addr = id as *mut u8;
            unsafe {
                core::ptr::write_volatile(addr, data as u8);
            }
            0
        }
        2 => {
            // Query syscall count for id
            // Get current task's syscall statistics
            get_syscall_num(id) as isize
        }
        _ => -1,
    }
}
