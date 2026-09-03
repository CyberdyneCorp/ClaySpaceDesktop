//! One transport over three surfaces.
//!
//! A host at twenty million vertices asks three questions, and they are the
//! same three whichever representation it is holding: what changed, give me
//! those bytes into a buffer I own, and what does this cost me. Answering them
//! per representation is three host code paths whose dirty sets mean different
//! things — a weld class, a face chunk and a base patch are not
//! interchangeable, and a host that treated them as such would upload the
//! wrong thing. So there is one chunk unit underneath, and [`SurfaceView`] is
//! the seam over it.
//!
//! The shipped paths stay: [`crate::Multires::dirty_blocks`] and
//! [`crate::Multires::copy_block`] are unchanged and a host using them keeps
//! working. What this adds is what they structurally cannot express, and all
//! three survive into this API rather than being flattened into a dirty flag.
//!
//! # Four revisions, not one
//!
//! [`ChunkRevisions`] is four counters and no summary. One counter cannot say
//! "geometry moved, connectivity did not, normals are still deferred, colours
//! are unchanged", so a host that has only one re-uploads an index buffer on
//! every dab. Compare [`topology`](ChunkRevisions::topology) to decide whether
//! the index buffer has to go up again; compare
//! [`normals`](ChunkRevisions::normals) to tell a deferred flush from a move.
//! They are drawn from one table-wide sequence, so comparing them *across*
//! chunks is meaningful too — "this chunk changed after that one" is a
//! question a host draining incrementally actually asks.
//!
//! # An acknowledgement, not an all-or-nothing clear
//!
//! [`SurfaceView::clear_dirty`] is the shipped form and it is still here for a
//! host that uploads everything it was told about in one frame. A host that
//! drains half a set and then drops a frame must not be made to choose between
//! re-uploading everything and losing a change, so
//! [`SurfaceView::acknowledge`] retires a chunk **only if its current revision
//! still matches the one the caller actually copied**. A chunk that changed
//! again in between stays dirty. Draining is therefore lossless at any rate,
//! which is what makes a preview budget a hint rather than a lie.
//!
//! The acknowledgement is modelled as a [`ChunkAck`], and the one way to build
//! one is [`ChunkCopy::ack`] — from what the copy *reported*, never from what
//! a host wishes it had seen. An acknowledgement assembled by hand from a
//! stale plan retires a change nobody uploaded, and nothing in the pixels says
//! so afterwards.
//!
//! # A stale readback says so
//!
//! [`ChunkReadback`] carries the revision the caller asked for beside what the
//! engine is at now, and [`stale`](ChunkReadback::stale) when they differ. The
//! data written is always *current* — this is not a failure — but a host
//! applying an older frame's plan can tell that its plan is out of date. A
//! host that draws a stale chunk draws something the engine does not think it
//! made.
//!
//! # A short buffer is retryable and an invalid argument is not
//!
//! The two mean opposite things to a host: grow and ask again against the call
//! itself was wrong. A drain loop written against the general pattern treats
//! the first as a fault and drops the chunk. So the retry is not offered here,
//! it is *taken*: [`SurfaceView::dirty_chunks`] and
//! [`SurfaceView::copy_chunk`] size themselves from the counts the engine
//! writes and hand back an owned buffer, so a caller never sees a truncation
//! and cannot conflate the two. [`SurfaceView::chunk_capacity`] is the
//! capacity query on its own, for a host sizing a pool up front.
//!
//! # The view is a call-site convenience, not a handle to store
//!
//! It names a surface it does not own and holds no copy of the geometry:
//! everything it reports is read at the moment it is asked. The borrow in
//! [`SurfaceView`]'s lifetime is what says so, and it is why a mutation to the
//! surface — which needs the surface back — ends the view.
//!
//! # A level's chunk table exists once the level has been looked at
//!
//! On a hierarchy the table comes into being when the level is first viewed,
//! so a stamp made before that has nothing to mark and the dirty set is empty
//! — correct, and surprising if a host sculpts a level it has never drawn. A
//! host that draws every frame has primed it by definition; one that does not
//! should take a view and read [`SurfaceView::chunk_infos_in_order`] once
//! before it starts, which is what a first frame would have done.
//!
//! # What is deliberately not here
//!
//! `clay_surface_view_from_dynamic` takes a `clay_dynamic_sculptor`, which
//! this crate does not wrap: there is no adaptive surface in this workspace
//! yet. A constructor for it would be an unconstructible type or a raw pointer
//! crossing a safe boundary, and this crate's rule is that a wrapper nobody
//! runs is a SAFETY comment nobody has checked. It arrives with the adaptive
//! surface, which is the change that can also run it.

use std::marker::PhantomData;
use std::ptr::NonNull;

use claycore_sys as sys;

use crate::buffer::size_query_array;
use crate::descriptor::Descriptor;
use crate::error::{check, ErrorKind, Result};
use crate::mesh::Mesh;
use crate::multires::Multires;
use crate::raw_failure;

// -- what a chunk is --------------------------------------------------------

/// Which representation is underneath.
///
/// It changes exactly one thing a caller can see, and
/// [`SurfaceView::copy_chunk`] says which: a fixed mesh and a hierarchy level
/// have a stable per-chunk vertex list and copy as their own vertices with
/// local indices, while an adaptive surface's topology changes under the stamp
/// being uploaded and copies as unwelded triangles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceKind {
    Fixed,
    Adaptive,
    Multires,
    /// A kind this build does not know, carried verbatim rather than mapped
    /// onto one it does — a host branching on `Fixed` because the number was
    /// unfamiliar would assume a welding rule that does not hold.
    Unknown(i32),
}

impl SurfaceKind {
    fn from_raw(code: i32) -> Self {
        use sys::clay_surface_kind as k;
        match code as sys::clay_surface_kind::Type {
            k::CLAY_SURFACE_FIXED => Self::Fixed,
            k::CLAY_SURFACE_ADAPTIVE => Self::Adaptive,
            k::CLAY_SURFACE_MULTIRES => Self::Multires,
            _ => Self::Unknown(code),
        }
    }

    /// Whether a chunk of this surface copies as its own welded vertex list.
    /// An adaptive surface copies unwelded triangles instead — read
    /// [`ChunkReadback::vertex_count`] rather than assuming either.
    pub fn is_welded(self) -> bool {
        matches!(self, Self::Fixed | Self::Multires)
    }
}

/// The four counters that say *what* changed about one chunk.
///
/// An array element and not a versioned descriptor: a caller passes one per
/// chunk per frame and reads thousands of them, so the layout is the contract
/// upstream and there is nothing here for a `struct_size` to negotiate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord, Hash)]
pub struct ChunkRevisions {
    /// Membership changed: the one an index buffer follows.
    pub topology: u64,
    /// The same faces, in different places.
    pub geometry: u64,
    /// Positions unchanged, shading normals rewritten — which is how a
    /// deferred normal flush is told from a move.
    pub normals: u64,
    /// Colour, mask, UV.
    pub attributes: u64,
}

impl ChunkRevisions {
    fn from_raw(raw: sys::clay_chunk_revisions) -> Self {
        Self {
            topology: raw.topology,
            geometry: raw.geometry,
            normals: raw.normals,
            attributes: raw.attributes,
        }
    }

    fn to_raw(self) -> sys::clay_chunk_revisions {
        sys::clay_chunk_revisions {
            topology: self.topology,
            geometry: self.geometry,
            normals: self.normals,
            attributes: self.attributes,
        }
    }
}

/// One chunk, as a host reads it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ChunkInfo {
    pub chunk: u32,
    /// The maximum of the four below, and the counter the shipped transport
    /// reports.
    pub revision: u64,
    pub revisions: ChunkRevisions,
    /// Float3 the chunk needs.
    pub vertex_count: u32,
    /// Uint32 the chunk needs; triangles.
    pub index_count: u32,
    pub geometry_dirty: bool,
    pub topology_dirty: bool,
    /// False where the id names a chunk that has since been released.
    ///
    /// A dirty set *may* name one: retiring a merged chunk from the list would
    /// be an O(dirty) erase on a path that runs during a stroke, to save a
    /// check that does not. Skip it; it is not an error.
    pub live: bool,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

impl ChunkInfo {
    fn from_raw(raw: sys::clay_chunk_info) -> Self {
        Self {
            chunk: raw.chunk,
            revision: raw.revision,
            revisions: ChunkRevisions::from_raw(raw.revisions),
            vertex_count: raw.vertex_count,
            index_count: raw.index_count,
            geometry_dirty: raw.geometry_dirty != 0,
            topology_dirty: raw.topology_dirty != 0,
            live: raw.live != 0,
            bounds_min: raw.bounds_min,
            bounds_max: raw.bounds_max,
        }
    }
}

/// What one copy did, and against what.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChunkReadback {
    pub chunk: u32,
    /// What the chunk needs, whether or not anything was written.
    pub vertex_count: u32,
    pub index_count: u32,
    /// What the caller said it had seen, echoed. All zero where it said
    /// nothing.
    pub requested: ChunkRevisions,
    /// What the engine is at now. Equal to `requested` on a fresh readback.
    pub current: ChunkRevisions,
    /// The engine moved on after the caller took its snapshot.
    ///
    /// The data written is *current* — this is not a failure — but a host
    /// applying an older frame's plan can tell that its plan is out of date.
    pub stale: bool,
}

impl ChunkReadback {
    fn from_raw(raw: sys::clay_chunk_readback) -> Self {
        Self {
            chunk: raw.chunk,
            vertex_count: raw.vertex_count,
            index_count: raw.index_count,
            requested: ChunkRevisions::from_raw(raw.requested),
            current: ChunkRevisions::from_raw(raw.current),
            stale: raw.stale != 0,
        }
    }
}

/// One chunk's bytes, into buffers this call owns, and what the copy was
/// against.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChunkCopy {
    pub readback: ChunkReadback,
    /// Three floats per vertex.
    pub positions: Vec<f32>,
    /// Three floats per vertex.
    pub normals: Vec<f32>,
    /// Triangles.
    pub indices: Vec<u32>,
}

impl ChunkCopy {
    /// The acknowledgement for what was just copied.
    ///
    /// The only way to build a [`ChunkAck`], and deliberately: it carries the
    /// `current` revision the copy *reported*, which is what makes a chunk
    /// that changed again in between stay dirty. An acknowledgement assembled
    /// from what a host wishes it had seen retires a change nobody uploaded.
    pub fn ack(&self) -> ChunkAck {
        ChunkAck {
            chunk: self.readback.chunk,
            seen: self.readback.current,
        }
    }
}

/// One chunk this caller has finished with, and the revision it finished with
/// it at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkAck {
    pub chunk: u32,
    pub seen: ChunkRevisions,
}

/// What a chunk aims to hold, for a view over a flat mesh.
///
/// The one case where the caller chooses, because a fixed mesh has no
/// partitioner of its own yet. `None` takes the library's defaults, which are
/// measured rather than guessed; a host with no opinion should pass `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkOptions {
    pub target_faces: u32,
    /// Below this two siblings merge.
    pub min_faces: u32,
    /// Above this a chunk splits; the gap is hysteresis.
    pub max_faces: u32,
}

impl ChunkOptions {
    /// The library's own, read from the engine rather than transcribed here.
    pub fn defaults() -> Result<Self> {
        let mut raw = sys::clay_chunk_options::sized();
        // SAFETY: a valid versioned descriptor out-parameter whose struct_size
        // is set above.
        check(
            unsafe { sys::clay_chunk_options_defaults(&mut raw) },
            "clay_chunk_options_defaults",
        )?;
        Ok(Self {
            target_faces: raw.target_faces,
            min_faces: raw.min_faces,
            max_faces: raw.max_faces,
        })
    }

    fn to_raw(self) -> sys::clay_chunk_options {
        let mut raw = sys::clay_chunk_options::sized();
        raw.target_faces = self.target_faces;
        raw.min_faces = self.min_faces;
        raw.max_faces = self.max_faces;
        raw
    }
}

// -- the view ---------------------------------------------------------------

/// A read seam over one surface, for as long as the surface is not being
/// changed.
///
/// Holds no geometry of its own. The lifetime is the borrow of the surface it
/// names, which is what stops it outliving the thing it reads and what forces
/// a host that is about to mutate to drop it first.
pub struct SurfaceView<'a> {
    raw: NonNull<sys::clay_surface_view>,
    surface: PhantomData<&'a mut ()>,
}

impl<'a> SurfaceView<'a> {
    /// A view over a flat mesh, partitioned on the spot.
    ///
    /// The chunk table is the *view's*, because the fixed sculptor keeps its
    /// own weld-class dirty list and whether that is retired in favour of a
    /// chunk dirty set is a measurement upstream has not made yet. So this
    /// view reports one partition of a static mesh and an empty dirty set; the
    /// hierarchy's carries a live one.
    pub fn over_mesh(mesh: &'a Mesh, options: Option<ChunkOptions>) -> Result<Self> {
        let raw_options = options.map(ChunkOptions::to_raw);
        let mut view = std::ptr::null_mut();
        // SAFETY: a valid mesh handle borrowed for the view's lifetime, an
        // optional versioned descriptor the call only reads, and an
        // out-parameter written only on success.
        check(
            unsafe {
                sys::clay_surface_view_from_mesh(
                    mesh.as_ptr(),
                    raw_options
                        .as_ref()
                        .map_or(std::ptr::null(), |o| o as *const _),
                    &mut view,
                )
            },
            "clay_surface_view_from_mesh",
        )?;
        Self::from_raw(view, "clay_surface_view_from_mesh")
    }

    /// A view over one level of a hierarchy.
    ///
    /// The surface is borrowed mutably because reading a level's chunks
    /// *evaluates* it, exactly as [`Multires::copy_block`] does — the level is
    /// built on demand and the handle is written to on the way.
    pub fn over_multires(surface: &'a mut Multires, level: u32) -> Result<Self> {
        let mut view = std::ptr::null_mut();
        // SAFETY: a valid hierarchy handle borrowed mutably for the view's
        // lifetime; the level is checked by the engine, which refuses one that
        // is gone rather than reading past the level list.
        check(
            unsafe { sys::clay_surface_view_from_multires(surface.as_ptr(), level, &mut view) },
            "clay_surface_view_from_multires",
        )?;
        Self::from_raw(view, "clay_surface_view_from_multires")
    }

    fn from_raw(raw: *mut sys::clay_surface_view, operation: &'static str) -> Result<Self> {
        NonNull::new(raw)
            .map(|raw| Self {
                raw,
                surface: PhantomData,
            })
            .ok_or_else(|| raw_failure(operation, ErrorKind::Backend))
    }

    /// Which representation is underneath.
    pub fn kind(&self) -> SurfaceKind {
        // SAFETY: an owned view handle; the call only reads a tag.
        SurfaceKind::from_raw(unsafe { sys::clay_surface_view_kind(self.raw.as_ptr()) })
    }

    /// How many chunk ids there are.
    ///
    /// Ids run from zero to this, and a slot in that range may be dead — see
    /// [`ChunkInfo::live`].
    ///
    /// Not `&self`: on a hierarchy, resolving the view evaluates the level.
    pub fn chunk_count(&mut self) -> usize {
        // SAFETY: an owned view handle. The entry point answers zero rather
        // than failing where the surface underneath has gone.
        unsafe { sys::clay_surface_view_chunk_count(self.raw.as_ptr()) }
    }

    /// One [`ChunkInfo`] per id, in the order asked.
    pub fn chunk_infos(&mut self, chunks: &[u32]) -> Result<Vec<ChunkInfo>> {
        self.fill_infos(Some(chunks), chunks.len())
    }

    /// The first `count` ids in order.
    ///
    /// What a host walking the whole surface wants, and it saves building an
    /// ascending array to say so.
    pub fn chunk_infos_in_order(&mut self, count: usize) -> Result<Vec<ChunkInfo>> {
        self.fill_infos(None, count)
    }

    fn fill_infos(&mut self, chunks: Option<&[u32]>, count: usize) -> Result<Vec<ChunkInfo>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut raw = vec![sys::clay_chunk_info::default(); count];
        // SAFETY: an owned view handle; `chunks` is either null or a slice of
        // exactly `count` ids, and `raw` has exactly `count` slots, which is
        // what the entry point fills. Neither pointer outlives this call.
        check(
            unsafe {
                sys::clay_surface_view_chunk_infos(
                    self.raw.as_ptr(),
                    chunks.map_or(std::ptr::null(), <[u32]>::as_ptr),
                    count,
                    raw.as_mut_ptr(),
                )
            },
            "clay_surface_view_chunk_infos",
        )?;
        Ok(raw.into_iter().map(ChunkInfo::from_raw).collect())
    }

    /// The chunks the stamps since the last drain touched.
    ///
    /// Sized from the count the engine writes and grown again if it moved
    /// between the two calls, so a short buffer is never something the caller
    /// has to recognise — which is the point, because the code for it is
    /// indistinguishable from the code for an invalid argument at the call
    /// site and means the opposite thing.
    pub fn dirty_chunks(&mut self) -> Result<Vec<u32>> {
        let view = self.raw.as_ptr();
        size_query_array("clay_surface_view_dirty_chunks", |buf, count| {
            // SAFETY: an owned view handle; `buf` is either null — the size
            // query — or a buffer of `*count` `uint32_t`, which is what the
            // entry point's contract asks for. `size_query_array` owns the
            // retry.
            unsafe { sys::clay_surface_view_dirty_chunks(view, buf, count) }
        })
    }

    /// What one chunk needs, without copying it.
    ///
    /// The capacity query on its own, for a host sizing a pool up front.
    pub fn chunk_capacity(&mut self, chunk: u32) -> Result<ChunkReadback> {
        let mut raw = sys::clay_chunk_readback::sized();
        // SAFETY: an owned view handle and a versioned out-descriptor. Every
        // buffer is null, which is the documented capacity query: it writes
        // nothing and reports what the chunk needs.
        check(
            unsafe {
                sys::clay_surface_view_copy_chunk(
                    self.raw.as_ptr(),
                    chunk,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    &mut raw,
                )
            },
            "clay_surface_view_copy_chunk",
        )?;
        Ok(ChunkReadback::from_raw(raw))
    }

    /// One chunk's positions, normals and indices, into buffers this call
    /// owns.
    ///
    /// `expected` is what the caller has already seen of this chunk; `None`
    /// means "I have not seen it before" and never reports stale. Pass what a
    /// previous [`ChunkCopy`] reported as
    /// [`current`](ChunkReadback::current), so that a chunk which moved since
    /// then says so through [`ChunkReadback::stale`].
    ///
    /// The capacity query and the fill are both taken here, and a buffer that
    /// grew between them is retried rather than reported: a truncation from
    /// this transport is `CLAY_ERROR_BUFFER_TOO_SMALL`, which is retryable,
    /// and a caller that saw it beside an invalid argument would have to tell
    /// two opposite meanings apart from one code.
    pub fn copy_chunk(
        &mut self,
        chunk: u32,
        expected: Option<ChunkRevisions>,
    ) -> Result<ChunkCopy> {
        let want = expected.map(ChunkRevisions::to_raw);
        let want_ptr = want.as_ref().map_or(std::ptr::null(), |r| r as *const _);

        // Two attempts, not a loop: the first sizes from the engine's own
        // counts, and only a change to the surface between the two calls can
        // move them. A second disagreement is not a race, it is a surface
        // being mutated under a read, and spinning on it would hide that.
        let mut sized = self.chunk_capacity(chunk)?;
        for _ in 0..2 {
            let mut positions = vec![0.0f32; sized.vertex_count as usize * 3];
            let mut normals = vec![0.0f32; sized.vertex_count as usize * 3];
            let mut indices = vec![0u32; sized.index_count as usize];
            let mut raw = sys::clay_chunk_readback::sized();

            // SAFETY: an owned view handle, a versioned out-descriptor, and
            // three buffers whose capacities are stated as exactly their
            // lengths. `expected` is either null or a single revision struct
            // the call only reads. Nothing here outlives the call.
            let code = unsafe {
                sys::clay_surface_view_copy_chunk(
                    self.raw.as_ptr(),
                    chunk,
                    want_ptr,
                    positions.as_mut_ptr(),
                    positions.len(),
                    normals.as_mut_ptr(),
                    normals.len(),
                    indices.as_mut_ptr(),
                    indices.len(),
                    &mut raw,
                )
            };
            match check(code, "clay_surface_view_copy_chunk") {
                Ok(()) => {
                    let readback = ChunkReadback::from_raw(raw);
                    positions.truncate(readback.vertex_count as usize * 3);
                    normals.truncate(readback.vertex_count as usize * 3);
                    indices.truncate(readback.index_count as usize);
                    return Ok(ChunkCopy {
                        readback,
                        positions,
                        normals,
                        indices,
                    });
                }
                // The one recoverable outcome, and the reason this loop is
                // here rather than at every call site: nothing was written,
                // and the descriptor says what to allocate. An invalid
                // argument means the call itself was wrong and is returned.
                Err(e) if e.kind() == ErrorKind::BufferTooSmall => {
                    sized = ChunkReadback::from_raw(raw);
                }
                Err(e) => return Err(e),
            }
        }
        Err(raw_failure(
            "clay_surface_view_copy_chunk",
            ErrorKind::BufferTooSmall,
        ))
    }

    /// Retires chunks from the dirty set, each one only if it has not changed
    /// since the caller copied it.
    ///
    /// Returns how many of the ids are clean afterwards, which includes ones
    /// that were never dirty. The rest changed again between the copy and this
    /// call and are still waiting — which is what makes draining across frames
    /// lossless at any rate.
    pub fn acknowledge(&mut self, acks: &[ChunkAck]) -> Result<usize> {
        if acks.is_empty() {
            return Ok(0);
        }
        // Two parallel arrays because that is the entry point's shape; they
        // are built here rather than asked of the caller so that a mismatched
        // pair — a chunk acknowledged at another chunk's revision — is not
        // something a caller can express.
        let chunks: Vec<u32> = acks.iter().map(|a| a.chunk).collect();
        let seen: Vec<sys::clay_chunk_revisions> = acks.iter().map(|a| a.seen.to_raw()).collect();
        let mut clean = 0usize;
        // SAFETY: an owned view handle and two arrays of exactly `acks.len()`
        // elements each, which is the count passed; the engine only reads
        // them, and `clean` is a valid out-parameter.
        check(
            unsafe {
                sys::clay_surface_view_acknowledge(
                    self.raw.as_ptr(),
                    chunks.as_ptr(),
                    seen.as_ptr(),
                    acks.len(),
                    &mut clean,
                )
            },
            "clay_surface_view_acknowledge",
        )?;
        Ok(clean)
    }

    /// Drops the whole dirty set.
    ///
    /// The all-or-nothing form, for a host that uploads everything it was told
    /// about in one frame and has nothing to reconcile. Prefer
    /// [`acknowledge`](Self::acknowledge) if you drain incrementally: this one
    /// retires a chunk that changed after it was copied, and nothing
    /// afterwards says the change was lost.
    pub fn clear_dirty(&mut self) -> Result<()> {
        // SAFETY: an owned view handle.
        check(
            unsafe { sys::clay_surface_view_clear_dirty(self.raw.as_ptr()) },
            "clay_surface_view_clear_dirty",
        )
    }
}

impl Drop for SurfaceView<'_> {
    fn drop(&mut self) {
        // SAFETY: an owned handle, released exactly once. It owns nothing of
        // the surface it names, so this frees the view and no geometry.
        unsafe { sys::clay_surface_view_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for SurfaceView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceView")
            .field("kind", &self.kind())
            .finish()
    }
}
