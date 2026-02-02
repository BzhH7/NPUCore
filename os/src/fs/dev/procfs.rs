//! /proc 虚拟文件系统实现
//! 
//! 提供动态进程信息访问，类似 Linux 的 procfs

use crate::fs::{dirent::Dirent, DiskInodeType};
use alloc::sync::Arc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use alloc::format;
use spin::Mutex;

use alloc::string::ToString;
use crate::{
    fs::{directory_tree::DirectoryTreeNode, file_trait::File, layout::Stat, StatMode, OpenFlags},
    mm::UserBuffer,
    syscall::errno::{EACCES, ENOTDIR, ESPIPE, ENOENT},
    task::{list_all_tasks, find_task_by_pid, TaskStatus},
};

/// /proc 目录的虚拟文件 - 列出所有进程目录
pub struct ProcDir {
    offset: Mutex<usize>,
}

impl ProcDir {
    pub fn new() -> Self {
        Self {
            offset: Mutex::new(0),
        }
    }
}

impl File for ProcDir {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(ProcDir::new())
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, _offset: Option<&mut usize>, _buf: &mut [u8]) -> usize {
        0
    }

    fn write(&self, _offset: Option<&mut usize>, _buf: &[u8]) -> usize {
        0
    }

    fn r_ready(&self) -> bool {
        true
    }

    fn w_ready(&self) -> bool {
        false
    }

    fn get_size(&self) -> usize {
        0
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 5),
            1,
            StatMode::S_IFDIR.bits() | 0o555,
            1,
            crate::makedev!(1, 3),
            0,
            0,
            0,
            0,
        )
    }

    fn read_user(&self, _offset: Option<usize>, _buf: UserBuffer) -> usize {
        0
    }

    fn write_user(&self, _offset: Option<usize>, _buf: UserBuffer) -> usize {
        ESPIPE as usize
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::Directory
    }

    fn info_dirtree_node(&self, _dirnode_ptr: alloc::sync::Weak<DirectoryTreeNode>) {}

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, _flags: OpenFlags, _special_use: bool) -> Arc<dyn File> {
        Arc::new(ProcDir::new())
    }

    fn open_subfile(&self) -> Result<Vec<(String, Arc<dyn File>)>, isize> {
        // 返回进程目录列表
        let tasks = list_all_tasks();
        let mut entries = Vec::new();
        
        for task in tasks {
            let pid_str = format!("{}", task.pid.0);
            entries.push((pid_str, Arc::new(ProcPidDir::new(task.pid.0)) as Arc<dyn File>));
        }
        
        Ok(entries)
    }

    fn create(&self, _name: &str, _file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> {
        Err(EACCES)
    }

    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize> {
        Err(EACCES)
    }

    fn unlink(&self, _delete: bool) -> Result<(), isize> {
        Err(EACCES)
    }

    fn get_dirent(&self, count: usize) -> Vec<Dirent> {
        use core::mem::size_of;
        
        let tasks = list_all_tasks();
        let mut dirents = Vec::new();
        
        // count 是字节数，需要计算最大条目数
        let max_items = count / size_of::<Dirent>();
        
        // 获取当前 offset
        let mut offset = self.offset.lock();
        let start_idx = *offset;
        
        for (i, task) in tasks.iter().enumerate().skip(start_idx) {
            if dirents.len() >= max_items {
                break;
            }
            let name = format!("{}", task.pid.0);
            dirents.push(Dirent::new(
                task.pid.0,
                (i + 2) as isize,  // d_off: 下一个条目的 offset (i+1 是当前，i+2 是下一个)
                DiskInodeType::Directory as u8,
                &name,
            ));
            *offset = i + 1;  // 更新 offset
        }
        
        dirents
    }

    fn lseek(&self, offset: isize, whence: crate::fs::layout::SeekWhence) -> Result<usize, isize> {
        let mut current = self.offset.lock();
        let new_offset = match whence {
            crate::fs::layout::SeekWhence::SEEK_SET => offset,
            crate::fs::layout::SeekWhence::SEEK_CUR => *current as isize + offset,
            crate::fs::layout::SeekWhence::SEEK_END => offset, // 目录大小未知，暂不支持
            _ => return Err(crate::syscall::errno::EINVAL),
        };
        if new_offset < 0 {
            return Err(crate::syscall::errno::EINVAL);
        }
        *current = new_offset as usize;
        Ok(new_offset as usize)
    }

    fn modify_size(&self, _diff: isize) -> Result<(), isize> {
        Err(EACCES)
    }

    fn truncate_size(&self, _new_size: usize) -> Result<(), isize> {
        Err(EACCES)
    }

    fn set_timestamp(&self, _ctime: Option<usize>, _atime: Option<usize>, _mtime: Option<usize>) {}

    fn get_single_cache(&self, _offset: usize) -> Result<Arc<Mutex<crate::fs::cache::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<Vec<Arc<Mutex<crate::fs::cache::PageCache>>>, ()> {
        Err(())
    }

    fn hang_up(&self) -> bool {
        false
    }

    fn fcntl(&self, _cmd: u32, _arg: u32) -> isize {
        -1
    }

    fn oom(&self) -> usize {
        0
    }
}

/// /proc/<pid> 目录
pub struct ProcPidDir {
    pid: usize,
    offset: Mutex<usize>,
}

impl ProcPidDir {
    pub fn new(pid: usize) -> Self {
        Self { 
            pid,
            offset: Mutex::new(0),
        }
    }
}

impl File for ProcPidDir {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(ProcPidDir::new(self.pid))
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, _offset: Option<&mut usize>, _buf: &mut [u8]) -> usize {
        0
    }

    fn write(&self, _offset: Option<&mut usize>, _buf: &[u8]) -> usize {
        0
    }

    fn r_ready(&self) -> bool {
        true
    }

    fn w_ready(&self) -> bool {
        false
    }

    fn get_size(&self) -> usize {
        0
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 5),
            self.pid as u64,
            StatMode::S_IFDIR.bits() | 0o555,
            1,
            crate::makedev!(1, 3),
            0,
            0,
            0,
            0,
        )
    }

    fn read_user(&self, _offset: Option<usize>, _buf: UserBuffer) -> usize {
        0
    }

    fn write_user(&self, _offset: Option<usize>, _buf: UserBuffer) -> usize {
        ESPIPE as usize
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::Directory
    }

    fn info_dirtree_node(&self, _dirnode_ptr: alloc::sync::Weak<DirectoryTreeNode>) {}

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, _flags: OpenFlags, _special_use: bool) -> Arc<dyn File> {
        Arc::new(ProcPidDir::new(self.pid))
    }

    fn open_subfile(&self) -> Result<Vec<(String, Arc<dyn File>)>, isize> {
        // 检查进程是否存在
        if find_task_by_pid(self.pid).is_none() {
            return Err(ENOENT);
        }
        
        Ok(vec![
            ("stat".to_string(), Arc::new(ProcPidStat::new(self.pid)) as Arc<dyn File>),
            ("status".to_string(), Arc::new(ProcPidStatus::new(self.pid)) as Arc<dyn File>),
            ("comm".to_string(), Arc::new(ProcPidComm::new(self.pid)) as Arc<dyn File>),
        ])
    }

    fn create(&self, _name: &str, _file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> {
        Err(EACCES)
    }

    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize> {
        Err(EACCES)
    }

    fn unlink(&self, _delete: bool) -> Result<(), isize> {
        Err(EACCES)
    }

    fn get_dirent(&self, count: usize) -> Vec<Dirent> {
        use core::mem::size_of;
        
        let entries = ["stat", "status", "comm"];
        let mut dirents = Vec::new();
        
        // count 是字节数，需要计算最大条目数
        let max_items = count / size_of::<Dirent>();
        
        // 获取当前 offset
        let mut offset = self.offset.lock();
        let start_idx = *offset;
        
        for (i, name) in entries.iter().enumerate().skip(start_idx) {
            if dirents.len() >= max_items {
                break;
            }
            dirents.push(Dirent::new(
                self.pid * 100 + i,
                (i + 2) as isize,
                DiskInodeType::File as u8,
                name,
            ));
            *offset = i + 1;
        }
        
        dirents
    }

    fn lseek(&self, offset: isize, whence: crate::fs::layout::SeekWhence) -> Result<usize, isize> {
        let mut current = self.offset.lock();
        let new_offset = match whence {
            crate::fs::layout::SeekWhence::SEEK_SET => offset,
            crate::fs::layout::SeekWhence::SEEK_CUR => *current as isize + offset,
            crate::fs::layout::SeekWhence::SEEK_END => offset,
            _ => return Err(crate::syscall::errno::EINVAL),
        };
        if new_offset < 0 {
            return Err(crate::syscall::errno::EINVAL);
        }
        *current = new_offset as usize;
        Ok(new_offset as usize)
    }

    fn modify_size(&self, _diff: isize) -> Result<(), isize> {
        Err(EACCES)
    }

    fn truncate_size(&self, _new_size: usize) -> Result<(), isize> {
        Err(EACCES)
    }

    fn set_timestamp(&self, _ctime: Option<usize>, _atime: Option<usize>, _mtime: Option<usize>) {}

    fn get_single_cache(&self, _offset: usize) -> Result<Arc<Mutex<crate::fs::cache::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<Vec<Arc<Mutex<crate::fs::cache::PageCache>>>, ()> {
        Err(())
    }

    fn hang_up(&self) -> bool {
        false
    }

    fn fcntl(&self, _cmd: u32, _arg: u32) -> isize {
        -1
    }

    fn oom(&self) -> usize {
        0
    }
}

/// /proc/<pid>/stat 文件 - Linux 兼容格式
pub struct ProcPidStat {
    pid: usize,
    offset: Mutex<usize>,
}

impl ProcPidStat {
    pub fn new(pid: usize) -> Self {
        Self { 
            pid,
            offset: Mutex::new(0),
        }
    }
    
    /// 生成 /proc/<pid>/stat 格式的内容
    /// 格式: pid (comm) state ppid pgrp session tty_nr tpgid flags minflt cminflt majflt cmajflt
    ///       utime stime cutime cstime priority nice num_threads itrealvalue starttime vsize rss ...
    fn generate_stat(&self) -> String {
        if let Some(task) = find_task_by_pid(self.pid) {
            let inner = task.acquire_inner_lock();
            let comm = task.get_comm();
            
            // 状态映射
            let state = match inner.task_status {
                TaskStatus::Ready => 'R',
                TaskStatus::Running => 'R',
                TaskStatus::Interruptible => 'S',
                TaskStatus::Zombie => 'Z',
            };
            
            // 获取 CPU 时间（单位：时钟节拍）
            let utime = inner.rusage.ru_utime.tv_sec * 100 + inner.rusage.ru_utime.tv_usec / 10000;
            let stime = inner.rusage.ru_stime.tv_sec * 100 + inner.rusage.ru_stime.tv_usec / 10000;
            
            // 获取父进程 PID
            let ppid = inner.parent.as_ref()
                .and_then(|p| p.upgrade())
                .map(|p| p.pid.0)
                .unwrap_or(0);
            
            // 获取进程组 ID
            let pgrp = inner.pgid;
            
            // nice 值
            let nice = inner.sched_entity.nice;
            
            // vruntime 可以作为 CPU 时间的近似
            let vruntime = inner.sched_entity.vruntime;
            
            drop(inner);
            
            // 简化的 stat 格式（只包含常用字段）
            format!(
                "{} ({}) {} {} {} 0 0 0 0 0 0 0 0 {} {} 0 0 20 {} 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n",
                self.pid,  // 1. pid
                comm,      // 2. comm
                state,     // 3. state
                ppid,      // 4. ppid
                pgrp,      // 5. pgrp
                utime,     // 14. utime
                stime,     // 15. stime
                nice,      // 19. nice
            )
        } else {
            String::new()
        }
    }
}

impl File for ProcPidStat {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(ProcPidStat::new(self.pid))
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, _offset: Option<&mut usize>, _buf: &mut [u8]) -> usize {
        0
    }

    fn write(&self, _offset: Option<&mut usize>, _buf: &[u8]) -> usize {
        0
    }

    fn r_ready(&self) -> bool {
        true
    }

    fn w_ready(&self) -> bool {
        false
    }

    fn get_size(&self) -> usize {
        self.generate_stat().len()
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 5),
            self.pid as u64,
            StatMode::S_IFREG.bits() | 0o444,
            1,
            crate::makedev!(1, 3),
            0,
            0,
            0,
            0,
        )
    }

    fn read_user(&self, offset: Option<usize>, mut buf: UserBuffer) -> usize {
        let content = self.generate_stat();
        let bytes = content.as_bytes();
        
        let start = offset.unwrap_or_else(|| {
            let mut off = self.offset.lock();
            let cur = *off;
            *off += buf.len();
            cur
        });
        
        if start >= bytes.len() {
            return 0;
        }
        
        let end = (start + buf.len()).min(bytes.len());
        buf.write(&bytes[start..end]);
        end - start
    }

    fn write_user(&self, _offset: Option<usize>, _buf: UserBuffer) -> usize {
        ESPIPE as usize
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::File
    }

    fn info_dirtree_node(&self, _dirnode_ptr: alloc::sync::Weak<DirectoryTreeNode>) {}

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, _flags: OpenFlags, _special_use: bool) -> Arc<dyn File> {
        Arc::new(ProcPidStat::new(self.pid))
    }

    fn open_subfile(&self) -> Result<Vec<(String, Arc<dyn File>)>, isize> {
        Err(ENOTDIR)
    }

    fn create(&self, _name: &str, _file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> {
        Err(EACCES)
    }

    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize> {
        Err(EACCES)
    }

    fn unlink(&self, _delete: bool) -> Result<(), isize> {
        Err(EACCES)
    }

    fn get_dirent(&self, _count: usize) -> Vec<Dirent> {
        Vec::new()
    }

    fn lseek(&self, offset: isize, whence: crate::fs::layout::SeekWhence) -> Result<usize, isize> {
        let mut current = self.offset.lock();
        let new_offset = match whence {
            crate::fs::layout::SeekWhence::SEEK_SET => offset,
            crate::fs::layout::SeekWhence::SEEK_CUR => *current as isize + offset,
            crate::fs::layout::SeekWhence::SEEK_END => self.get_size() as isize + offset,
            _ => return Err(crate::syscall::errno::EINVAL),
        };
        if new_offset < 0 {
            return Err(crate::syscall::errno::EINVAL);
        }
        *current = new_offset as usize;
        Ok(new_offset as usize)
    }

    fn modify_size(&self, _diff: isize) -> Result<(), isize> {
        Err(EACCES)
    }

    fn truncate_size(&self, _new_size: usize) -> Result<(), isize> {
        Err(EACCES)
    }

    fn set_timestamp(&self, _ctime: Option<usize>, _atime: Option<usize>, _mtime: Option<usize>) {}

    fn get_single_cache(&self, _offset: usize) -> Result<Arc<Mutex<crate::fs::cache::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<Vec<Arc<Mutex<crate::fs::cache::PageCache>>>, ()> {
        Err(())
    }

    fn hang_up(&self) -> bool {
        false
    }

    fn fcntl(&self, _cmd: u32, _arg: u32) -> isize {
        -1
    }

    fn oom(&self) -> usize {
        0
    }
}

/// /proc/<pid>/status 文件 - 人类可读格式
pub struct ProcPidStatus {
    pid: usize,
    offset: Mutex<usize>,
}

impl ProcPidStatus {
    pub fn new(pid: usize) -> Self {
        Self { 
            pid,
            offset: Mutex::new(0),
        }
    }
    
    fn generate_status(&self) -> String {
        if let Some(task) = find_task_by_pid(self.pid) {
            let inner = task.acquire_inner_lock();
            let comm = task.get_comm();
            
            let state = match inner.task_status {
                TaskStatus::Ready => "R (running)",
                TaskStatus::Running => "R (running)",
                TaskStatus::Interruptible => "S (sleeping)",
                TaskStatus::Zombie => "Z (zombie)",
            };
            
            let ppid = inner.parent.as_ref()
                .and_then(|p| p.upgrade())
                .map(|p| p.pid.0)
                .unwrap_or(0);
            
            let utime_sec = inner.rusage.ru_utime.tv_sec;
            let stime_sec = inner.rusage.ru_stime.tv_sec;
            
            drop(inner);
            
            format!(
                "Name:\t{}\n\
                 State:\t{}\n\
                 Tgid:\t{}\n\
                 Pid:\t{}\n\
                 PPid:\t{}\n\
                 Threads:\t1\n\
                 Utime:\t{} s\n\
                 Stime:\t{} s\n",
                comm,
                state,
                task.tgid,
                self.pid,
                ppid,
                utime_sec,
                stime_sec,
            )
        } else {
            String::new()
        }
    }
}

impl File for ProcPidStatus {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(ProcPidStatus::new(self.pid))
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, _offset: Option<&mut usize>, _buf: &mut [u8]) -> usize {
        0
    }

    fn write(&self, _offset: Option<&mut usize>, _buf: &[u8]) -> usize {
        0
    }

    fn r_ready(&self) -> bool {
        true
    }

    fn w_ready(&self) -> bool {
        false
    }

    fn get_size(&self) -> usize {
        self.generate_status().len()
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 5),
            self.pid as u64,
            StatMode::S_IFREG.bits() | 0o444,
            1,
            crate::makedev!(1, 3),
            0,
            0,
            0,
            0,
        )
    }

    fn read_user(&self, offset: Option<usize>, mut buf: UserBuffer) -> usize {
        let content = self.generate_status();
        let bytes = content.as_bytes();
        
        let start = offset.unwrap_or_else(|| {
            let mut off = self.offset.lock();
            let cur = *off;
            *off += buf.len();
            cur
        });
        
        if start >= bytes.len() {
            return 0;
        }
        
        let end = (start + buf.len()).min(bytes.len());
        buf.write(&bytes[start..end]);
        end - start
    }

    fn write_user(&self, _offset: Option<usize>, _buf: UserBuffer) -> usize {
        ESPIPE as usize
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::File
    }

    fn info_dirtree_node(&self, _dirnode_ptr: alloc::sync::Weak<DirectoryTreeNode>) {}

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, _flags: OpenFlags, _special_use: bool) -> Arc<dyn File> {
        Arc::new(ProcPidStatus::new(self.pid))
    }

    fn open_subfile(&self) -> Result<Vec<(String, Arc<dyn File>)>, isize> {
        Err(ENOTDIR)
    }

    fn create(&self, _name: &str, _file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> {
        Err(EACCES)
    }

    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize> {
        Err(EACCES)
    }

    fn unlink(&self, _delete: bool) -> Result<(), isize> {
        Err(EACCES)
    }

    fn get_dirent(&self, _count: usize) -> Vec<Dirent> {
        Vec::new()
    }

    fn lseek(&self, offset: isize, whence: crate::fs::layout::SeekWhence) -> Result<usize, isize> {
        let mut current = self.offset.lock();
        let new_offset = match whence {
            crate::fs::layout::SeekWhence::SEEK_SET => offset,
            crate::fs::layout::SeekWhence::SEEK_CUR => *current as isize + offset,
            crate::fs::layout::SeekWhence::SEEK_END => self.get_size() as isize + offset,
            _ => return Err(crate::syscall::errno::EINVAL),
        };
        if new_offset < 0 {
            return Err(crate::syscall::errno::EINVAL);
        }
        *current = new_offset as usize;
        Ok(new_offset as usize)
    }

    fn modify_size(&self, _diff: isize) -> Result<(), isize> {
        Err(EACCES)
    }

    fn truncate_size(&self, _new_size: usize) -> Result<(), isize> {
        Err(EACCES)
    }

    fn set_timestamp(&self, _ctime: Option<usize>, _atime: Option<usize>, _mtime: Option<usize>) {}

    fn get_single_cache(&self, _offset: usize) -> Result<Arc<Mutex<crate::fs::cache::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<Vec<Arc<Mutex<crate::fs::cache::PageCache>>>, ()> {
        Err(())
    }

    fn hang_up(&self) -> bool {
        false
    }

    fn fcntl(&self, _cmd: u32, _arg: u32) -> isize {
        -1
    }

    fn oom(&self) -> usize {
        0
    }
}

/// /proc/<pid>/comm 文件 - 命令名
pub struct ProcPidComm {
    pid: usize,
    offset: Mutex<usize>,
}

impl ProcPidComm {
    pub fn new(pid: usize) -> Self {
        Self { 
            pid,
            offset: Mutex::new(0),
        }
    }
    
    fn generate_comm(&self) -> String {
        if let Some(task) = find_task_by_pid(self.pid) {
            format!("{}\n", task.get_comm())
        } else {
            String::new()
        }
    }
}

impl File for ProcPidComm {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(ProcPidComm::new(self.pid))
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, _offset: Option<&mut usize>, _buf: &mut [u8]) -> usize {
        0
    }

    fn write(&self, _offset: Option<&mut usize>, _buf: &[u8]) -> usize {
        0
    }

    fn r_ready(&self) -> bool {
        true
    }

    fn w_ready(&self) -> bool {
        false
    }

    fn get_size(&self) -> usize {
        self.generate_comm().len()
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 5),
            self.pid as u64,
            StatMode::S_IFREG.bits() | 0o444,
            1,
            crate::makedev!(1, 3),
            0,
            0,
            0,
            0,
        )
    }

    fn read_user(&self, offset: Option<usize>, mut buf: UserBuffer) -> usize {
        let content = self.generate_comm();
        let bytes = content.as_bytes();
        
        let start = offset.unwrap_or_else(|| {
            let mut off = self.offset.lock();
            let cur = *off;
            *off += buf.len();
            cur
        });
        
        if start >= bytes.len() {
            return 0;
        }
        
        let end = (start + buf.len()).min(bytes.len());
        buf.write(&bytes[start..end]);
        end - start
    }

    fn write_user(&self, _offset: Option<usize>, _buf: UserBuffer) -> usize {
        ESPIPE as usize
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::File
    }

    fn info_dirtree_node(&self, _dirnode_ptr: alloc::sync::Weak<DirectoryTreeNode>) {}

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, _flags: OpenFlags, _special_use: bool) -> Arc<dyn File> {
        Arc::new(ProcPidComm::new(self.pid))
    }

    fn open_subfile(&self) -> Result<Vec<(String, Arc<dyn File>)>, isize> {
        Err(ENOTDIR)
    }

    fn create(&self, _name: &str, _file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> {
        Err(EACCES)
    }

    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize> {
        Err(EACCES)
    }

    fn unlink(&self, _delete: bool) -> Result<(), isize> {
        Err(EACCES)
    }

    fn get_dirent(&self, _count: usize) -> Vec<Dirent> {
        Vec::new()
    }

    fn lseek(&self, offset: isize, whence: crate::fs::layout::SeekWhence) -> Result<usize, isize> {
        let mut current = self.offset.lock();
        let new_offset = match whence {
            crate::fs::layout::SeekWhence::SEEK_SET => offset,
            crate::fs::layout::SeekWhence::SEEK_CUR => *current as isize + offset,
            crate::fs::layout::SeekWhence::SEEK_END => self.get_size() as isize + offset,
            _ => return Err(crate::syscall::errno::EINVAL),
        };
        if new_offset < 0 {
            return Err(crate::syscall::errno::EINVAL);
        }
        *current = new_offset as usize;
        Ok(new_offset as usize)
    }

    fn modify_size(&self, _diff: isize) -> Result<(), isize> {
        Err(EACCES)
    }

    fn truncate_size(&self, _new_size: usize) -> Result<(), isize> {
        Err(EACCES)
    }

    fn set_timestamp(&self, _ctime: Option<usize>, _atime: Option<usize>, _mtime: Option<usize>) {}

    fn get_single_cache(&self, _offset: usize) -> Result<Arc<Mutex<crate::fs::cache::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<Vec<Arc<Mutex<crate::fs::cache::PageCache>>>, ()> {
        Err(())
    }

    fn hang_up(&self) -> bool {
        false
    }

    fn fcntl(&self, _cmd: u32, _arg: u32) -> isize {
        -1
    }

    fn oom(&self) -> usize {
        0
    }
}

/// 解析路径中的 PID（如 "/proc/123/stat" 中的 123）
pub fn parse_proc_pid(path: &str) -> Option<usize> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if parts.len() >= 2 && parts[0] == "proc" {
        parts[1].parse().ok()
    } else {
        None
    }
}

/// 解析 /proc/<pid>/<subfile> 路径
pub fn open_proc_file(path: &str) -> Option<Arc<dyn File>> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    
    if parts.is_empty() || parts[0] != "proc" {
        return None;
    }
    
    match parts.len() {
        1 => {
            // /proc
            Some(Arc::new(ProcDir::new()))
        }
        2 => {
            // /proc/<pid>
            if let Ok(pid) = parts[1].parse::<usize>() {
                if find_task_by_pid(pid).is_some() {
                    Some(Arc::new(ProcPidDir::new(pid)))
                } else {
                    None
                }
            } else {
                None
            }
        }
        3 => {
            // /proc/<pid>/<file>
            if let Ok(pid) = parts[1].parse::<usize>() {
                if find_task_by_pid(pid).is_some() {
                    match parts[2] {
                        "stat" => Some(Arc::new(ProcPidStat::new(pid))),
                        "status" => Some(Arc::new(ProcPidStatus::new(pid))),
                        "comm" => Some(Arc::new(ProcPidComm::new(pid))),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}
