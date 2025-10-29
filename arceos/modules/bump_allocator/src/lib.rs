#![no_std]

use allocator::{BaseAllocator, ByteAllocator, PageAllocator, AllocError};
use core::ptr::NonNull;

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,       //起始地址
    end: usize,           //结束地址
    b_pointer: usize,      // 字节分配指针（向右移动）
    p_pointer: usize,      // 页分配指针（向左移动）
    alloc_count: usize,      // 字节分配计数
}

/// 向上对齐到 align 的倍数
#[inline]
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// 向下对齐到 align 的倍数
#[inline]
fn align_down(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}

//[start,end) 左闭右开区间
impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pointer: 0, //字节指针
            p_pointer: 0, //页指针
            alloc_count: 0,
        }
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pointer = self.start;
        self.p_pointer = self.end;  // 是左闭右开区间
        self.alloc_count = 0;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> allocator::AllocResult {
        Err(AllocError::NoMemory)
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(
        &mut self,
        layout: core::alloc::Layout,
    ) -> allocator::AllocResult<core::ptr::NonNull<u8>> {
        //1对齐当前指针到所需的对齐边界
        let alloc_start = align_up(self.b_pointer, layout.align());
        
        //2计算分配结束位置
        let alloc_end = alloc_start
            .checked_add(layout.size())
            .ok_or(AllocError::NoMemory)?;
        
        //3检查是否与页分配区域冲突
        if alloc_end > self.p_pointer {
            return Err(AllocError::NoMemory);
        }
        //4更新字节指针
        self.b_pointer = alloc_end;
        self.alloc_count += 1;
        Ok(NonNull::new(alloc_start as *mut u8).unwrap())
    }

    fn dealloc(&mut self, pos: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        // 减少分配计数
        if self.alloc_count > 0 {
            self.alloc_count -= 1;
        }
        // 当所有分配都释放后，重置字节区域
        if self.alloc_count == 0 {
            self.b_pointer = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        // 总的可用(字节)!数
        self.p_pointer.saturating_sub(self.start)
    }

    fn used_bytes(&self) -> usize {
        //已使用的字节数
        self.b_pointer.saturating_sub(self.start)
    }

    fn available_bytes(&self) -> usize {
        //可用的字节数
        self.p_pointer.saturating_sub(self.b_pointer)
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> allocator::AllocResult<usize> {
        //1计算需要分配的总大小
        let alloc_size = num_pages
            .checked_mul(SIZE)
            .ok_or(AllocError::NoMemory)?;
        //2计算新的页指针位置（向左移动）
        let new_p_pointer = self.p_pointer
            .checked_sub(alloc_size)
            .ok_or(AllocError::NoMemory)?;
        
        //3向下对齐到指定的对齐边界
        let alloc_start = align_down(new_p_pointer, align_pow2);
        
        //4检查是否与字节分配区域冲突
        if alloc_start < self.b_pointer {
            return Err(AllocError::NoMemory);
        }
        
        //5更新页指针
        self.p_pointer = alloc_start;
        
        //6返回分配的起始地址
        Ok(alloc_start)
    }

    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {
        //2个指针,中间不能恰
    }

    fn total_pages(&self) -> usize {
        // 总的可用页数
        (self.end - self.start) / SIZE
    }

    fn used_pages(&self) -> usize {
        // 已使用的页数
        (self.end - self.p_pointer) / SIZE
    }

    fn available_pages(&self) -> usize {
        // 可用的页数（考虑字节区域占用）
        self.p_pointer.saturating_sub(self.b_pointer) / SIZE
    }
}