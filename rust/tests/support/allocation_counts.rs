use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountingAllocator;

thread_local! {
    static ALLOCATIONS: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

fn record_allocation(bytes: usize) {
    ALLOCATIONS.with(|counter| {
        if let Some((count, total)) = counter.get() {
            counter.set(Some((count + 1, total + bytes)));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation(size);
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

pub fn measure(operation: impl FnOnce()) -> (usize, usize) {
    ALLOCATIONS.set(Some((0, 0)));
    operation();
    ALLOCATIONS.replace(None).unwrap()
}
