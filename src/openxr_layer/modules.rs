#[derive(Default)]
pub struct OpenXRModules {
    pub local_dimming: LocalDimmingModule,
    pub boundary_visibility: BoundaryVisibilityModule,
    pub passthrough: PassthroughModule,
}

#[derive(Default)]
pub enum LocalDimmingMode {
    #[default]
    DONT_MODIFY,
    OVERRIDE_ON,
    OVERRIDE_OFF,
}

#[derive(Default)]
pub struct LocalDimmingModule {
    pub mode: LocalDimmingMode,
}

#[derive(Default, Debug)]
pub enum BoundaryVisibilityStatus {
    #[default]
    STATUS_UNKNOWN,
    TO_REQUEST_VISIBILITY_SUPPRESSED,
    REQUESTED_VISIBILITY_SUPPRESSED,
    CONFIRMED_VISIBILITY_SUPPRESSED,
    TO_REQUEST_VISIBILITY_NOT_SUPPRESSED,
    REQUESTED_VISIBILITY_NOT_SUPPRESSED,
    CONFIRMED_VISIBILITY_NOT_SUPPRESSED,
}

#[derive(Default)]
pub struct BoundaryVisibilityModule {
    pub supported_by_runtime: bool,
    pub status: BoundaryVisibilityStatus,
}

// #[derive(Default, Debug)]
// pub enum PassthroughStatus {
//     #[default]
//     STATUS_UNKNOWN,
//     TO_REQUEST_VISIBILITY_SUPPRESSED,
//     REQUESTED_VISIBILITY_SUPPRESSED,
//     CONFIRMED_VISIBILITY_SUPPRESSED,
//     TO_REQUEST_VISIBILITY_NOT_SUPPRESSED,
//     REQUESTED_VISIBILITY_NOT_SUPPRESSED,
//     CONFIRMED_VISIBILITY_NOT_SUPPRESSED,
// }

#[derive(Default)]
pub struct PassthroughModule {
    pub supported_by_runtime: bool,
}