//!
//! Arenas are large memory areas (usually 1GiB+) from which mimalloc allocates memory.
//! The arenas are usually allocated on-demand from the OS but can be reserved explicitly.
//! It is also possible to give previously allocated memory to mimalloc to manage.
//! Heaps can be associated with a specific arena to only allocate memory from that arena.
//!

pub use crate::extended::Heap;

/// Memory strategy to use when creating new arenas
#[derive(Debug, Clone, Copy)]
pub enum MemoryStrat {
    /// Commit Memory upfront
    /// [usize] how much memory to commit upfront in bytes
    Commit(usize),
    /// Reserve Memory, dont' commit
    /// [usize] how much memory to reserve for use by [Arena]
    Reserve(usize),
    /// Same as [MemoryStrat::Commit], but enables the usage
    /// of large pages (2MiB+)
    CommitLarge(usize),
    /// Same as [MemoryStrat::Reserve], but enables the usage
    /// of large pages (2MiB+)
    ReserveLarge(usize),
}

/// A Transparent wrapper type around a [mi::mi_arena_id_t]
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Arena(mi::mi_arena_id_t);

impl Arena {
    /// Reserve OS memory for use by mimalloc. Reserved areas are used
    /// before allocating from the OS again. By reserving a large area upfront,
    /// allocation can be more efficient, and can be better managed on systems
    /// without `mmap`/`VirtualAlloc` (like WASM for example).
    ///
    ///
    /// See: [MemoryStrat] for details on configuring the memory size
    /// and commit/reserve strategy for creating a new mimalloc Arena
    ///
    /// To create an exclusive arena, which only allows allocations to be
    /// made out of the newly created Arena
    ///
    /// This calls [mi::mi_reserve_os_memory_ex] internally, so this
    /// function might fail, in which case an error message with the
    /// propagated ERRNO value is returned instead
    ///
    #[inline]
    pub fn new(strat: MemoryStrat) -> anyhow::Result<Self> {
        Self::with_exclusive(strat, false)
    }

    #[inline]
    pub fn heap_new(&self) -> Heap {
        Heap::new_in_arena(self)
    }

    pub const fn id(&self) -> mi::mi_arena_id_t {
        self.0
    }

    ///
    /// Same as [Self::new], but only allows allocations if specifically for this arena
    /// (i.e.) only heaps assocaiated with this arena can allocate from this arena
    ///
    /// Reserve OS memory for use by mimalloc. Reserved areas are used
    /// before allocating from the OS again. By reserving a large area upfront,
    /// allocation can be more efficient, and can be better managed on systems
    /// without `mmap`/`VirtualAlloc` (like WASM for example).
    ///
    ///
    /// See: [MemoryStrat] for details on configuring the memory size
    /// and commit/reserve strategy for creating a new mimalloc Arena
    ///
    /// This calls [mi::mi_reserve_os_memory_ex] internally, so this
    /// function might fail, in which case an error message with the
    /// propagated ERRNO value is returned instead
    ///
    ///
    ///
    #[inline]
    pub fn exclusive(strat: MemoryStrat) -> anyhow::Result<Self> {
        Self::with_exclusive(strat, true)
    }

    /// Helper to keep [Self::new] and [Self::exclusive] DRY
    fn with_exclusive(strat: MemoryStrat, exclusive: bool) -> anyhow::Result<Self> {
        let mut id = -1;
        let err = match strat {
            MemoryStrat::Commit(commit) => {
                Self::reserve_os_memory_ex(commit, true, false, exclusive, &raw mut id)
            }
            MemoryStrat::Reserve(reserve) => {
                Self::reserve_os_memory_ex(reserve, false, false, exclusive, &raw mut id)
            }
            MemoryStrat::CommitLarge(commit_large) => {
                Self::reserve_os_memory_ex(commit_large, true, true, exclusive, &raw mut id)
            }
            MemoryStrat::ReserveLarge(reserve_large) => {
                Self::reserve_os_memory_ex(reserve_large, false, true, exclusive, &raw mut id)
            }
        };

        if err != 0 || id == -1 {
            anyhow::bail!("Mimalloc failed to reserve os memory! ERRNO: {err}");
        } else {
            Ok(Self(id))
        }
    }

    /// Wrapper helper method for [mi::mi_reserve_os_memory_ex]
    ///
    /// Reserve OS memory for use by mimalloc. Reserved areas are used
    /// before allocating from the OS again. By reserving a large area upfront,
    /// allocation can be more efficient, and can be better managed on systems
    /// without `mmap`/`VirtualAlloc` (like WASM for example).
    ///
    /// - `size` The size to reserve.
    /// - `commit` Commit the memory upfront.
    /// - `allow_large` Allow large OS pages (2MiB) to be used?
    /// - `exclusive` Only allow allocations if specifically for this arena.
    /// - `arena_id` Pointer who's value will be set to the new arena_id if successful.
    ///
    /// Returns 0 if successful, and an error code otherwise (e.g. `ENOMEM`)
    #[inline(always)]
    fn reserve_os_memory_ex(
        size: usize,
        commit: bool,
        allow_large: bool,
        exclusive: bool,
        arena_id: *mut mi::mi_arena_id_t,
    ) -> i32 {
        unsafe { mi::mi_reserve_os_memory_ex(size, commit, allow_large, exclusive, arena_id) }
    }
}

// TODO: Implement a Rust API that abstracts [mi::mi_manage_os_memory_ex]
// This is a little challenging, as [mi::mi_manage_os_memory_ex] takes a pointer to memory that has already been allocated
// by a user somewhere (or could be a statically sized buffer). So in order for all this to remain memory-safe, the memory passed
// to [mi::mi_manage_os_memory_ex] MUST remain alive and valid for the entire duration of the created arena (through [mi::mi_manage_os_memory_ex]).
//
// Id also like for the user to be able to pass in static or dynamic memory and have it 'just werk'. This is pretty cool because Rust's borrow checker can help
// us make sure that memory passed to [mi::mi_manage_os_memory_ex] lives for as long as it is used by the arena it creates
//
// pub struct ArenaMemory<Buf> {
//     /// is the memory we are using all commited?
//     committed: bool,
//     /// Does the memory use large OS Pages?
//     is_large: bool,
//     /// Is the memory completely zeroed?
//     is_zeroed: bool,
//     /// Can this only be allocated through an exclusive arena?
//     exclusive: bool,

//     /// Memory to be used by mimalloc.
//     /// I chose to have this field be a genereic type
//     /// so that it makes more sense for someone who might not want to allocate with
//     /// a Vec but instead use a statically allocated byte buffer
//     mem: Buf,
// }

// #[derive(Debug, Clone, Copy, Default)]
// pub struct MemInfo {
//     pub committed: bool,
//     pub is_large: bool,
//     pub is_zeroed: bool,
//     pub exclusive: bool,
//     pub size: usize,
// }

// pub type ArenaMemoryStatic<const S: usize> = ArenaMemory<[u8; S]>;
// pub type ArenaMemoryVec<A: Allocator> = ArenaMemory<Vec<u8, A>>;
// pub type ArenaMemoryBuf<'a> = ArenaMemory<&'a mut [u8]>;
// pub type ArenaMemoryPtr = ArenaMemory<NonNull<[u8]>>;

// impl<B> ArenaMemory<B> {}

// /// Essentiallyh the same as [Arena],
// /// but tracks a mutable byte buffer that will be used by
// /// newly created [Arena]. This type is to ensure
// /// that the mutable buffer of bytes lives for at least as long
// /// as [ScopedArena]
// pub struct ExternArena<'arena, Memory: Pointer> {
//     inner: Arena,
//     mem: Memory,
//     _pd: PhantomData<&'arena Memory>,
// }

// impl<'a, M> ExternArena<'a, M> {
//     pub fn new(mem: M, info: MemInfo) -> anyhow::Result<Self> {}

//     /// Manage a particular memory area for use by mimalloc.
//     /// This is just like `mi_reserve_os_memory_ex` except that the area should already be
//     /// allocated in some manner and available for use my mimalloc.
//     ///
//     /// # Safety
//     /// mimalloc will likely segfault when allocating from the arena if the arena `start` & `size`
//     /// aren't aligned with mimalloc's `MI_SEGMENT_ALIGN` (e.g. 32MB on x86_64 machines).
//     ///
//     /// - `start` Start of the memory area
//     /// - `size` The size of the memory area. Must be large than `MI_ARENA_BLOCK_SIZE` (e.g. 64MB
//     ///          on x86_64 machines).
//     /// - `commit` Set true if the memory range is already commited.
//     /// - `is_large` Set true if the memory range consists of large files, or if the memory should
//     ///              not be decommitted or protected (like rdma etc.).
//     /// - `is_zero` Set true if the memory range consists only of zeros.
//     /// - `numa_node` Possible associated numa node or `-1`.
//     /// - `exclusive` Only allow allocations if specifically for this arena.
//     /// - `arena_id` Pointer who's value will be set to the new arena_id if successful.
//     ///
//     /// Returns `true` if arena was successfully allocated
//     fn manage_os_memory_ex(
//         start: NonNull<[u8]>,
//         is_committed: bool,
//         is_large: bool,
//         is_zero: bool,
//         numa_node: i32,
//         exclusive: bool,
//         arena_id: *mut mi_arena_id_t,
//     ) -> bool {
//         unsafe {
//             mi::mi_manage_os_memory_ex(
//                 start.as_ptr() as *const _,
//                 start.len(),
//                 is_committed,
//                 is_large,
//                 is_zero,
//                 numa_node,
//                 exclusive,
//                 arena_id,
//             )
//         }
//     }
// }
