# 操作系统实验五报告

## 实验概述

本次实验主要聚焦于操作系统中的同步机制和死锁检测，是对进程间通信与同步原语的深入探索。我实现了基于银行家算法的死锁检测机制，并增强了信号量和互斥锁的功能，提高了系统的安全性和可靠性。

## 实现内容

### 死锁检测机制

在本实验中，我实现了基于银行家算法的死锁检测机制，通过以下步骤完成：

1. 在进程控制块中添加死锁检测标志：
```rust
pub struct ProcessControlBlockInner {
    // 其他字段...
    pub deadlock_check: bool,
}
```

2. 实现了`sys_enable_deadlock_detect`系统调用，允许用户程序开启或关闭死锁检测：
```rust
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    
    if enabled != 1 && enabled != 0 {
        return -1;
    }
    process_inner.deadlock_check = enabled == 1;
    0
}
```

3. 实现银行家算法进行死锁检测：
```rust
fn deadlock_check(available: Vec<usize>, allocation: Vec<Vec<usize>>, need: Vec<Vec<usize>>) -> bool {
    let (n, m) = (allocation.len(), allocation[0].len());
    let mut work = available;
    let mut finish = vec![false; n];
    
    loop {
        let mut idx = usize::MAX;
        for i in 0..n {
            if finish[i] { continue; }
            
            let mut flag = true;
            for j in 0..m {
                if need[i][j] > work[j] {
                    flag = false;
                    break;
                }
            }
            
            if flag {
                idx = i;
                break;
            }
        }
        
        if idx != usize::MAX {
            for j in 0..m {
                work[j] += allocation[idx][j];
            }
            finish[idx] = true;
        } else {
            break;
        }
    }
    
    finish.iter().all(|&x| x)
}
```

### 增强的同步原语

#### 互斥锁实现

为互斥锁系统调用添加了死锁检测功能：

```rust
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    
    // 记录当前任务需要获取的互斥锁
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.mutex_need = mutex_id;
    drop(task_inner);
    
    // 进行死锁检测
    if process_inner.deadlock_check {
        // 收集系统资源状态并进行检测
        let n = process_inner.tasks.len();
        let m = process_inner.mutex_list.len();
        let mut available = vec![1; m];
        let mut need = vec![vec![0; m]; n];
        let mut allocation = vec![vec![0; m]; n];
        
        // 构建资源分配矩阵
        for (i, task_opt) in process_inner.tasks.iter().enumerate() {
            if let Some(task) = task_opt {
                let task_inner = task.inner_exclusive_access();
                for &mid in &task_inner.mutex_allocation {
                    allocation[i][mid] += 1;
                    available[mid] -= 1;
                }
                let nid = task_inner.mutex_need;
                if nid != usize::MAX {
                    need[i][nid] += 1;
                }
            }
        }
        
        // 死锁检测结果处理
        if !deadlock_check(available, allocation, need) {
            return -0xDEAD; // 返回死锁错误码
        }
    }
    
    // 获取锁并更新资源分配状态
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    mutex.lock();
    
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.mutex_allocation.push(mutex_id);
    task_inner.mutex_need = usize::MAX;
    
    0
}
```

#### 信号量实现

同样为信号量添加了死锁检测功能：

```rust
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    
    // 记录当前任务需要获取的信号量
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.sem_need = sem_id;
    drop(task_inner);
    
    // 进行死锁检测
    if process_inner.deadlock_check {
        // 类似互斥锁实现，收集资源状态并检测...
        if !deadlock_check(available, allocation, need) {
            return -0xDEAD;
        }
    }
    
    // 获取信号量
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.down();
    
    0
}
```

### 任务控制块增强

为了支持死锁检测，对任务控制块进行了扩展：

```rust
pub struct TaskControlBlockInner {
    // 其他字段...
    pub mutex_need: usize,
    pub sem_need: usize,
    pub mutex_allocation: Vec<usize>,
    pub sem_allocation: Vec<(usize, usize)>,
}
```

这些字段记录了每个任务当前分配的资源以及正在申请的资源，为死锁检测提供必要的信息。

## 实验挑战与解决方案

1. **资源状态收集**：需要准确收集系统中所有资源的分配状态。解决方案是在每个进程和任务中维护资源分配和需求记录。

2. **多种资源的处理**：需要同时处理互斥锁和信号量两种资源。解决方案是为每种资源单独实现检测逻辑，并在任务控制块中分别记录。

3. **状态一致性维护**：需要确保资源状态的更新与实际操作同步。解决方案是在每次资源操作后立即更新相应的状态记录。

## 实验总结

通过本次实验，我深入理解了操作系统中的同步机制和死锁问题。实现的死锁检测机制可以有效地识别潜在的死锁情况，提高了系统的安全性和可靠性。此外，对互斥锁和信号量的增强使它们不仅能够提供基本的同步功能，还能预防和检测死锁情况。

银行家算法作为一种经典的死锁预防方法，其工作原理和实现细节在本实验中得到了充分的探索。通过维护系统资源的分配状态和需求矩阵，可以在资源分配前进行安全性检查，避免系统进入不安全状态。

这次实验的经验对于理解现代操作系统中的进程管理和资源分配机制非常有价值，也为后续开发更复杂的系统软件奠定了基础。