#![no_std]

use allocator::{AllocResult, AllocError, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
pub struct EarlyAllocator<const PAGE_SIZE: usize> {
    start: usize,
    end: usize,
    byte_pos: usize,
    page_pos: usize,
}

impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    pub const fn new() -> Self {
        EarlyAllocator {
            start: 0,
            end: 0,
            byte_pos: 0,
            page_pos: 0,
        }
    }
}

impl<const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    /// Initialize the allocator with a free memory region.
    fn init(&mut self, start: usize, size: usize) {
        self.end = start + size;
        self.start = start;
        self.byte_pos = start;
        self.page_pos = self.end;
    }
    /// Add a free memory region to the allocator.
    fn add_memory(&mut self, _start: usize, _size: usize) -> AllocResult {
        Ok(())
    }
}

impl<const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    /// Allocate memory with the given size (in bytes) and alignment.
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let size = layout.size();
        let align = layout.align();
        let mask = align - 1;
        let b_pos = (self.byte_pos + mask) & !mask;
        let b_end = b_pos + size;
        if b_end > self.page_pos {
            return Err(AllocError::NoMemory);
        }
        self.byte_pos = b_end;
        Ok(NonNull::new(b_pos as *mut u8).unwrap())
    }

    /// Deallocate memory at the given position, size, and alignment.
    fn dealloc(&mut self, pos: NonNull<u8>, layout: Layout) {
        let size = layout.size();
        let b_end = pos.as_ptr() as usize + size;
        if b_end == self.byte_pos {
            self.byte_pos = pos.as_ptr() as usize;
        }
    }

    /// Returns total memory size in bytes.
    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    /// Returns allocated memory size in bytes.
    fn used_bytes(&self) -> usize {
        self.byte_pos - self.start
    }

    /// Returns available memory size in bytes.
    fn available_bytes(&self) -> usize {
        self.page_pos - self.byte_pos
    }
}

impl<const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    /// The size of a memory page.
    const PAGE_SIZE: usize = PAGE_SIZE;

    /// Allocate contiguous memory pages with given count and alignment.
    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        let align = 1 << align_pow2;
        let mask = align - 1;
        let p_end = self.page_pos & !mask;
        let p_pos = p_end - num_pages * PAGE_SIZE;
        if p_pos < self.byte_pos {
            return Err(AllocError::NoMemory);
        }
        self.page_pos = p_pos;
        Ok(p_pos)
    }

    /// Deallocate contiguous memory pages with given position and count.
    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {
        let p_end = pos + num_pages * PAGE_SIZE;
        if p_end == self.page_pos {
            self.page_pos = pos;
        }
    }

    /// Returns the total number of memory pages.
    fn total_pages(&self) -> usize {
        (self.end - self.byte_pos) / PAGE_SIZE
    }

    /// Returns the number of allocated memory pages.
    fn used_pages(&self) -> usize {
        (self.end - self.page_pos) / PAGE_SIZE
    }

    /// Returns the number of available memory pages.
    fn available_pages(&self) -> usize {
        (self.page_pos - self.byte_pos) / PAGE_SIZE
    }
}
