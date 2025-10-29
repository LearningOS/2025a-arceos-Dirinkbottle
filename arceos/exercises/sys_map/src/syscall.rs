#![allow(dead_code)]

extern crate alloc;

use core::ffi::{c_void, c_char, c_int};
use axhal::arch::TrapFrame;
use axhal::trap::{register_trap_handler, SYSCALL, PAGE_FAULT};
use axerrno::LinuxError;
use axtask::current;
use axtask::TaskExtRef;
use axhal::paging::MappingFlags;
use arceos_posix_api as api;

const SYS_IOCTL: usize = 29;
const SYS_OPENAT: usize = 56;
const SYS_CLOSE: usize = 57;
const SYS_READ: usize = 63;
const SYS_WRITE: usize = 64;
const SYS_WRITEV: usize = 66;
const SYS_EXIT: usize = 93;
const SYS_EXIT_GROUP: usize = 94;
const SYS_SET_TID_ADDRESS: usize = 96;
const SYS_MMAP: usize = 222;

const AT_FDCWD: i32 = -100;

/// Macro to generate syscall body
///
/// It will receive a function which return Result<_, LinuxError> and convert it to
/// the type which is specified by the caller.
#[macro_export]
macro_rules! syscall_body {
    ($fn: ident, $($stmt: tt)*) => {{
        #[allow(clippy::redundant_closure_call)]
        let res = (|| -> axerrno::LinuxResult<_> { $($stmt)* })();
        match res {
            Ok(_) | Err(axerrno::LinuxError::EAGAIN) => debug!(concat!(stringify!($fn), " => {:?}"),  res),
            Err(_) => info!(concat!(stringify!($fn), " => {:?}"), res),
        }
        match res {
            Ok(v) => v as _,
            Err(e) => {
                -e.code() as _
            }
        }
    }};
}

bitflags::bitflags! {
    #[derive(Debug)]
    /// permissions for sys_mmap
    ///
    /// See <https://github.com/bminor/glibc/blob/master/bits/mman.h>
    struct MmapProt: i32 {
        /// Page can be read.
        const PROT_READ = 1 << 0;
        /// Page can be written.
        const PROT_WRITE = 1 << 1;
        /// Page can be executed.
        const PROT_EXEC = 1 << 2;
    }
}

impl From<MmapProt> for MappingFlags {
    fn from(value: MmapProt) -> Self {
        let mut flags = MappingFlags::USER;
        if value.contains(MmapProt::PROT_READ) {
            flags |= MappingFlags::READ;
        }
        if value.contains(MmapProt::PROT_WRITE) {
            flags |= MappingFlags::WRITE;
        }
        if value.contains(MmapProt::PROT_EXEC) {
            flags |= MappingFlags::EXECUTE;
        }
        flags
    }
}

bitflags::bitflags! {
    #[derive(Debug)]
    /// flags for sys_mmap
    ///
    /// See <https://github.com/bminor/glibc/blob/master/bits/mman.h>
    struct MmapFlags: i32 {
        /// Share changes
        const MAP_SHARED = 1 << 0;
        /// Changes private; copy pages on write.
        const MAP_PRIVATE = 1 << 1;
        /// Map address must be exactly as requested, no matter whether it is available.
        const MAP_FIXED = 1 << 4;
        /// Don't use a file.
        const MAP_ANONYMOUS = 1 << 5;
        /// Don't check for reservations.
        const MAP_NORESERVE = 1 << 14;
        /// Allocation is for a stack.
        const MAP_STACK = 0x20000;
    }
}

#[register_trap_handler(SYSCALL)]
fn handle_syscall(tf: &TrapFrame, syscall_num: usize) -> isize {
    ax_println!("handle_syscall [{}] ...", syscall_num);
    let ret = match syscall_num {
         SYS_IOCTL => sys_ioctl(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _) as _,
        SYS_SET_TID_ADDRESS => sys_set_tid_address(tf.arg0() as _),
        SYS_OPENAT => sys_openat(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _, tf.arg3() as _),
        SYS_CLOSE => sys_close(tf.arg0() as _),
        SYS_READ => sys_read(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_WRITE => sys_write(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_WRITEV => sys_writev(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_EXIT_GROUP => {
            ax_println!("[SYS_EXIT_GROUP]: system is exiting ..");
            axtask::exit(tf.arg0() as _)
        },
        SYS_EXIT => {
            ax_println!("[SYS_EXIT]: system is exiting ..");
            axtask::exit(tf.arg0() as _)
        },
        SYS_MMAP => sys_mmap(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4() as _,
            tf.arg5() as _,
        ),
        _ => {
            ax_println!("Unimplemented syscall: {}", syscall_num);
            -LinuxError::ENOSYS.code() as _
        }
    };
    ret
}
//入手点
#[allow(unused_variables)]
fn sys_mmap(
    addr: *mut usize, 
    length: usize, 
    prot: i32, // 页面权限
    flags: i32, 
    fd: i32,
    _offset: isize, 
) -> isize {
use memory_addr::{VirtAddr, VirtAddrRange};
//修复：考虑文件映射
    syscall_body!(sys_mmap,{
       // ax_println!(
        //    "sys_mmap: addr={:?},length={:#x},prot={:#x},flags={:#x},fd={}",
         //   addr, length, prot, flags, fd
        //);
        
        let map_flags = MmapFlags::from_bits_truncate(flags);//忽略未知位 容错
        
        // 检查是否为匿名映射：MAP_ANONYMOUS标志或者fd == -1，来采用延迟分配
        let is_anonymous = map_flags.contains(MmapFlags::MAP_ANONYMOUS) || fd == -1;
        
        // 对于文件映射，我们只支持 MAP_PRIVATE（私有映射，写时复制）
        if !is_anonymous {
            if !map_flags.contains(MmapFlags::MAP_PRIVATE) {
             //   ax_println!("sys_mmap: only support MAP_PRIVATE for file mapping");
                return Err(LinuxError::ENOSYS);
            }
            //ax_println!("sys_mmap:file mapping (MAP_PRIVATE)");
        } else {
            //ax_println!("sys_mmap:anonymous mapping");
        }
        if length == 0 {
            return Err(LinuxError::EINVAL);
        }
        //对齐长度!!!
        const PAGE_SIZE: usize = 0x1000;//4096 
        let aligned_length = (length + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        //获取地址空间
        let curr = current();
        let mut aspace = curr.task_ext().aspace.lock();
        let addr_hint = VirtAddr::from(addr as usize);
        let alloc_addr = if map_flags.contains(MmapFlags::MAP_FIXED) {
            //必须用指定地址
            if addr as usize == 0 {
                return Err(LinuxError::EINVAL);
            }
            addr_hint
        } else {
            //自动分配
            let va_range = VirtAddrRange::from_start_size(
                aspace.base(),
                aspace.size()
            );
            aspace.find_free_area(//在指定范围随机查找空闲区域用来满足mmap
                if addr as usize != 0 { addr_hint/*从这里开始向后搜索 */ } else { aspace.base() },
                aligned_length,
                va_range
            ).ok_or_else(|| {
           //     ax_println!("sys_mmap:find_free_area failed,no memory");
                LinuxError::ENOMEM
            })?
        };
        
        //ax_println!("sys_mmap:alloc_addr={:#x},aligned_length={:#x}",alloc_addr,aligned_length);
        
        //转换权限
        let prot_flags = MmapProt::from_bits_truncate(prot);
        let mapping_flags = MappingFlags::from(prot_flags);
        
       // ax_println!("sys_mmap:mapping_flags={:?}",mapping_flags);
        
        //创建映射
        if is_anonymous {
            // 匿名映射：延迟分配
            aspace.map_alloc(
                alloc_addr,//起始地址
                aligned_length,//长度
                mapping_flags,// 权限
                false//backend pagefault alloc - 延迟分配
            ).map_err(|e| {
               // ax_println!("sys_mmap:map_alloc failed: {:?}", e);
                LinuxError::ENOMEM
            })?;
        } else {
            // 文件映射：立即分配并读取文件
            aspace.map_alloc(
                alloc_addr,
                aligned_length,
                mapping_flags,
                true  // populate=true - 立即分配物理页
            ).map_err(|e| {
               // ax_println!("sys_mmap:map_alloc file failed:{:?}",e);
                LinuxError::ENOMEM
            })?;
            
            // 读取文件内容到映射的内存
            let mut buffer = alloc::vec![0u8; length];
            let read_len = api::sys_read(fd, buffer.as_mut_ptr() as *mut c_void, length);
            if read_len < 0 {
            //    ax_println!("sys_mmap:read file failed");
                return Err(LinuxError::EIO);
            }
            
            // 写入到映射的地址空间
            aspace.write(alloc_addr, &buffer[..read_len as usize])
                .map_err(|e| {
                  //  ax_println!("sys_mmap: write to mapped area failed:{:?}",e);
                    LinuxError::EFAULT
                })?;
            
            //ax_println!("sys_mmap:file mapping,read{}bytes",read_len);
        }
        
     //   ax_println!("sys_mmap:success,returning {:#x}", alloc_addr);
        
        // 返回分配的地址
        Ok(alloc_addr.as_usize())
    })
}

fn sys_openat(dfd: c_int, fname: *const c_char, flags: c_int, mode: api::ctypes::mode_t) -> isize {
    assert_eq!(dfd, AT_FDCWD);
    api::sys_open(fname, flags, mode) as isize
}

fn sys_close(fd: i32) -> isize {
    api::sys_close(fd) as isize
}

fn sys_read(fd: i32, buf: *mut c_void, count: usize) -> isize {
    api::sys_read(fd, buf, count)
}

fn sys_write(fd: i32, buf: *const c_void, count: usize) -> isize {
    api::sys_write(fd, buf, count)
}

fn sys_writev(fd: i32, iov: *const api::ctypes::iovec, iocnt: i32) -> isize {
    unsafe { api::sys_writev(fd, iov, iocnt) }
}

fn sys_set_tid_address(tid_ptd: *const i32) -> isize {
    let curr = current();
    curr.task_ext().set_clear_child_tid(tid_ptd as _);
    curr.id().as_u64() as isize
}

fn sys_ioctl(_fd: i32, _op: usize, _argp: *mut c_void) -> i32 {
    ax_println!("Ignore SYS_IOCTL");
    0
}




//向traphand列表注册pagefault函数->真正的模块分离！！！ 这样traphandler就可以做到分离模块
#[register_trap_handler(PAGE_FAULT)]
fn pagefault_handler_to_area_backend(viradr: memory_addr::VirtAddr, mapflag: MappingFlags, from_user: bool) -> bool {
    //接下来应该传入maparea的backend端来处理页帧分配和映射逻辑,还是模块分离，减少代码耦合！！
    
    //参考了答案梳理执行流程
    if from_user {//不处理没有user映射权限的区域
        if !current()
            .task_ext()
            .aspace
            .lock()
            .handle_page_fault(viradr, mapflag)//传到area的backend处理
        {
            ax_println!("{}: segmentation fault, exit!", current().id_name());
            axtask::exit(-1);
        } else {
            ax_println!("{}: handle page fault OK!", current().id_name());
        }
        true
    } else {
        false
    }
    
    //trap的cause识别->通过traphand函数列表进行分发做到职责分离->pagefault处理函数仅仅处理权限然后传到->任务地址空间的pagefault处理找到对应触发pagefult地址所属area->传到area的backend来处理实际的分配逻辑和页表映射！ 
}

