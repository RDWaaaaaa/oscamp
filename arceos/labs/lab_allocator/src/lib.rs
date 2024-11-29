//! Allocator algorithm in lab.

#![no_std]
#![allow(unused_variables)]

use allocator::{BaseAllocator, ByteAllocator, AllocResult};
use core::alloc::Layout;
use core::ptr::NonNull;

/// 对齐大小（最小块大小）
const ALIGNMENT: usize = 3; // 对齐单位是 2^3 = 8 字节
/// 一级索引数量
const FL_INDEX_COUNT: usize = 29; // 最大支持块大小 2GB
/// 二级索引数量
const SL_INDEX_COUNT: usize = 32; // 每一级索引分为 32 个子类

pub struct LabByteAllocator {
    fl_bitmap: u32,
    sl_bitmap: [u32; FL_INDEX_COUNT],
    free_blocks: [[Option<NonNull<FreeBlockHeader>>; SL_INDEX_COUNT]; FL_INDEX_COUNT],
    total_memory: usize,
    used_memory: usize,
}
unsafe impl Send for LabByteAllocator {}
unsafe impl Sync for LabByteAllocator {}

impl LabByteAllocator {
    /// 创建一个新的分配器
    pub const fn new() -> Self {
        Self {
            fl_bitmap: 0,
            sl_bitmap: [0; FL_INDEX_COUNT],
            free_blocks: [[None; SL_INDEX_COUNT]; FL_INDEX_COUNT],
            total_memory: 0,
            used_memory: 0,
        }
    }

    /// 映射大小到索引
    fn mapping(size: usize) -> (usize, usize) {
        // 计算高位索引 fl
        let mut fl = 0;
        let mut temp_size = size;
        while temp_size > 1 {
            fl += 1;
            temp_size >>= 1;
        }

        // 计算二级索引 sl
        let sl = (size >> (fl - SL_INDEX_COUNT)) & (SL_INDEX_COUNT - 1);

        (fl - 1, sl) // fl - 1 是因为 fl 从 1 开始计数
    }

    /// 插入空闲块
    fn insert_free_block(&mut self, block: NonNull<FreeBlockHeader>) {
        unsafe {
            let block_ref = block.as_ref();
            let size = block_ref.common.size;
            let (fl, sl) = Self::mapping(size);

            self.free_blocks[fl][sl] = Some(block);
            self.fl_bitmap |= 1 << fl;
            self.sl_bitmap[fl] |= 1 << sl;
        }
    }

    /// 从空闲链表中移除块
    fn remove_free_block(&mut self, fl: usize, sl: usize) -> Option<NonNull<FreeBlockHeader>> {
        let block = self.free_blocks[fl][sl];
        self.free_blocks[fl][sl] = None;

        if self.free_blocks[fl].iter().all(Option::is_none) {
            self.fl_bitmap &= !(1 << fl);
        }
        if block.is_some() {
            self.sl_bitmap[fl] &= !(1 << sl);
        }

        block
    }

    /// 查找合适的块
    fn find_suitable_block(&self, size: usize) -> Option<(usize, usize)> {
        let (fl, sl) = Self::mapping(size);

        for i in fl..FL_INDEX_COUNT {
            let sl_mask = if i == fl { self.sl_bitmap[i] & !((1 << sl) - 1) } else { self.sl_bitmap[i] };
            if sl_mask != 0 {
                let suitable_sl = sl_mask.trailing_zeros() as usize;
                return Some((i, suitable_sl));
            }
        }

        None
    }
}

impl BaseAllocator for LabByteAllocator {
    fn init(&mut self, start: usize, size: usize) {
        let aligned_start = (start + (1 << ALIGNMENT) - 1) & !((1 << ALIGNMENT) - 1);
        let aligned_size = size & !((1 << ALIGNMENT) - 1);

        let block = aligned_start as *mut FreeBlockHeader;
        unsafe {
            (*block).common.size = aligned_size;
            (*block).common.prev_phys_blk = None;
            (*block).next_free = None;
            (*block).prev_free = None;
        }
        self.insert_free_block(unsafe { NonNull::new_unchecked(block) });
        self.total_memory = aligned_size;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        let aligned_start = (start + (1 << ALIGNMENT) - 1) & !((1 << ALIGNMENT) - 1);
        let aligned_size = size & !((1 << ALIGNMENT) - 1);

        let block = aligned_start as *mut FreeBlockHeader;
        unsafe {
            (*block).common.size = aligned_size;
            (*block).common.prev_phys_blk = None;
            (*block).next_free = None;
            (*block).prev_free = None;
        }
        self.insert_free_block(unsafe { NonNull::new_unchecked(block) });

        Ok(())
    }
}

impl ByteAllocator for LabByteAllocator {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let size = (layout.size() + (1 << ALIGNMENT) - 1) & !((1 << ALIGNMENT) - 1);

        if let Some((fl, sl)) = self.find_suitable_block(size) {
            if let Some(block) = self.remove_free_block(fl, sl) {
                unsafe {
                    let block_ref = block.as_ref();
                    let block_size = block_ref.common.size;

                    if block_size > size + core::mem::size_of::<FreeBlockHeader>() {
                        let new_block = (block.as_ptr() as usize + size) as *mut FreeBlockHeader;
                        (*new_block).common.size = block_size - size;
                        self.insert_free_block(NonNull::new_unchecked(new_block));
                    }

                    self.used_memory += size;
                    return Ok(NonNull::new_unchecked(block.as_ptr() as *mut u8));
                }
            }
        }

        Err(allocator::AllocError::NoMemory)
    }

    fn dealloc(&mut self, pos: NonNull<u8>, layout: Layout) {
        let size = (layout.size() + (1 << ALIGNMENT) - 1) & !((1 << ALIGNMENT) - 1);
        let block = pos.as_ptr() as *mut FreeBlockHeader;

        unsafe {
            (*block).common.size = size;
            self.insert_free_block(NonNull::new_unchecked(block));
            self.used_memory -= size;
        }
    }

    fn total_bytes(&self) -> usize {
        self.total_memory
    }

    fn used_bytes(&self) -> usize {
        self.used_memory
    }

    fn available_bytes(&self) -> usize {
        self.total_memory - self.used_memory
    }
}

#[repr(C)]
struct BlockHeader {
    size: usize,
    prev_phys_blk: Option<NonNull<BlockHeader>>,
}

#[repr(C)]
struct FreeBlockHeader {
    common: BlockHeader,
    next_free: Option<NonNull<FreeBlockHeader>>,
    prev_free: Option<NonNull<FreeBlockHeader>>,
}
