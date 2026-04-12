// Copyright 2019 Octavian Oncescu

#![no_std]
#![feature(allocator_api)]

//! A drop-in global allocator wrapper around the [mimalloc](https://github.com/microsoft/mimalloc) allocator.
//! Mimalloc is a general purpose, performance oriented allocator built by Microsoft.
//!
//! ## Usage
//! ```rust,ignore
//! use mimalloc::MiMalloc;
//!
//! #[global_allocator]
//! static GLOBAL: MiMalloc = MiMalloc;
//! ```
//!
//! ## Usage with secure mode
//! Using secure mode adds guard pages,
//! randomized allocation, encrypted free lists, etc. The performance penalty is usually
//! around 10% according to [mimalloc's](https://github.com/microsoft/mimalloc)
//! own benchmarks.
//!
//! To enable secure mode, put in `Cargo.toml`:
//! ```rust,ignore
//! [dependencies]
//! mimalloc = { version = "*", features = ["secure"] }
//! ```

extern crate alloc;
extern crate libmimalloc_sys as ffi;

#[cfg(feature = "extended")]
mod extended;

use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
use ffi::*;

/// Drop-in mimalloc global allocator.
///
/// ## Usage
/// ```rust,ignore
/// use mimalloc::MiMalloc;
///
/// #[global_allocator]
/// static GLOBAL: MiMalloc = MiMalloc;
/// ```
pub struct MiMalloc;

unsafe impl GlobalAlloc for MiMalloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { mi_malloc_aligned(layout.size(), layout.align()) as *mut u8 }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { mi_zalloc_aligned(layout.size(), layout.align()) as *mut u8 }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { mi_free(ptr as *mut c_void) };
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { mi_realloc_aligned(ptr as *mut c_void, new_size, layout.align()) as *mut u8 }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use crate::extended::Heap;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Buf<const S: usize>([u8; S]);

    impl<const S: usize> Buf<S> {
        pub const fn new() -> Self {
            Self([0u8; S])
        }
    }

    #[test]
    fn heap_resizes_vec() {
        let h = Heap::new();
        let mut v = Vec::with_capacity_in(4, h.clone());
        let bs = [Buf::<255>::new(); 255];

        // try to cause vec to grow memory as much as possible
        v.extend_from_slice(&bs[0..50]);
        v.extend_from_slice(&bs[50..150]);
        v.extend_from_slice(&bs[150..175]);
        v.extend_from_slice(&bs[175..200]);
        v.extend_from_slice(&bs[200..]);

        let v2 = v.clone();

        assert_eq!(v, v2);
    }

    #[test]
    fn it_frees_allocated_memory() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc(layout);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_allocated_big_memory() {
        unsafe {
            let layout = Layout::from_size_align(1 << 20, 32).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc(layout);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_zero_allocated_memory() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc_zeroed(layout);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_zero_allocated_big_memory() {
        unsafe {
            let layout = Layout::from_size_align(1 << 20, 32).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc_zeroed(layout);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_reallocated_memory() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc(layout);
            let ptr = alloc.realloc(ptr, layout, 16);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_reallocated_big_memory() {
        unsafe {
            let layout = Layout::from_size_align(1 << 20, 32).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc(layout);
            let ptr = alloc.realloc(ptr, layout, 2 << 20);
            alloc.dealloc(ptr, layout);
        }
    }
}
