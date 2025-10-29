#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
extern crate axstd as std;
extern crate alloc;
use axstd::io;
use axhal::paging::MappingFlags;
use axhal::arch::UspaceContext;
use axhal::mem::VirtAddr;
use axsync::Mutex;
use alloc::sync::Arc;
use axmm::AddrSpace;
use axtask::TaskExtRef;
use axhal::trap::{register_trap_handler, PAGE_FAULT};


//向traphand列表注册pagefualt函数->真正的模块分离！！！ 这样traphandler就可以做到分离模块
#[register_trap_handler(PAGE_FAULT)]
fn pagefault_handler_to_area_backend(viradr: VirtAddr, mapflag: MappingFlags, from_user: bool) -> bool {
//接下来应该传入maparea的backend端来处理页帧分配和映射逻辑,还是模块分离，减少代码耦合！！

//参考了答案梳理执行流程
    if from_user {//不处理没有user映射权限的区域
        if !axtask::current()
            .task_ext()
            .aspace
            .lock()
            .handle_page_fault(viradr, mapflag)//传到area的backend处理
        {
            ax_println!("{}: segmentation fault, exit!", axtask::current().id_name());
            axtask::exit(-1);
        } else {
            ax_println!("{}: handle page fault OK!", axtask::current().id_name());
        }
        true
    } else {
        false
    }

//trap的cause识别->通过traphand函数列表进行分发做到职责分离->pagefault处理函数仅仅处理权限然后传到->任务地址空间的pagefault处理找到对应触发pagefult地址所属area->传到area的backend来处理实际的分配逻辑和页表映射！ 


}

