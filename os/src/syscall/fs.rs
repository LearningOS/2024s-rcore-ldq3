//! File and filesystem-related syscalls
use crate::fs::{open_file, OpenFlags, Stat,ROOT_INODE};
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer};
use crate::task::{current_task, current_user_token};
use alloc::sync::Arc;
use crate::task::TaskControlBlock;
use easy_fs::{block_cache_sync_all, DirEntry, DIRENT_SZ};
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

pub fn sys_fstat(_fd: usize, _st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // check the fd
    if _fd>=current_task().unwrap().inner_exclusive_access().fd_table.len(){
        return -1;
    }
    // get the current task control block(因为需要使用文件描述符表，所以需要获取当前的TCB) 
    let tcb_ptr:Arc::<TaskControlBlock> =current_task().unwrap();
    let inner= tcb_ptr.inner_exclusive_access();
    // check the fd
    if inner.fd_table[_fd].is_none(){
        return -1;
    }
    // get the osinode
    let osinode = inner.fd_table[_fd].as_ref().unwrap().clone();
    drop(inner);
    drop(tcb_ptr);
    // get the stat
    let stat=osinode.get_stat();
    let stat_raw_ptr=&stat as *const Stat as *const u8;
    let buffers = translated_byte_buffer(current_user_token(),_st as *const u8,core::mem::size_of::<Stat>());  //get the translated byte buffer
    // change the stat to byte slice
    let stat_slice: &[u8];
    unsafe{
        stat_slice =core::slice::from_raw_parts(stat_raw_ptr,core::mem::size_of::<Stat>());
    }
    let mut start=0;
    // write the time to the buffer(phiysical memory)
    for buffer in buffers {
        if buffer.len()>=stat_slice.len()-start{
            buffer.copy_from_slice(&stat_slice[start..]);
        }else{
            buffer.copy_from_slice(&stat_slice[start..start+buffer.len()]);
        }
        start+=buffer.len();
    }
    0
}


pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    // need to create a new Diskinode(the same to old one)
    trace!(
        "kernel:pid[{}] sys_linkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // check the old_name
    let old_name=translated_str(current_user_token(),_old_name);
    let old_inode=ROOT_INODE.find(&old_name);
    if old_inode.is_none(){
        return -1;
    }
    let old_inode=old_inode.unwrap();

    // get the old node's disk inode
    // increase the link count
    old_inode.modify_disk_inode(|old_disk_inode |{
        old_disk_inode.nlink+=1;
        // DiskInode{
        //     size:old_disk_inode.size,
        //     direct:old_disk_inode.direct,
        //     indirect1:old_disk_inode.indirect1,
        //     indirect2:old_disk_inode.indirect2,
        //     // increase the link count
        //     nlink:old_disk_inode.nlink,
        //     type_:DiskInodeType::File,
        // }
    });

    // create a new dirent
    let new_name=translated_str(current_user_token(),_new_name);
    ROOT_INODE.modify_disk_inode(|root_inode| {
        // append file in the dirent
        let file_count = (root_inode.size as usize) / DIRENT_SZ;
        let new_size = (file_count + 1) * DIRENT_SZ;
        // increase size
        ROOT_INODE.increase_size(new_size as u32, root_inode, &mut ROOT_INODE.get_fs().lock());
        // write dirent
        let dirent = DirEntry::new(&new_name, old_inode.get_inode_id());
        root_inode.write_at(
            file_count * DIRENT_SZ,
            dirent.as_bytes(),
            &ROOT_INODE.get_block_device(),
        );
    });
    drop(old_inode);
    // // the new file has been created
    // if new_inode.is_none(){
    //     return -1;
    // }
    // let new_inode = new_inode.unwrap();
    // // as the new file must be a file, so we need not to change the type of the new file
    // // copy the old disk inode to the new disk inode to create a nlink
    // new_inode.modify_disk_inode(|new_disk_inode|{
    //     new_disk_inode.size = old_disk_node.size;
    //     new_disk_inode.direct=old_disk_node.direct;
    //     new_disk_inode.indirect1=old_disk_node.indirect1;
    //     new_disk_inode.indirect2=old_disk_node.indirect2;
    //     new_disk_inode.type_=DiskInodeType::File;
    //     new_disk_inode.nlink=old_disk_node.nlink;
    // });
    // drop(new_inode);
    block_cache_sync_all();
    // release efs lock automatically by compiler
    0
}

pub fn sys_unlinkat(_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // check the name and get the inode
    let name=translated_str(current_user_token(),_name);
    let inode=ROOT_INODE.find(&name);
    if inode.is_none(){
        return -1;
    }
    let inode=inode.unwrap();

    // decrease the link count
    let no_link=inode.modify_disk_inode(|disk_inode|{
        disk_inode.nlink-=1;
        match disk_inode.nlink {
            0=>true,
            _=>false,
        }
    });

    // recycle the block
    if no_link{
        // recycle the datanode
        inode.clear();
        // recycle the disknode
        let fs = ROOT_INODE.get_fs();
        let mut fs = fs.lock();
        fs.dealloc_inode(inode.get_inode_id());
    }

    drop(inode);

    // remove the dirent
    // find the dirent number
    let num=ROOT_INODE.delete_dirent(&name);
    if num==-1{
        return -1;
    }
    0
}