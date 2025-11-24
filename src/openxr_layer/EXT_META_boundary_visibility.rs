// https://github.com/meta-quest/Meta-OpenXR-SDK/blob/main/OpenXR/meta_openxr_preview/meta_boundary_visibility.h

use std::ffi::c_void;

use openxr::StructureType;
use openxr_sys::Bool32;

// #define XR_META_boundary_visibility_SPEC_VERSION 1
// #define XR_META_BOUNDARY_VISIBILITY_EXTENSION_NAME "XR_META_boundary_visibility"
#[expect(non_upper_case_globals)]
pub const META_boundary_visibility_SPEC_VERSION: u32 = 1u32;
pub const META_BOUNDARY_VISIBILITY_EXTENSION_NAME: &[u8] = b"XR_META_boundary_visibility\0";

// typedef enum XrBoundaryVisibilityMETA {
//     // Boundary is not suppressed.
//     XR_BOUNDARY_VISIBILITY_NOT_SUPPRESSED_META = 1,
//     // Boundary is suppressed.
//     XR_BOUNDARY_VISIBILITY_SUPPRESSED_META = 2,
//     XR_BOUNDARY_VISIBILITY_MAX_ENUM_META = 0x7FFFFFFF
// } XrBoundaryVisibilityMETA;
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct BoundaryVisibilityMETA(i32);
impl BoundaryVisibilityMETA {
    pub const BOUNDARY_VISIBILITY_NOT_SUPPRESSED: BoundaryVisibilityMETA = Self(1i32);
    pub const BOUNDARY_VISIBILITY_SUPPRESSED: BoundaryVisibilityMETA = Self(2i32);
}

// typedef struct XrSystemBoundaryVisibilityPropertiesMETA {
//     XrStructureType             type;
//     const void* XR_MAY_ALIAS    next;
//     XrBool32                    supportsBoundaryVisibility;
// } XrSystemBoundaryVisibilityPropertiesMETA;
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SystemBoundaryVisibilityPropertiesMETA {
    pub ty: StructureType,
    pub next: *mut c_void,
    pub supports_boundary_visibility: Bool32,
}
// static const XrStructureType XR_TYPE_SYSTEM_BOUNDARY_VISIBILITY_PROPERTIES_META = (XrStructureType) 1000528000;
pub const TYPE_SYSTEM_BOUNDARY_VISIBILITY_PROPERTIES_META: i32 = 1000528000;
impl SystemBoundaryVisibilityPropertiesMETA {
    pub fn get_type() -> StructureType {
        StructureType::from_raw(TYPE_SYSTEM_BOUNDARY_VISIBILITY_PROPERTIES_META)
    }
}

// typedef struct XrEventDataBoundaryVisibilityChangedMETA {
//     XrStructureType             type;
//     const void* XR_MAY_ALIAS    next;
//     XrBoundaryVisibilityMETA    boundaryVisibility;
// } XrEventDataBoundaryVisibilityChangedMETA;
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct EventDataBoundaryVisibilityChangedMETA {
    pub ty: StructureType,
    pub next: *const c_void,
    pub boundary_visibility: BoundaryVisibilityMETA,
}
// static const XrStructureType XR_TYPE_EVENT_DATA_BOUNDARY_VISIBILITY_CHANGED_META = (XrStructureType) 1000528001;
impl EventDataBoundaryVisibilityChangedMETA {
    pub fn get_type() -> StructureType {
        StructureType::from_raw(1000528001i32)
    }
}

// typedef XrResult (XRAPI_PTR *PFN_xrRequestBoundaryVisibilityMETA)(XrSession session, XrBoundaryVisibilityMETA boundaryVisibility);
// #ifndef XR_NO_PROTOTYPES
// #ifdef XR_EXTENSION_PROTOTYPES
// XRAPI_ATTR XrResult XRAPI_CALL xrRequestBoundaryVisibilityMETA(
//     XrSession                                   session,
//     XrBoundaryVisibilityMETA                    boundaryVisibility);
pub mod pfn {
    use std::ffi::c_char;

    use openxr_sys::{Result, Session, pfn};

    use crate::openxr_layer::EXT_META_boundary_visibility::BoundaryVisibilityMETA;

    pub type RequestBoundaryVisibilityMETA = unsafe extern "system" fn(
        session: Session,
        boundary_vsibility: BoundaryVisibilityMETA,
    ) -> Result;
}

// static const XrResult XR_BOUNDARY_VISIBILITY_SUPPRESSION_NOT_ALLOWED_META = (XrResult) 1000528000;
// TODO
