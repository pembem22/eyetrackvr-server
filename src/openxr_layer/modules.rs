#[derive(Default)]
pub struct OpenXRModules {
    pub local_dimming: LocalDimmingModule,
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
