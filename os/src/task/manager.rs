//!Implementation of [`TaskManager`]
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
    }
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}

/// get the length of ready queue
pub fn get_length()->usize{
    let manager_inner = TASK_MANAGER.exclusive_access();
    let length = manager_inner.ready_queue.len();
    drop(manager_inner);
    length
}

/// find the smallesr stride
pub fn find_smallest_stride() -> Option<Arc<TaskControlBlock>> {

    let mut min_stride_task=fetch_task();
    let mut min_stride;
    // 注意这里需要传递的是引用,否则会发生所有权转移,导致min_stride_task不再有效
    if let Some(temp)=min_stride_task{
        // get the stride of the first task
        let inner=temp.inner_exclusive_access();
        min_stride=inner.stride;
        // 由于上面传递的不是引用，所以这里需要重新给min_stride_task赋值
        min_stride_task=Some(temp.clone());
        drop(inner);
    }else{
        // no task in ready queue
        return None;
    }
    let manager_inner = TASK_MANAGER.exclusive_access();
    // 提前获取长度防止在迭代过程中长度变化(长度其实就是当前还在就绪队列中的任务数)
    let length = manager_inner.ready_queue.len();
    drop(manager_inner);
    for _ in 0..length {
        // fetch the next task
        let task =fetch_task();
        if let Some(temp)=task{
            let inner=temp.inner_exclusive_access();
            if inner.stride < min_stride {
                // add the min_stride_task back to ready queue
                add_task(min_stride_task.unwrap());
                min_stride = inner.stride;
                min_stride_task = Some(temp.clone());
            }else{
                // add the task back to ready queue
                add_task(temp.clone());
            }
            drop(inner);
        }else{
            return None;
        }
    }
    min_stride_task
}
