use alloc::{alloc::Allocator, ffi::CString, sync::Arc};

use crate::MiMalloc;
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
    ptr::NonNull,
    str::FromStr,
};

impl MiMalloc {
    /// Get the mimalloc version.
    ///
    /// For mimalloc version 1.8.6, this will return 186.
    pub fn version(&self) -> u32 {
        unsafe { ffi::mi_version() as u32 }
    }

    /// Return the amount of available bytes in a memory block.
    ///
    /// # Safety
    /// `ptr` must point to a memory block allocated by mimalloc, or be null.
    #[inline]
    pub unsafe fn usable_size(&self, ptr: *const u8) -> usize {
        unsafe { ffi::mi_usable_size(ptr as *const c_void) }
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Heap(Arc<ffi::mi_heap_t, HeapManager>);

#[derive(Debug, Clone, Copy)]
struct HeapManager;

const PLACEHOLDER: [u8; 1] = [255];

unsafe impl alloc::alloc::Allocator for HeapManager {
    fn allocate(&self, _layout: Layout) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        let h = unsafe { ffi::mi_heap_new() };
        let p = h.cast::<u8>();
        let sl = NonNull::slice_from_raw_parts(NonNull::new(p).unwrap(), 1);
        Ok(sl)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, _layout: Layout) {
        // ensure this is a heap we are deleting
        debug_assert!(unsafe { ffi::mi_is_in_heap_region(ptr.as_ptr() as *const _) });
        let h = ptr.cast::<ffi::mi_heap_t>();
        unsafe { ffi::mi_heap_delete(h.as_ptr() as *mut _) }
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        self.allocate(layout)
    }

    unsafe fn grow(
        &self,
        _ptr: NonNull<u8>,
        _old_layout: Layout,
        _new_layout: Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        Ok(NonNull::from_ref(&PLACEHOLDER))
    }

    unsafe fn grow_zeroed(
        &self,
        _ptr: NonNull<u8>,
        _old_layout: Layout,
        _new_layout: Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        Ok(NonNull::from_ref(&PLACEHOLDER))
    }

    unsafe fn shrink(
        &self,
        _ptr: NonNull<u8>,
        _old_layout: Layout,
        _new_layout: Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        Ok(NonNull::from_ref(&PLACEHOLDER))
    }

    fn by_ref(&self) -> &Self
    where
        Self: Sized,
    {
        self
    }
}

impl Heap {
    pub fn new() -> Self {
        let h = unsafe { ffi::mi_heap_new() };
        // NOTE: here we pass in [HeapManager] as an allocator to ensure that when this [Arc] gets dropped,
        // the pointer it 'owns' is casted to a [ffi::mi_heap_t] and passed to [ffi::mi_heap_delete]
        let p = unsafe { Arc::from_raw_in(h, HeapManager) };
        Self(p)
    }

    #[inline]
    pub fn as_ptr(&self) -> NonNull<ffi::mi_heap_t> {
        NonNull::from_ref(self.0.as_ref())
    }

    #[inline]
    pub fn as_mut_ptr(&self) -> *mut ffi::mi_heap_t {
        core::ptr::from_ref(self.0.as_ref()) as *mut _
    }

    pub fn malloc(&self, layout: Layout) -> Option<NonNull<[u8]>> {
        let size = layout.size();
        let align = layout.align();
        let ptr = unsafe { ffi::mi_heap_malloc_aligned(self.as_mut_ptr(), size, align) };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let ptr = ptr.cast::<u8>();
        let sl = NonNull::slice_from_raw_parts(ptr, size);
        Some(sl)
    }

    pub fn zalloc(&self, layout: Layout) -> Option<NonNull<[u8]>> {
        let size = layout.size();
        let align = layout.align();
        let ptr = unsafe { ffi::mi_heap_zalloc_aligned(self.as_mut_ptr(), size, align) };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let ptr = ptr.cast::<u8>();
        let sl = NonNull::slice_from_raw_parts(ptr, size);
        Some(sl)
    }

    pub fn calloc(&self, count: usize, layout: Layout) -> Option<NonNull<[u8]>> {
        let size = layout.size();
        let align = layout.align();

        let ptr = unsafe { ffi::mi_heap_calloc_aligned(self.as_mut_ptr(), count, size, align) };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let ptr = ptr.cast::<u8>();
        let sl = NonNull::slice_from_raw_parts(ptr, size);
        Some(sl)
    }

    pub fn malloc_small(&self, layout: Layout) -> Option<NonNull<[u8]>> {
        let size = layout.size();
        if size >= ffi::MI_SMALL_SIZE_MAX {
            None
        } else {
            let ptr = unsafe { ffi::mi_heap_malloc_small(self.as_mut_ptr(), size) };
            let Some(ptr) = NonNull::new(ptr) else {
                return None;
            };
            let sl = NonNull::slice_from_raw_parts(ptr.cast::<u8>(), size);
            Some(sl)
        }
    }

    pub fn realloc(&self, ptr: NonNull<u8>, new_layout: Layout) -> Option<NonNull<[u8]>> {
        let ptr = unsafe {
            ffi::mi_heap_realloc_aligned(
                self.as_mut_ptr(),
                ptr.as_ptr() as *mut _,
                new_layout.size(),
                new_layout.align(),
            )
        };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let sl = NonNull::slice_from_raw_parts(ptr.cast::<u8>(), new_layout.size());
        Some(sl)
    }

    pub fn rezalloc(&self, ptr: NonNull<u8>, new_layout: Layout) -> Option<NonNull<[u8]>> {
        let ptr = unsafe {
            ffi::mi_heap_rezalloc_aligned(
                self.as_mut_ptr(),
                ptr.as_ptr() as *mut _,
                new_layout.size(),
                new_layout.align(),
            )
        };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let sl = NonNull::slice_from_raw_parts(ptr.cast::<u8>(), new_layout.size());
        Some(sl)
    }

    pub fn recalloc(
        &self,
        ptr: NonNull<u8>,
        new_count: usize,
        layout: Layout,
    ) -> Option<NonNull<[u8]>> {
        let size = layout.size();
        let align = layout.align();
        let ptr = unsafe {
            ffi::mi_heap_recalloc_aligned(
                self.as_mut_ptr(),
                ptr.as_ptr() as *mut _,
                new_count,
                size,
                align,
            )
        };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let sl = NonNull::slice_from_raw_parts(ptr.cast::<u8>(), size);
        Some(sl)
    }

    /// wraps [ffi::mi_heap_strndup]. Keep in mind since
    /// this requires converting a rust [str] into a null-terminated [*const i8] c-style string
    /// in order to be able to pass given [&str] to [ffi::mi_heap_strndup]. It looks like
    /// [CString] tries to avoid extra allocations if it can, but thats not gauranteed and
    /// this mehod might end up allocating 2 copies of a string. One of them is freed and the string that
    /// is returned is now owned by the caller, but its good to keep all this in mind when using this method.
    ///
    /// Though to be honest i can't imagine why someone would call this method in Rust. Ive just implemented this method
    /// in order to stay consistent with the core mimalloc api
    pub fn strdup(&self, s: &str) -> Option<NonNull<str>> {
        let cstr = CString::new(s).ok()?;
        let ptr =
            unsafe { ffi::mi_heap_strndup(self.as_mut_ptr(), cstr.as_ptr() as *const _, s.len()) };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let ptr = str_from_raw_parts(ptr.as_ptr() as *const _, s.len());
        let res = NonNull::from_ref(ptr);
        Some(res)
    }

    /// wraps [ffi::mi_heap_get_backing]
    #[inline]
    pub fn get_backing() -> NonNull<ffi::mi_heap_t> {
        let h = unsafe { ffi::mi_heap_get_backing() };
        NonNull::new(h).expect("mi_heap_get_backing returned nullptr!")
    }

    /// wraps [ffi::mi_heap_get_default]
    #[inline]
    pub fn get_default() -> NonNull<ffi::mi_heap_t> {
        let h = unsafe { ffi::mi_heap_get_default() };
        NonNull::new(h).expect("mi_heap_get_default returned nullptr!")
    }
}

unsafe impl core::marker::Sync for Heap {}
unsafe impl core::marker::Send for Heap {}

unsafe impl Allocator for Heap {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        self.malloc(layout).ok_or(alloc::alloc::AllocError)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe { ffi::mi_free(ptr.as_ptr() as *mut _) }
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        self.zalloc(layout).ok_or(alloc::alloc::AllocError)
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        self.realloc(ptr, new_layout)
            .ok_or(alloc::alloc::AllocError)
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        self.rezalloc(ptr, new_layout)
            .ok_or(alloc::alloc::AllocError)
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        self.realloc(ptr, new_layout)
            .ok_or(alloc::alloc::AllocError)
    }

    fn by_ref(&self) -> &Self
    where
        Self: Sized,
    {
        self
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ScopedHeap(*mut ffi::mi_heap_t);

impl ScopedHeap {
    pub fn new() -> Self {
        let h = unsafe { ffi::mi_heap_new() };
        Self(h)
    }

    pub fn zalloc(&self, layout: Layout) -> Option<NonNull<[u8]>> {
        let size = layout.size();
        let align = layout.align();
        let ptr = unsafe { ffi::mi_heap_zalloc_aligned(self.0, size, align) };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let ptr = ptr.cast::<u8>();
        let sl = NonNull::slice_from_raw_parts(ptr, size);
        Some(sl)
    }

    pub fn malloc(&self, layout: Layout) -> Option<NonNull<[u8]>> {
        let size = layout.size();
        let align = layout.align();
        let ptr = unsafe { ffi::mi_heap_malloc_aligned(self.0, size, align) };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let ptr = ptr.cast::<u8>();
        let sl = NonNull::slice_from_raw_parts(ptr, size);
        Some(sl)
    }

    pub fn calloc(&self, count: usize, layout: Layout) -> Option<NonNull<[u8]>> {
        let size = layout.size();
        let align = layout.align();

        let ptr = unsafe { ffi::mi_heap_calloc_aligned(self.0, count, size, align) };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let ptr = ptr.cast::<u8>();
        let sl = NonNull::slice_from_raw_parts(ptr, size);
        Some(sl)
    }

    pub fn malloc_small(&self, layout: Layout) -> Option<NonNull<[u8]>> {
        let size = layout.size();
        if size >= ffi::MI_SMALL_SIZE_MAX {
            None
        } else {
            let ptr = unsafe { ffi::mi_heap_malloc_small(self.0, size) };
            let Some(ptr) = NonNull::new(ptr) else {
                return None;
            };
            let sl = NonNull::slice_from_raw_parts(ptr.cast::<u8>(), size);
            Some(sl)
        }
    }

    pub fn realloc(&self, ptr: NonNull<u8>, new_layout: Layout) -> Option<NonNull<[u8]>> {
        let ptr = unsafe {
            ffi::mi_heap_realloc_aligned(
                self.0,
                ptr.as_ptr() as *mut _,
                new_layout.size(),
                new_layout.align(),
            )
        };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let sl = NonNull::slice_from_raw_parts(ptr.cast::<u8>(), new_layout.size());
        Some(sl)
    }

    pub fn rezalloc(&self, ptr: NonNull<u8>, new_layout: Layout) -> Option<NonNull<[u8]>> {
        let ptr = unsafe {
            ffi::mi_heap_rezalloc_aligned(
                self.0,
                ptr.as_ptr() as *mut _,
                new_layout.size(),
                new_layout.align(),
            )
        };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let sl = NonNull::slice_from_raw_parts(ptr.cast::<u8>(), new_layout.size());
        Some(sl)
    }

    pub fn recalloc(
        &self,
        ptr: NonNull<u8>,
        new_count: usize,
        layout: Layout,
    ) -> Option<NonNull<[u8]>> {
        let size = layout.size();
        let align = layout.align();
        let ptr = unsafe {
            ffi::mi_heap_recalloc_aligned(self.0, ptr.as_ptr() as *mut _, new_count, size, align)
        };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let sl = NonNull::slice_from_raw_parts(ptr.cast::<u8>(), size);
        Some(sl)
    }

    /// wraps [ffi::mi_heap_strndup]. Keep in mind since
    /// this requires converting a rust [str] into a null-terminated [*const i8] c-style string
    /// in order to be able to pass given [&str] to [ffi::mi_heap_strndup]. It looks like
    /// [CString] tries to avoid extra allocations if it can, but thats not gauranteed and
    /// this mehod might end up allocating 2 copies of a string. One of them is freed and the string that
    /// is returned is now owned by the caller, but its good to keep all this in mind when using this method.
    ///
    /// Though to be honest i can't imagine why someone would call this method in Rust. Ive just implemented this method
    /// in order to stay consistent with the core mimalloc api
    pub fn strdup(&self, s: &str) -> Option<NonNull<str>> {
        let cstr = CString::new(s).ok()?;
        let ptr = unsafe { ffi::mi_heap_strndup(self.0, cstr.as_ptr() as *const _, s.len()) };
        let Some(ptr) = NonNull::new(ptr) else {
            return None;
        };
        let ptr = str_from_raw_parts(ptr.as_ptr() as *const _, s.len());
        let res = NonNull::from_ref(ptr);
        Some(res)
    }

    /// wraps [ffi::mi_heap_get_backing]
    #[inline]
    pub fn get_backing() -> NonNull<ffi::mi_heap_t> {
        let h = unsafe { ffi::mi_heap_get_backing() };
        NonNull::new(h).expect("mi_heap_get_backing returned nullptr!")
    }

    /// wraps [ffi::mi_heap_get_default]
    #[inline]
    pub fn get_default() -> NonNull<ffi::mi_heap_t> {
        let h = unsafe { ffi::mi_heap_get_default() };
        NonNull::new(h).expect("mi_heap_get_default returned nullptr!")
    }
}

/// custom impl of [core::str::from_raw_parts], since that requires
/// a feature flag to be enabled, so  screw that i just did it myself, i think
/// this way is better anyway, plus its [const] :D
const fn str_from_raw_parts<'a>(ptr: *const i8, len: usize) -> &'a str {
    let buf = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    let Ok(s) = core::str::from_utf8(buf) else {
        return "";
    };
    s
}

impl Drop for ScopedHeap {
    fn drop(&mut self) {
        unsafe {
            ffi::mi_heap_delete(self.0);
        }
    }
}

unsafe impl alloc::alloc::Allocator for ScopedHeap {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        let Some(ptr) = self.malloc(layout) else {
            return Err(alloc::alloc::AllocError);
        };
        Ok(ptr)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, _layout: Layout) {
        unsafe {
            ffi::mi_free(ptr.as_ptr() as *mut _);
        }
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        let Some(ptr) = self.zalloc(layout) else {
            return Err(alloc::alloc::AllocError);
        };
        Ok(ptr)
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        self.realloc(ptr, new_layout)
            .ok_or(alloc::alloc::AllocError)
        // core::debug_assert!(
        //     new_layout.size() >= old_layout.size(),
        //     "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        // );
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        core::debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );

        self.rezalloc(ptr, new_layout)
            .ok_or(alloc::alloc::AllocError)

        // let new_ptr = self.allocate_zeroed(new_layout)?;

        // // SAFETY: because `new_layout.size()` must be greater than or equal to
        // // `old_layout.size()`, both the old and new memory allocation are valid for reads and
        // // writes for `old_layout.size()` bytes. Also, because the old allocation wasn't yet
        // // deallocated, it cannot overlap `new_ptr`. Thus, the call to `copy_nonoverlapping` is
        // // safe. The safety contract for `dealloc` must be upheld by the caller.
        // unsafe {
        //     core::ptr::copy_nonoverlapping(
        //         core::ptr.as_ptr(),
        //         new_ptr.as_mut_ptr(),
        //         old_layout.size(),
        //     );
        //     self.deallocate(core::ptr, old_layout);
        // }

        // Ok(new_ptr)
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        core::debug_assert!(
            new_layout.size() <= old_layout.size(),
            "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
        );

        self.realloc(ptr, new_layout)
            .ok_or(alloc::alloc::AllocError)

        // let new_ptr = self.allocate(new_layout)?;

        // // SAFETY: because `new_layout.size()` must be lower than or equal to
        // // `old_layout.size()`, both the old and new memory allocation are valid for reads and
        // // writes for `new_layout.size()` bytes. Also, because the old allocation wasn't yet
        // // deallocated, it cannot overlap `new_ptr`. Thus, the call to `copy_nonoverlapping` is
        // // safe. The safety contract for `dealloc` must be upheld by the caller.
        // unsafe {
        //     core::ptr::copy_nonoverlapping(
        //         core::ptr.as_ptr(),
        //         new_ptr.as_mut_ptr(),
        //         new_layout.size(),
        //     );
        //     self.deallocate(core::ptr, old_layout);
        // }

        // Ok(new_ptr)
    }

    fn by_ref(&self) -> &Self
    where
        Self: Sized,
    {
        self
    }
}

#[cfg(test)]
mod test {
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    use super::*;
    use core::alloc::GlobalAlloc;
    use core::alloc::Layout;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[test]
    fn it_gets_version() {
        let version = MiMalloc.version();
        assert!(version != 0);
    }

    #[test]
    fn it_checks_usable_size() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc(layout);
            let usable_size = alloc.usable_size(ptr);
            alloc.dealloc(ptr, layout);
            assert!(usable_size >= 8);
        }
    }

    #[test]
    fn heap_works() {
        let h = Heap::new();
        let mut v = Vec::new_in(h.clone());
        v.push(100);
        v.push(200);
        v.push(300);

        assert_eq!(v[0], 100);
        assert_eq!(v[1], 200);
        assert_eq!(v[2], 300);

        let p = Box::new_in(Point { x: 100, y: 50 }, h.clone());

        assert_eq!(*p, Point { x: 100, y: 50 });
    }

    #[test]
    fn scoped_heap_works() {
        let h = ScopedHeap::new();

        let mut v = Vec::new_in(h.by_ref());
        v.push(100);
        v.push(200);
        v.push(300);

        assert_eq!(v[0], 100);
        assert_eq!(v[1], 200);
        assert_eq!(v[2], 300);

        let p = Box::new_in(Point { x: 100, y: 50 }, h.by_ref());

        assert_eq!(*p, Point { x: 100, y: 50 });
    }
}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout)
            .map(|p| p.as_ptr() as *mut u8)
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(ptr) = NonNull::new(ptr) else {
            return;
        };
        unsafe { self.deallocate(ptr, layout) };
    }
}
