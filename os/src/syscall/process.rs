//! Process management syscalls
//!
use alloc::sync::Arc;

use crate::{
    config::MAX_SYSCALL_NUM,
    fs::{open_file, OpenFlags},
    mm::{translated_byte_buffer, translated_refmut, translated_str, MapPermission, PageTable, VirtAddr, VirtPageNum},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TaskControlBlock, TaskStatus
    },
};

use crate::timer::get_time_us;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
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
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
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

// #[no_mangle]
/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
/// pid=-1 means waiting for any child process.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
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
        // ++++ temporarily access child PCB exclusively（暂时获取子进程TCB的使用权）
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


pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    // _ts is a virtual address
    trace!("kernel: sys_get_time");
    let us = get_time_us();     //get the time in us
    let mut time=TimeVal{
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    let time_raw_ptr=&mut time as *const TimeVal as *const u8;
    let time_slice: &[u8];
    unsafe{
        time_slice =core::slice::from_raw_parts(time_raw_ptr,core::mem::size_of::<TimeVal>());
    }
    //get the current task control block(这个函数返回的是一个Arc指针的clone，后面还是直接手动drop掉)
    let tcb_ptr:Arc::<TaskControlBlock> =current_task().unwrap();
    let inner= (*tcb_ptr).inner_exclusive_access();
    let buffers = translated_byte_buffer(inner.get_user_token(),_ts as *const u8,core::mem::size_of::<TimeVal>());  //get the translated byte buffer
    drop(inner);
    // drop(tcb_ptr);
    let mut start=0;
    // write the time to the buffer(phiysical memory)
    for buffer in buffers {
        if start+buffer.len()>=time_slice.len(){
            buffer.copy_from_slice(&time_slice[start..]);
        }else{
            buffer.copy_from_slice(&time_slice[start..start+buffer.len()]);
        }
        start+=buffer.len();
    }
    0
}

pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    //get the current task control block(这个函数返回的是一个Arc指针的clone，后面还是直接手动drop掉)
    let tcb_ptr:Arc::<TaskControlBlock> =current_task().unwrap();
    // inner borrow the task control block
    let inner= (*tcb_ptr).inner_exclusive_access();
    let buffers = translated_byte_buffer(inner.get_user_token(),_ti as *const u8,core::mem::size_of::<TimeVal>());  //get the translated byte buffer
    let us = get_time_us();     //get the time in us
    let mut temp_info=TaskInfo{
        status:TaskStatus::Running,
        syscall_times:inner.syscall_times.clone(),
        time:(us-inner.sche_time.unwrap())/1000,
    };
    // get the raw pointer of TaskInfo
    let info_raw_ptr=&mut temp_info as *const TaskInfo as *const u8;
    let info_slice: &[u8];
    // get the slice of TaskInfo
    unsafe{
        info_slice =core::slice::from_raw_parts(info_raw_ptr,core::mem::size_of::<TaskInfo>());
    }
    drop(inner);
    // drop(tcb_ptr);

    let mut start=0;
    // write the time to the buffer(phiysical memory)
    for buffer in buffers {
        if start+buffer.len()>=info_slice.len(){
            buffer.copy_from_slice(&info_slice[start..]);
        }else{
            buffer.copy_from_slice(&info_slice[start..start+buffer.len()]);
        }
        start+=buffer.len();
    }
    0
}

#[no_mangle]
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    if (_start%4096)!=0{                //start address is not page align
        println!("start address is not page align");
        return -1;
    }
    //assert the port is valid
    if _port&(!0x7)!=0||_port&0x7==0{
        println!("the port is not valid");
        return -1;
    }

    let vpn =VirtPageNum::from(_start>>12); //vpn is the virtual page number(page align)
    
    let page_num =(_len+4095)/4096;      //page_num is the number of pages
    
    //get the current task control block
    let tcb_ptr:Arc::<TaskControlBlock> =current_task().unwrap();
    let mut inner= (*tcb_ptr).inner_exclusive_access();
    let page_table=PageTable::from_token(inner.get_user_token());  //get the page table

    //check the page_entry is valid or not
    for i in 0..page_num{
        match page_table.translate(VirtPageNum::from(usize::from(vpn)+i)){
            Some(pte) if pte.is_valid()=>{
                return -1;
            }
            _=>{continue;}
        }
    }
    //create a new map
    let mut flags =MapPermission::from_bits((_port<<1) as u8).unwrap();  //shift the port to the left by 1(为了对上页表项的标志位)
    flags=flags | MapPermission::U;     // add the user flag
    inner.memory_set.insert_framed_area(VirtAddr::from(_start),VirtAddr::from(_start+_len),flags);  //insert the new map area
    inner.memory_set.append_to(VirtAddr::from(_start),VirtAddr::from(_start+_len));    //init the new map area
    drop(inner);
    // drop(tcb_ptr);
    return 0;
}

#[no_mangle]
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    
    if (_start%4096)!=0{                //start address is not page align
        println!("start address is not page align");
        return -1;
    }

    let vpn =VirtPageNum::from(_start>>12); //vpn is the virtual page number(page align)

    let page_num =(_len+4095)/4096;      //page_num is the number of pages
    let tcb_ptr:Arc::<TaskControlBlock> =current_task().unwrap();
    let mut inner= (*tcb_ptr).inner_exclusive_access();
    let page_table=PageTable::from_token(inner.get_user_token());  //get the page table
    //check the pageentry is valid or not
    for i in 0..page_num{
        match page_table.translate(VirtPageNum::from(usize::from(vpn)+i)){
            Some(pte) if pte.is_valid()=>{continue;}
            _=>{
                return -1;
            }
        }
    }
    // remove the map area
    inner.memory_set.shrink_to(VirtAddr::from(_start),VirtAddr::from(_start)); //ummap the map area
    drop(inner);
    return 0;
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

pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // get the path of the new app
    let path = translated_str(current_user_token(), _path);
    if let Some(inode) = open_file(&path, OpenFlags::RDONLY) {
        let v: alloc::vec::Vec<u8> = inode.read_all();
        // get the app data(create a new task control block)
        let tcb=Arc::new(TaskControlBlock::new(v.as_slice()));
        let pid = tcb.getpid();
        // get the task control block of the current task(当前任务一定存在，不然不会有进程调用spawn)
        let current_tcb=current_task().unwrap();
        let mut inner =current_tcb.inner_exclusive_access();
        // add the new task to the children list of the current task
        inner.children.push(tcb.clone());
        drop(inner);
        let mut inner = tcb.inner_exclusive_access();
        inner.parent=Some(Arc::downgrade(&current_tcb));
        drop(inner);
        // for child in inner.children.iter() {
        //     println!("spwan====parent pid :{},child pid:{}",current_tcb.getpid(),child.getpid());
        // }
        // drop(current_tcb);
        // and add it to the task list（此时会发生所有权的转移，所以需要在之前就取出当前进程的pid）
        // println!("spwan====pid is {}, Num is {}",tcb.getpid(),Arc::strong_count(&tcb));
        add_task(tcb);
        pid as isize
    }else{
        -1
    }
}

pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if _prio>=2{
        _prio
    }else{
        -1
    }
}