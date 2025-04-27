# Lab1 Report for rCore OS

## Overview

This lab involved implementing syscall tracing functionality in the rCore operating system. The implementation tracks the number of syscall invocations for each task and provides a mechanism to query this information through a new syscall interface.

## Implementation Details

### Syscall Tracing

I implemented syscall tracing by:

1. Adding a `MAX_SYSCALL_NUM` constant (512) to define the maximum number of possible syscalls
2. Creating a `SyscallInfo` structure to store syscall statistics
3. Updating the `TaskControlBlock` structure to include an array of `SyscallInfo`
4. Adding functions to track and query syscall usage

### Key Components Added

1. **SyscallInfo Structure**:
    ```rust
    #[derive(Clone, Copy)]
    pub struct SyscallInfo {
         pub id: usize,
         pub times: usize
    }
    ```

2. **TaskControlBlock Update**:
    Added the `call` field to track syscalls:
    ```rust
    pub struct TaskControlBlock {
         pub task_status: TaskStatus,
         pub task_cx: TaskContext,
         pub call: [SyscallInfo; MAX_SYSCALL_NUM],
    }
    ```

3. **Syscall Tracking Functions**:
    ```rust
    pub fn get_syscall_num(syscall_id: usize) -> usize {
         let inner = TASK_MANAGER.inner.exclusive_access();
         let current = inner.current_task;
         inner.tasks[current].call[syscall_id].times
    }

    pub fn update_syscall_num(syscall_id: usize) {
         let mut inner = TASK_MANAGER.inner.exclusive_access();
         let current = inner.current_task;
         inner.tasks[current].call[syscall_id].times += 1; 
    }
    ```

4. **Enhanced sys_trace Implementation**:
    Expanded the functionality to:
    - Read bytes at specified addresses
    - Write bytes to specified addresses
    - Query syscall count for a specific syscall ID

### Dependencies

Added the `array-init` crate (v2.0) to help initialize the syscall tracking arrays.

## Challenges

1. Understanding the existing task management system and integrating the syscall tracking mechanism without disrupting other functionality.
2. Ensuring thread-safe access to the syscall counters.
3. Properly implementing the trace syscall with multiple functionalities based on the trace request type.

## Conclusion

The implementation successfully provides syscall tracing capabilities to the rCore OS. This feature could be useful for debugging, performance analysis, and understanding application behavior at the system call level.