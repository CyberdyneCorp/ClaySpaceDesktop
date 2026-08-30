//! Versioned descriptor structs.
//!
//! The engine versions every descriptor by a leading `struct_size` the caller
//! sets and the library reads only up to. Getting that field wrong is the one
//! mistake the ABI cannot catch for us, so no call site writes it by hand: the
//! trait below fills it from `size_of` of the type actually compiled against.

use claycore_sys as sys;

/// A descriptor whose first field is `struct_size`.
///
/// # Safety
///
/// The implementing type must be a `#[repr(C)]` descriptor generated from the
/// engine header whose first field is a `uint32_t struct_size`, so that
/// writing `size_of::<Self>()` there is what the engine expects to read.
pub(crate) unsafe trait Descriptor: Sized + Default {
    /// Writes `struct_size` from the size of the type as compiled here.
    fn sized() -> Self {
        let mut value = Self::default();
        // SAFETY: by the trait's contract the first field is a `uint32_t`, so
        // writing the size through a pointer to the struct's start writes that
        // field and nothing else.
        unsafe {
            *(&mut value as *mut Self as *mut u32) = std::mem::size_of::<Self>() as u32;
        }
        value
    }
}

macro_rules! descriptors {
    ($($ty:ty),+ $(,)?) => {
        $(
            // SAFETY: each of these is bindgen output for a header struct
            // documented as beginning with `uint32_t struct_size`.
            unsafe impl Descriptor for $ty {}
        )+
    };
}

descriptors! {
    sys::clay_mesh_params,
    sys::clay_vertex_layout,
    sys::clay_brick_mesh_params,
    sys::clay_brick_config,
    sys::clay_brick_stats,
    sys::clay_import_budget,
    sys::clay_layer_info,
    sys::clay_mesh_layer_desc,
    sys::clay_move_params,
    sys::clay_relax_params,
    sys::clay_flatten_params,
    sys::clay_volume_params,
    sys::clay_mesh_brush_desc,
    sys::clay_mesh_hit,
    sys::clay_mesh_deform_desc,
    sys::clay_field_report,
    sys::clay_consolidation_params,
    sys::clay_consolidation_cost,
    sys::clay_sculpt_policy,
    sys::clay_sculpt_dirty,
    sys::clay_sculpt_budget,
    sys::clay_sdf_preview_delta_info,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_size_is_written_from_the_compiled_type() {
        let params = sys::clay_mesh_params::sized();
        assert_eq!(
            params.struct_size as usize,
            std::mem::size_of::<sys::clay_mesh_params>()
        );

        let layout = sys::clay_vertex_layout::sized();
        assert_eq!(
            layout.struct_size as usize,
            std::mem::size_of::<sys::clay_vertex_layout>()
        );
    }

    #[test]
    fn every_other_field_is_left_at_its_default() {
        let params = sys::clay_mesh_params::sized();
        assert_eq!(params.voxel_size, 0.0);
        assert_eq!(params.resolution, 0);
        assert_eq!(params.decimate, 0);
    }
}
