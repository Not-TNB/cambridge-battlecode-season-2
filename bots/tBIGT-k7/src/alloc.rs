// Size-class allocator (v6). Heap allocation and freeing through the standard allocator (dlmalloc)
// took 30.9% of all our fuel: ~47 allocations a turn at ~187 fuel each, and frees at ~141 (2026-10-23
// local profile, tools/local/fuelscope.py). Small blocks (up to MAX_SMALL bytes, nearly all of
// ours) now come from a free list per size class, a few instructions each way; a class with an
// empty list carves new blocks from CHUNK-byte chunks taken from the standard allocator. Freed
// small blocks go back on their class's list and are never returned, so memory use is the peak
// per class. Larger blocks, and alignments above MAX_ALIGN, go to the standard allocator. The bot
// is single-threaded (WebAssembly), so the lists need no locking. Only how memory is handed out
// changes, never what the bot decides. Off: the standard allocator.
// 2026-10-23: allocation 71 fuel and free 45 (with the profiler's wrapper) against 187 and 141;
// heap 12.3% of fuel against 30.9%. Against the bot with the standard allocator, 416 games head to
// head (52 maps, both sides): 266/416 on seeds 0-3, 284/416 on seeds 4-7; fuel-exhausted turns
// 3.1% against 5.9%, gold at 500 +16%, gems tributed +25%, deaths 11.4 against 16.3. With
// DENSE_GROUP_AT and PARTIAL_SIGHT, against bot-v5: 300/416 (fuel-exhausted 3.2% against 6.4%).
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;

pub const SIZE_CLASSES: bool = true;
// Block sizes, each a multiple of MAX_ALIGN; a request takes the smallest that fits.
const CLASSES: [usize; 14] = [
    16, 32, 48, 64, 96, 128, 192, 256, 384, 512, 640, 768, 896, 1024,
];
const MAX_SMALL: usize = 1024;
const MAX_ALIGN: usize = 16;
const CHUNK: usize = 64 * 1024;

struct State {
    free: [*mut u8; CLASSES.len()], // each free block's first word points to the next
    next: *mut u8,                  // where the current chunk's unused space starts
    end: *mut u8,
}

pub struct SizeClassAlloc {
    state: UnsafeCell<State>,
}

// SAFETY: the bot runs on one thread (wasm32-wasip1), so the state is never touched concurrently.
unsafe impl Sync for SizeClassAlloc {}

impl SizeClassAlloc {
    pub const fn new() -> Self {
        Self {
            state: UnsafeCell::new(State {
                free: [std::ptr::null_mut(); CLASSES.len()],
                next: std::ptr::null_mut(),
                end: std::ptr::null_mut(),
            }),
        }
    }
}

// The size class for a layout, or None for the standard allocator.
fn class_of(layout: Layout) -> Option<usize> {
    if !SIZE_CLASSES || layout.size() > MAX_SMALL || layout.align() > MAX_ALIGN {
        return None;
    }
    let size = layout.size().max(1);
    CLASSES.iter().position(|&c| c >= size)
}

unsafe impl GlobalAlloc for SizeClassAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(class) = class_of(layout) else {
            return unsafe { System.alloc(layout) };
        };
        // SAFETY: single-threaded (see Sync above); no reference to the state outlives this call.
        let state = unsafe { &mut *self.state.get() };
        let head = state.free[class];
        if !head.is_null() {
            // SAFETY: a free block's first word holds the next free block of its class.
            state.free[class] = unsafe { *(head as *mut *mut u8) };
            return head;
        }
        let size = CLASSES[class];
        if (state.end as usize).saturating_sub(state.next as usize) < size {
            // SAFETY: CHUNK and MAX_ALIGN make a valid, non-zero layout.
            let chunk =
                unsafe { System.alloc(Layout::from_size_align_unchecked(CHUNK, MAX_ALIGN)) };
            if chunk.is_null() {
                return chunk;
            }
            // The rest of the old chunk is left unused (at most MAX_SMALL bytes).
            state.next = chunk;
            // SAFETY: inside the chunk just allocated.
            state.end = unsafe { chunk.add(CHUNK) };
        }
        let block = state.next;
        // SAFETY: at least `size` bytes remain in the chunk; sizes are multiples of MAX_ALIGN, so
        // blocks stay MAX_ALIGN-aligned.
        state.next = unsafe { block.add(size) };
        block
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(class) = class_of(layout) else {
            return unsafe { System.dealloc(ptr, layout) };
        };
        // SAFETY: single-threaded; `ptr` came from this class (same layout as when allocated) and
        // is at least one pointer wide (classes start at 16 bytes).
        let state = unsafe { &mut *self.state.get() };
        unsafe { *(ptr as *mut *mut u8) = state.free[class] };
        state.free[class] = ptr;
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller guarantees `new_size` with `layout.align()` is a valid layout.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        match (class_of(layout), class_of(new_layout)) {
            // Same class: the block already fits.
            (Some(a), Some(b)) if a == b => ptr,
            // Both large: the standard allocator can grow in place.
            (None, None) => unsafe { System.realloc(ptr, layout, new_size) },
            _ => {
                // SAFETY: as GlobalAlloc's default realloc: allocate, copy what fits, free the old.
                let new = unsafe { self.alloc(new_layout) };
                if !new.is_null() {
                    unsafe {
                        std::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
                        self.dealloc(ptr, layout);
                    }
                }
                new
            }
        }
    }
}
