use std::{collections::HashMap, ffi::c_void, ptr, time::SystemTime};

use once_cell::sync::Lazy;
use openxr_sys::{
    self as xr_sys, BaseInStructure, CompositionLayerBaseHeader, CompositionLayerFlags,
    CompositionLayerQuad, Extent2Df, Extent2Di, EyeVisibility, FrameEndInfo,
    GraphicsBindingOpenGLESAndroidKHR, LoaderInitInfoBaseHeaderKHR, Offset2Di, Posef, Quaternionf,
    Rect2Di, SessionCreateInfo, SwapchainCreateFlags, SwapchainCreateInfo, SwapchainSubImage,
    SwapchainUsageFlags, Vector3f, pfn,
};

use openxr::{self as xr};

#[cfg(feature = "android")]
use openxr_sys::LoaderInitInfoAndroidKHR;

// use crate::server::OSCServer;

pub static mut LAYER: Lazy<OpenXRLayer> = Lazy::new(OpenXRLayer::new);

struct Extension {
    name: &'static str,
    version: u32,
}

const ADVERTISED_EXTENSIONS: &[Extension] = &[Extension {
    name: "XR_EXT_eye_gaze_interaction",
    version: 1,
}];

pub struct OpenXRLayer {
    pub instance: Option<xr::Instance>,
    pub session: Option<xr::Session<xr::OpenGlEs>>,

    pub get_instance_proc_addr: Option<pfn::GetInstanceProcAddr>,
    pub enumerate_instance_extensions_properties: Option<pfn::EnumerateInstanceExtensionProperties>,
    pub get_system_properties: Option<pfn::GetSystemProperties>,
    pub suggest_interaction_profile_bindings: Option<pfn::SuggestInteractionProfileBindings>,
    pub path_to_string: Option<pfn::PathToString>,
    pub create_action_space: Option<pfn::CreateActionSpace>,
    pub get_action_state_pose: Option<pfn::GetActionStatePose>,
    pub locate_space: Option<pfn::LocateSpace>,
    pub locate_views: Option<pfn::LocateViews>,
    pub end_frame: Option<pfn::EndFrame>,

    pub create_session: Option<pfn::CreateSession>,
    pub initalize_loader_khr: Option<pfn::InitializeLoaderKHR>,
    pub create_swapchain: Option<pfn::CreateSwapchain>,
    pub create_reference_space: Option<pfn::CreateReferenceSpace>,

    pub application_vm: *mut c_void,
    pub application_context: *mut c_void,

    pub gles_display: *mut c_void,
    pub gles_config: *mut c_void,
    pub gles_context: *mut c_void,

    debug_window_swapchain: Option<xr::Swapchain<xr::OpenGlEs>>,
    view_reference_space: Option<xr::Space>,

    possible_spaces: HashMap<(xr::Action<xr::Posef>, xr::Path), xr::Space>,

    eye_gaze_action: Option<xr::Action<xr::Posef>>,
    l_eye_gaze_space: Option<xr::Space>,
    r_eye_gaze_space: Option<xr::Space>,

    start_time: SystemTime,
    // server: OSCServer,
}

impl OpenXRLayer {
    pub fn new() -> OpenXRLayer {
        OpenXRLayer {
            instance: None,
            session: None,

            get_instance_proc_addr: None,
            enumerate_instance_extensions_properties: None,
            get_system_properties: None,
            suggest_interaction_profile_bindings: None,
            path_to_string: None,
            eye_gaze_action: None,
            create_action_space: None,
            l_eye_gaze_space: None,
            r_eye_gaze_space: None,
            get_action_state_pose: None,
            locate_space: None,
            locate_views: None,
            create_session: None,
            initalize_loader_khr: None,
            create_swapchain: None,
            end_frame: None,
            create_reference_space: None,
            possible_spaces: HashMap::new(),
            start_time: SystemTime::now(),
            // server: OSCServer::new(),
            application_context: ptr::null_mut(),
            application_vm: ptr::null_mut(),

            gles_display: ptr::null_mut(),
            gles_config: ptr::null_mut(),
            gles_context: ptr::null_mut(),

            debug_window_swapchain: None,
            view_reference_space: None,
        }
    }

    fn on_session_created(&mut self) {
        let session = self.session.as_ref().unwrap();

        self.debug_window_swapchain = Some(
            session
                .create_swapchain(&xr::SwapchainCreateInfo {
                    create_flags: SwapchainCreateFlags::EMPTY,
                    // TODO: Set the proper flags, those are copied from Steam Link.
                    usage_flags: SwapchainUsageFlags::COLOR_ATTACHMENT
                        | SwapchainUsageFlags::SAMPLED,
                    format: 35907, // OpenGL ES 3 GL_SRGB8_ALPHA8
                    sample_count: 1,
                    width: 1344,
                    height: 1344,
                    face_count: 1,
                    array_size: 1,
                    mip_count: 1,
                })
                .expect("failed to create swapchain"),
        );

        self.view_reference_space = Some(
            session
                .create_reference_space(xr::ReferenceSpaceType::VIEW, xr::Posef::IDENTITY)
                .expect("failed to create view reference space"),
        );
    }

    pub unsafe fn create_session(
        &mut self,
        instance: xr_sys::Instance,
        create_info: *const SessionCreateInfo,
        session: *mut xr_sys::Session,
    ) -> openxr_sys::Result {
        unsafe {
            let create_info = &*create_info;
            let next = &*(create_info.next as *const BaseInStructure);

            println!("create_info {create_info:?}");
            println!("create_info.next {next:?}");

            assert_eq!(
                next.ty,
                openxr_sys::StructureType::GRAPHICS_BINDING_OPENGL_ES_ANDROID_KHR
            );

            let graphics_binding_open_glesandroid_khr =
                &*(create_info.next as *mut GraphicsBindingOpenGLESAndroidKHR);

            println!(
                "graphics_binding_open_glesandroid_khr {graphics_binding_open_glesandroid_khr:?}"
            );

            let result = self.create_session.unwrap()(instance, create_info, session);

            if result != openxr_sys::Result::SUCCESS {
                return result;
            }

            // instance.create_session(system, info)

            let (session, _, _) = xr::Session::<xr::OpenGlEs>::from_raw(
                self.instance.as_ref().unwrap().clone(),
                *session,
                Box::new(()),
            );
            self.session = Some(session);

            self.on_session_created();

            openxr_sys::Result::SUCCESS
        }
    }

    pub unsafe fn initalize_loader_khr(
        &mut self,
        loader_init_info: *const LoaderInitInfoBaseHeaderKHR,
    ) -> openxr_sys::Result {
        unsafe {
            // OpenXR is only for Android so far.
            assert_eq!(
                (*loader_init_info).ty,
                openxr_sys::StructureType::LOADER_INIT_INFO_ANDROID_KHR
            );

            let loader_init_info_android = &*(loader_init_info as *const LoaderInitInfoAndroidKHR);

            println!("loader_init_info_android: {loader_init_info_android:?}");

            self.application_context = loader_init_info_android.application_context;
            self.application_vm = loader_init_info_android.application_vm;

            // From the OpenXR specs:
            // If the xrInitializeLoaderKHR function is discovered through the manifest,
            // xrInitializeLoaderKHR will be called before xrNegotiateLoaderRuntimeInterface or
            // xrNegotiateLoaderApiLayerInterface has been called on the runtime or layer respectively.
            //
            // Means we cannot call the next xrInitializeLoaderKHR function here to return the result.
            openxr_sys::Result::SUCCESS
        }
    }

    pub unsafe fn create_swapchain(
        &self,
        session: xr_sys::Session,
        create_info: *const SwapchainCreateInfo,
        swapchain: *mut xr_sys::Swapchain,
    ) -> openxr_sys::Result {
        unsafe {
            println!("create_info: {:?}", *create_info);

            let result = self.create_swapchain.unwrap()(session, create_info, swapchain);

            println!("swapchain: {:?}", *swapchain);

            result
        }
    }

    pub unsafe fn end_frame(
        &self,
        session: xr_sys::Session,
        frame_end_info: *const FrameEndInfo,
    ) -> openxr_sys::Result {
        // Create our copy of FrameEndInfo.
        let mut frame_end_info = unsafe { *frame_end_info };

        let original_layer_count = frame_end_info.layer_count as usize;
        let original_layers =
            unsafe { std::slice::from_raw_parts(frame_end_info.layers, original_layer_count) };

        // TODO: Don't reallocate every frame.
        let mut layers = Vec::<*const CompositionLayerBaseHeader>::new();
        layers.reserve_exact(original_layer_count + 1);
        layers.extend_from_slice(original_layers);

        let my_layer = CompositionLayerQuad {
            ty: openxr_sys::StructureType::COMPOSITION_LAYER_QUAD,
            next: ptr::null(),
            eye_visibility: EyeVisibility::BOTH,
            layer_flags: CompositionLayerFlags::EMPTY,
            space: self.view_reference_space.as_ref().unwrap().as_raw(),
            pose: Posef {
                position: Vector3f {
                    x: 0.0,
                    y: 0.0,
                    z: -1.0,
                },
                orientation: Quaternionf {
                    x: -0.002,
                    y: -0.670,
                    z: 0.223,
                    w: 0.708,
                },
            },
            size: Extent2Df {
                width: 0.1,
                height: 0.1,
            },
            sub_image: SwapchainSubImage {
                swapchain: self.debug_window_swapchain.as_ref().unwrap().as_raw(),
                image_rect: Rect2Di {
                    extent: Extent2Di {
                        width: 1280,
                        height: 720,
                    },
                    offset: Offset2Di { x: 0, y: 0 },
                },
                image_array_index: 0,
            },
        };

        // TODO: Is there a better way to do this cast?
        layers
            .push((&my_layer as *const CompositionLayerQuad) as *const CompositionLayerBaseHeader);

        frame_end_info.layer_count = layers.len() as u32;
        frame_end_info.layers = layers.as_ptr();

        assert_eq!(
            unsafe { self.end_frame.unwrap()(session, &frame_end_info) },
            openxr_sys::Result::SUCCESS
        );

        openxr_sys::Result::SUCCESS
    }

    /*
    pub unsafe fn enumerate_instance_extension_properties(
        &self,
        layer_name: *const c_char,
        property_capacity_input: u32,
        property_count_output: *mut u32,
        properties_ptr: *mut ExtensionProperties,
    ) -> Result {
        let mut result = self.enumerate_instance_extensions_properties.unwrap()(
            layer_name,
            property_capacity_input,
            property_count_output,
            properties_ptr,
        );

        let base_offset = *property_count_output as usize;
        *property_count_output += ADVERTISED_EXTENSIONS.len() as u32;
        if property_capacity_input > 0 {
            if property_capacity_input < *property_count_output {
                result = Result::ERROR_SIZE_INSUFFICIENT;
            } else {
                result = Result::SUCCESS;

                let properties = std::slice::from_raw_parts_mut(
                    properties_ptr,
                    (*property_count_output).try_into().unwrap(),
                );

                for i in base_offset..*property_count_output as usize {
                    if properties[i].ty != StructureType::EXTENSION_PROPERTIES {
                        result = Result::ERROR_VALIDATION_FAILURE;
                        break;
                    }

                    let extension = &ADVERTISED_EXTENSIONS[i - base_offset];

                    std::ptr::copy(
                        extension.name.as_ptr(),
                        properties[i].extension_name.as_mut_ptr() as *mut u8,
                        extension.name.len(),
                    );
                    properties[i].extension_version = extension.version;
                }
            }
        }

        result
    }

    pub unsafe fn get_system_properties(
        &self,
        instance: Instance,
        system_id: SystemId,
        properties: *mut SystemProperties,
    ) -> Result {
        println!("--> get_system_properties");

        let mut property_ptr = properties as *mut BaseOutStructure;
        while !property_ptr.is_null() {
            let property = &mut *property_ptr;

            println!("get_system_properties type {:?}", property.ty);

            if property.ty == StructureType::SYSTEM_EYE_GAZE_INTERACTION_PROPERTIES_EXT {
                let property = &mut *(property_ptr as *mut SystemEyeGazeInteractionPropertiesEXT);
                property.supports_eye_gaze_interaction = true.into();
            }

            property_ptr = property.next;
        }

        let result = self.get_system_properties.unwrap()(instance, system_id, properties);
        if result != Result::SUCCESS {
            println!("get_system_properties result: {result:?}");
            return result;
        }

        println!("<-- get_system_properties");
        Result::SUCCESS
    }

    pub unsafe fn suggest_interaction_profile_bindings(
        &mut self,
        instance: Instance,
        suggested_bindings: *const InteractionProfileSuggestedBinding,
    ) -> Result {
        let suggested_bindings = &*suggested_bindings;

        let interaction_profile = self.path_to_string(suggested_bindings.interaction_profile);

        println!(
            "suggest_interaction_profile_bindings {:?} {}",
            suggested_bindings, interaction_profile
        );

        if interaction_profile != "/interaction_profiles/ext/eye_gaze_interaction" {
            return self.suggest_interaction_profile_bindings.unwrap()(
                instance,
                suggested_bindings,
            );
        }

        let suggested_bindings = std::slice::from_raw_parts(
            suggested_bindings.suggested_bindings,
            suggested_bindings
                .count_suggested_bindings
                .try_into()
                .unwrap(),
        );

        for suggested_binding in suggested_bindings {
            let binding = self.path_to_string(suggested_binding.binding);
            println!("suggest_interaction_profile_bindings binding path {binding}");
            if binding == "/user/eyes_ext/input/gaze_ext/pose" {
                self.eye_gaze_action = Some(suggested_binding.action);
                println!(
                    "suggest_interaction_profile_bindings saved eye gaze action {:?}",
                    suggested_binding.action
                );

                if let Some(l_eye_gaze_space) = self
                    .possible_spaces
                    // TODO: Don't hardcode "/user/hand/left" as Path(1)
                    .get(&(suggested_binding.action, Path::from_raw(1)))
                {
                    self.l_eye_gaze_space = Some(*l_eye_gaze_space);
                    println!("L eye gaze space found: {:?}", l_eye_gaze_space);
                }
                if let Some(r_eye_gaze_space) = self
                    .possible_spaces
                    // TODO: Don't hardcode "/user/hand/right" as Path(2)
                    .get(&(suggested_binding.action, Path::from_raw(2)))
                {
                    self.r_eye_gaze_space = Some(*r_eye_gaze_space);
                    println!("R eye gaze space found: {:?}", r_eye_gaze_space);
                }

                self.possible_spaces.clear();

                println!(
                    "test {:?} {:?}",
                    self.path_to_string(Path::from_raw(1)), // "/user/hand/left"
                    self.path_to_string(Path::from_raw(2)), // "/user/hand/right"
                );
            }
        }

        Result::SUCCESS
    }

    pub unsafe fn create_action_space(
        &mut self,
        session: Session,
        create_info: *const ActionSpaceCreateInfo,
        space: *mut Space,
    ) -> Result {
        println!("--> create_action_space {:?}", *create_info);
        let result = self.create_action_space.unwrap()(session, create_info, space);
        if result != Result::SUCCESS {
            return result;
        }

        // Spaced are created before actions, so save them all and choose later when the action is known.
        let create_info = &*create_info;
        self.possible_spaces
            .insert((create_info.action, create_info.subaction_path), *space);

        println!("<-- create_action_space");
        Result::SUCCESS
    }

    pub unsafe fn get_action_state_pose(
        &self,
        session: Session,
        get_info: *const ActionStateGetInfo,
        state: *mut ActionStatePose,
    ) -> Result {
        if !self
            .eye_gaze_action
            .is_some_and(|a| a == (*get_info).action)
        {
            return self.get_action_state_pose.unwrap()(session, get_info, state);
        }

        // println!("--> get_action_state_pose {:?}", (*get_info).subaction_path);

        let eye_gaze_data = self.server.eye_gaze_data.lock().unwrap();
        let state = &mut *state;

        // Report tracking as disabled if there is no data incoming.
        state.is_active =
            (eye_gaze_data.time.elapsed().unwrap() < Duration::from_millis(50)).into();

        // println!("<-- get_action_state_pose");
        Result::SUCCESS
    }

    pub unsafe fn locate_space(
        &self,
        space: Space,
        base_space: Space,
        time: Time,
        location: *mut SpaceLocation,
    ) -> Result {
        // println!("--> locate_space {:?} {:?} {:?}", space, base_space, time);

        let is_left = self.l_eye_gaze_space.is_some_and(|s| s == space);
        let is_right = self.r_eye_gaze_space.is_some_and(|s| s == space);

        if !is_left && !is_right {
            return self.locate_space.unwrap()(space, base_space, time, location);
        }

        // println!("locate_space {:?} {:?}", space, base_space);

        let location = &mut *location;

        location.location_flags |= SpaceLocationFlags::POSITION_TRACKED;
        location.location_flags |= SpaceLocationFlags::ORIENTATION_TRACKED;

        let eye_gaze_data = self.server.eye_gaze_data.lock().unwrap();

        let (pitch, yaw) = if is_left {
            (eye_gaze_data.l_pitch, eye_gaze_data.l_yaw)
        } else {
            (eye_gaze_data.r_pitch, eye_gaze_data.r_yaw)
        };

        use quaternion_core as quat;
        let q = quat::from_euler_angles(
            quat::RotationType::Extrinsic,
            quat::RotationSequence::XYZ,
            [pitch, yaw, 0.0],
        );

        // TODO: Figure out if this is correct position.
        // If eyeball position is required, can use `xrLocateView` to query camera position.
        location.pose.position = Vector3f {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        location.pose.orientation = Quaternionf {
            w: q.0,
            x: q.1[0],
            y: q.1[1],
            z: q.1[2],
        };

        // println!("locate_space {:?}", location);

        if !location.next.is_null() {
            let eye_gaze_sample_time = &mut *(location.next as *mut EyeGazeSampleTimeEXT);
            eye_gaze_sample_time.time = Time::from_nanos(0);
            // println!("locate_space {:?}", eye_gaze_sample_time);
        }

        Result::SUCCESS
    }

    pub unsafe fn locate_views(
        &self,
        session: Session,
        view_locate_info: *const ViewLocateInfo,
        view_state: *mut ViewState,
        view_capacity_input: u32,
        view_count_output: *mut u32,
        views: *mut View,
    ) -> Result {
        let res = self.locate_views.unwrap()(
            session,
            view_locate_info,
            view_state,
            view_capacity_input,
            view_count_output,
            views,
        );

        if res != Result::SUCCESS {
            return res;
        }

        if (*view_locate_info).view_configuration_type != ViewConfigurationType::PRIMARY_STEREO {
            return Result::SUCCESS;
        }

        let views = std::slice::from_raw_parts_mut(views, (*view_count_output).try_into().unwrap());

        let apply_pupil_offset = |view: &mut View, is_left: bool| {
            use quat::QuaternionOps;
            use quaternion_core as quat;

            const EYEBALL_RADIUS: f32 = 12.0 * 0.1;

            let pos = view.pose.position;
            let mut pos = [pos.x, pos.y, pos.z];

            let quat = view.pose.orientation;
            let fwd_q = (quat.w, [quat.x, quat.y, quat.z]);

            let mut fwd_v = quat::to_rotation_vector(fwd_q);
            fwd_v = quat::normalize(fwd_v);

            pos = pos.sub(fwd_v.scale(EYEBALL_RADIUS));

            let eye_gaze_data = self.server.eye_gaze_data.lock().unwrap();

            let (pitch, yaw) = if is_left {
                (eye_gaze_data.l_pitch, eye_gaze_data.l_yaw)
            } else {
                (eye_gaze_data.r_pitch, eye_gaze_data.r_yaw)
            };

            let gaze_q = quat::from_euler_angles(
                quat::RotationType::Extrinsic,
                quat::RotationSequence::XYZ,
                [pitch, yaw, 0.0],
            );

            let gaze_fwd_q = quat::mul(fwd_q, gaze_q);
            let gaze_fwd_v = quat::normalize(quat::to_rotation_vector(gaze_fwd_q));

            pos = pos.add(gaze_fwd_v.scale(EYEBALL_RADIUS));

            view.pose.position = Vector3f {
                x: pos[0],
                y: pos[1],
                z: pos[2],
            }
        };

        apply_pupil_offset(&mut views[0], true);
        apply_pupil_offset(&mut views[1], false);

        Result::SUCCESS
    }
    */
}
