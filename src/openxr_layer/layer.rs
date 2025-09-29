use std::{
    any::Any,
    collections::HashMap,
    ffi::{c_char, c_void},
    ptr,
    sync::{Condvar, Mutex},
};

use log::{debug, info, trace};
use once_cell::sync::Lazy;
use openxr::{self as xr, FaceConfidence2FB, FaceExpression2FB};
use openxr_sys::{
    self as xr_sys, BaseInStructure, CompositionLayerBaseHeader, CompositionLayerFlags,
    CompositionLayerQuad, Extent2Df, Extent2Di, EyeVisibility, FrameBeginInfo, FrameEndInfo,
    GraphicsBindingOpenGLESAndroidKHR, GraphicsBindingVulkanKHR, LoaderInitInfoBaseHeaderKHR,
    Offset2Di, Posef, Quaternionf, Rect2Di, SessionCreateInfo, SwapchainCreateFlags,
    SwapchainCreateInfo, SwapchainSubImage, SwapchainUsageFlags, Vector3f, pfn,
};
use quaternion_core as quat;

#[cfg(feature = "android")]
use openxr_sys::LoaderInitInfoAndroidKHR;

use crate::openxr_output::OPENXR_OUTPUT_BRIDGE;

pub static mut LAYER: Lazy<OpenXRLayer> = Lazy::new(OpenXRLayer::new);

pub struct Extension {
    pub name: &'static str,
    pub version: u32,
}

pub const ADVERTISED_EXTENSIONS: &[Extension] = &[
    Extension {
        name: "XR_EXT_eye_gaze_interaction",
        version: 1,
    },
    Extension {
        name: "XR_FB_face_tracking2",
        version: 1,
    },
    Extension {
        name: "XR_FB_eye_tracking_social",
        version: 1,
    },
];

#[cfg(feature = "gui")]
#[derive(Default, Debug)]
pub struct RenderSignal {
    pub ready: bool,
    pub mutex: Mutex<bool>,
    pub condvar: Condvar,
}

#[cfg(feature = "gui")]
#[derive(Default, Debug)]
pub struct EGLPointers {
    pub display: *mut c_void, // EGLDisplay
    pub config: *mut c_void,  // EGLConfig
    pub context: *mut c_void, // EGLContext
}

#[cfg(feature = "gui")]
#[link(name = "EGL")]
unsafe extern "C" {
    unsafe fn eglMakeCurrent(
        display: *mut c_void,
        draw: *mut c_void,
        read: *mut c_void,
        context: *mut c_void,
    ) -> u8;
}

/// Pitch yaw in degrees!
fn quat_from_pitch_yaw(pitch: f32, yaw: f32) -> quat::Quaternion<f32> {
    quat::from_euler_angles(
        quat::RotationType::Extrinsic,
        quat::RotationSequence::XYZ,
        [-pitch.to_radians(), -yaw.to_radians(), 0.0],
    )
}

pub struct OpenXRLayer {
    pub instance: Option<xr::Instance>,
    pub session: Option<xr::Session<xr::OpenGlEs>>,

    pub get_instance_proc_addr: Option<pfn::GetInstanceProcAddr>,
    pub enumerate_instance_extensions_properties: Option<pfn::EnumerateInstanceExtensionProperties>,
    pub get_system_properties: Option<pfn::GetSystemProperties>,
    pub suggest_interaction_profile_bindings: Option<pfn::SuggestInteractionProfileBindings>,
    pub create_action_space: Option<pfn::CreateActionSpace>,
    pub get_action_state_pose: Option<pfn::GetActionStatePose>,
    pub locate_space: Option<pfn::LocateSpace>,
    pub locate_views: Option<pfn::LocateViews>,
    pub end_frame: Option<pfn::EndFrame>,
    pub begin_frame: Option<pfn::BeginFrame>,

    pub create_session: Option<pfn::CreateSession>,
    pub initalize_loader_khr: Option<pfn::InitializeLoaderKHR>,
    pub create_swapchain: Option<pfn::CreateSwapchain>,
    pub create_reference_space: Option<pfn::CreateReferenceSpace>,

    #[cfg(feature = "gui")]
    pub egl_pointers: Option<EGLPointers>,
    #[cfg(feature = "gui")]
    pub egl_image: u32,
    #[cfg(feature = "gui")]
    swapchain_images: Vec<u32>,
    #[cfg(feature = "gui")]
    pub render_signal: RenderSignal,
    #[cfg(feature = "gui")]
    debug_window_swapchain: Option<xr::Swapchain<xr::OpenGlEs>>,

    view_reference_space: Option<xr::Space>,
    // `xr::Action<xr::Posef>` doesn't implement `Cmp` or `Hash`, so use `xr_sys::Action`.
    possible_spaces: HashMap<(xr_sys::Action, xr::Path), xr_sys::Space>,
    // See above why not `xr::Action<xr::Posef>`.
    eye_gaze_action: Option<xr_sys::Action>,
    eye_gaze_space: Option<xr_sys::Space>,
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
            eye_gaze_action: None,
            create_action_space: None,
            eye_gaze_space: None,
            get_action_state_pose: None,
            locate_space: None,
            locate_views: None,
            create_session: None,
            initalize_loader_khr: None,
            create_swapchain: None,
            end_frame: None,
            begin_frame: None,
            create_reference_space: None,
            possible_spaces: HashMap::new(),

            #[cfg(feature = "gui")]
            egl_pointers: None,
            #[cfg(feature = "gui")]
            egl_image: Default::default(),
            #[cfg(feature = "gui")]
            swapchain_images: Default::default(),
            #[cfg(feature = "gui")]
            render_signal: Default::default(),
            #[cfg(feature = "gui")]
            debug_window_swapchain: None,

            view_reference_space: None,
        }
    }

    fn on_session_created(&mut self) {
        let session = self.session.as_ref().unwrap();

        #[cfg(feature = "gui")]
        {
            use crate::ui::{UI_WINDOW_H, UI_WINDOW_W};

            let swapchain = session
                .create_swapchain(&xr::SwapchainCreateInfo {
                    create_flags: SwapchainCreateFlags::EMPTY,
                    // TODO: Set the proper flags, those are copied from Steam Link.
                    usage_flags: SwapchainUsageFlags::COLOR_ATTACHMENT
                        | SwapchainUsageFlags::SAMPLED,
                    format: glow::SRGB8_ALPHA8,
                    sample_count: 1,
                    width: UI_WINDOW_W,
                    height: UI_WINDOW_H,
                    face_count: 1,
                    array_size: 1,
                    mip_count: 1,
                })
                .expect("failed to create swapchain");
            self.swapchain_images = swapchain.enumerate_images().unwrap();
            self.debug_window_swapchain = Some(swapchain);
        }

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
    ) -> xr_sys::Result {
        unsafe {
            let create_info = &*create_info;
            let next = &*(create_info.next as *const BaseInStructure);

            debug!("create_info {create_info:?}");
            debug!("create_info.next {next:?}");

            match next.ty {
                xr_sys::StructureType::GRAPHICS_BINDING_OPENGL_ES_ANDROID_KHR => {
                    let graphics_binding_open_glesandroid_khr =
                        &*(create_info.next as *mut GraphicsBindingOpenGLESAndroidKHR);
                    info!(
                        "graphics_binding_open_glesandroid_khr {graphics_binding_open_glesandroid_khr:#?}"
                    );

                    #[cfg(feature = "gui")]
                    {
                        self.egl_pointers = Some(EGLPointers {
                            config: graphics_binding_open_glesandroid_khr.config,
                            context: graphics_binding_open_glesandroid_khr.context,
                            display: graphics_binding_open_glesandroid_khr.display,
                        });
                    }
                }
                // `xr_sys::StructureType::GRAPHICS_BINDING_VULKAN2_KHR` is an alias for the one below.
                xr_sys::StructureType::GRAPHICS_BINDING_VULKAN_KHR => {
                    let graphics_binding_vulkan_khr =
                        &*(create_info.next as *mut GraphicsBindingVulkanKHR);
                    info!(
                        "graphics_binding_vulkan_khr {graphics_binding_vulkan_khr:#?}"
                    );
                }
                _ => {}
            }

            let result = self.create_session.unwrap()(instance, create_info, session);

            if result != xr_sys::Result::SUCCESS {
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

            xr_sys::Result::SUCCESS
        }
    }

    pub unsafe fn initalize_loader_khr(
        &mut self,
        loader_init_info: *const LoaderInitInfoBaseHeaderKHR,
    ) -> xr_sys::Result {
        unsafe {
            // OpenXR is only for Android so far.
            assert_eq!(
                (*loader_init_info).ty,
                xr_sys::StructureType::LOADER_INIT_INFO_ANDROID_KHR
            );

            let loader_init_info_android = &*(loader_init_info as *const LoaderInitInfoAndroidKHR);

            println!("loader_init_info_android: {loader_init_info_android:?}");

            ndk_context::initialize_android_context(
                loader_init_info_android.application_vm,
                loader_init_info_android.application_context,
            );

            // From the OpenXR specs:
            // If the xrInitializeLoaderKHR function is discovered through the manifest,
            // xrInitializeLoaderKHR will be called before xrNegotiateLoaderRuntimeInterface or
            // xrNegotiateLoaderApiLayerInterface has been called on the runtime or layer respectively.
            //
            // Means we cannot call the next xrInitializeLoaderKHR function here to return the result.
            xr_sys::Result::SUCCESS
        }
    }

    pub unsafe fn create_swapchain(
        &self,
        session: xr_sys::Session,
        create_info: *const SwapchainCreateInfo,
        swapchain: *mut xr_sys::Swapchain,
    ) -> xr_sys::Result {
        unsafe {
            println!("create_info: {:?}", *create_info);

            let next = (*create_info).next;
            if !next.is_null() {
                let next = next as *const BaseInStructure;
                println!("{next:?}");
                if (*next).ty == xr_sys::StructureType::ANDROID_SURFACE_SWAPCHAIN_CREATE_INFO_FB {
                    let next = next as *const xr_sys::AndroidSurfaceSwapchainCreateInfoFB;
                    println!("{next:?}");
                }
            }

            let result = self.create_swapchain.unwrap()(session, create_info, swapchain);

            println!("swapchain: {:?}", *swapchain);

            result
        }
    }

    pub unsafe fn end_frame(
        &mut self,
        session: xr_sys::Session,
        frame_end_info: *const FrameEndInfo,
    ) -> xr_sys::Result {
        // Create our copy of FrameEndInfo.
        let mut frame_end_info = unsafe { *frame_end_info };

        let mut owned = Vec::<Box<dyn Any>>::new();

        let mut layers = Vec::from(unsafe {
            std::slice::from_raw_parts(frame_end_info.layers, frame_end_info.layer_count as usize)
        });

        #[cfg(feature = "gui")]
        {
            use crate::ui::{UI_WINDOW_H, UI_WINDOW_W};

            let egl_pointers = self.egl_pointers.as_mut().unwrap();

            let swapchain = self.debug_window_swapchain.as_mut().unwrap();
            let image_index = swapchain.acquire_image().unwrap();
            swapchain.wait_image(xr::Duration::INFINITE).unwrap();

            self.egl_image = self.swapchain_images[image_index as usize];

            // (Steam Link) Unbind the context from the thread, assuming it's here.
            unsafe {
                eglMakeCurrent(
                    egl_pointers.display,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            };

            // Wake up the render thread.
            {
                let mut ready = self.render_signal.mutex.lock().unwrap();
                *ready = true;
                self.render_signal.condvar.notify_one();
            }

            // Render thread does the rendering.

            // Wait for the render thread to finish.
            {
                let mut ready = self.render_signal.mutex.lock().unwrap();
                while *ready {
                    ready = self.render_signal.condvar.wait(ready).unwrap();
                }
            }

            // (Steam Link) Bind the context back.
            unsafe {
                eglMakeCurrent(
                    egl_pointers.display,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    egl_pointers.context,
                )
            };

            swapchain.release_image().unwrap();

            let q = quat::from_euler_angles(
                quat::RotationType::Extrinsic,
                quat::RotationSequence::XYZ,
                [-45f32.to_radians(), 0.0, 0.0],
            );

            let my_layer = Box::new(CompositionLayerQuad {
                ty: xr_sys::StructureType::COMPOSITION_LAYER_QUAD,
                next: ptr::null(),
                eye_visibility: EyeVisibility::BOTH,
                layer_flags: CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA,
                space: self
                    .view_reference_space
                    .as_ref()
                    .expect("view reference space is not initialized")
                    .as_raw(),
                pose: Posef {
                    position: Vector3f {
                        x: 0.0,
                        y: -0.10,
                        z: -0.20,
                    },
                    orientation: Quaternionf {
                        w: q.0,
                        x: q.1[0],
                        y: q.1[1],
                        z: q.1[2],
                    },
                },
                size: Extent2Df {
                    width: 0.16,
                    height: 0.09,
                },
                sub_image: SwapchainSubImage {
                    swapchain: self.debug_window_swapchain.as_ref().unwrap().as_raw(),
                    image_rect: Rect2Di {
                        extent: Extent2Di {
                            width: UI_WINDOW_W as i32,
                            height: UI_WINDOW_H as i32,
                        },
                        offset: Offset2Di { x: 0, y: 0 },
                    },
                    image_array_index: 0,
                },
            });

            // TODO: Is there a better way to do this cast?
            layers.push(
                (&*my_layer as *const CompositionLayerQuad) as *const CompositionLayerBaseHeader,
            );
            owned.push(my_layer);
        }

        frame_end_info.layers = layers.as_ptr();
        frame_end_info.layer_count = layers.len() as u32;

        assert_eq!(
            unsafe { self.end_frame.unwrap()(session, &frame_end_info) },
            xr_sys::Result::SUCCESS
        );

        xr_sys::Result::SUCCESS
    }

    pub unsafe fn begin_frame(
        &mut self,
        session: xr_sys::Session,
        frame_begin_info: *const FrameBeginInfo,
    ) -> xr_sys::Result {
        unsafe { self.begin_frame.unwrap()(session, frame_begin_info) }
    }

    pub unsafe fn enumerate_instance_extension_properties(
        &self,
        layer_name: *const c_char,
        property_capacity_input: u32,
        property_count_output: *mut u32,
        properties_ptr: *mut xr_sys::ExtensionProperties,
    ) -> xr_sys::Result {
        let mut result = unsafe {
            self.enumerate_instance_extensions_properties.unwrap()(
                layer_name,
                property_capacity_input,
                property_count_output,
                properties_ptr,
            )
        };

        let base_offset = unsafe { *property_count_output } as usize;
        unsafe { *property_count_output += ADVERTISED_EXTENSIONS.len() as u32 };
        if property_capacity_input > 0 {
            if property_capacity_input < unsafe { *property_count_output } {
                result = xr_sys::Result::ERROR_SIZE_INSUFFICIENT;
            } else {
                result = xr_sys::Result::SUCCESS;

                let properties = unsafe {
                    std::slice::from_raw_parts_mut(
                        properties_ptr,
                        (*property_count_output).try_into().unwrap(),
                    )
                };

                for i in base_offset..unsafe { *property_count_output } as usize {
                    if properties[i].ty != xr_sys::StructureType::EXTENSION_PROPERTIES {
                        result = xr_sys::Result::ERROR_VALIDATION_FAILURE;
                        break;
                    }

                    let extension = &ADVERTISED_EXTENSIONS[i - base_offset];

                    unsafe {
                        std::ptr::copy(
                            extension.name.as_ptr(),
                            properties[i].extension_name.as_mut_ptr() as *mut u8,
                            extension.name.len(),
                        )
                    };
                    properties[i].extension_version = extension.version;
                }
            }
        }

        result
    }

    pub unsafe fn get_system_properties(
        &self,
        instance: xr_sys::Instance,
        system_id: xr_sys::SystemId,
        properties: *mut xr_sys::SystemProperties,
    ) -> xr_sys::Result {
        // TODO: this is still not following the specs. It does hide the
        // structures, but also can reorder them after unhiding. The specs
        // say modifying `type` and `next` are prohibited, so no reordering.
        let mut hidden_properties = Vec::new();

        let mut prev_property_ptr = ptr::null_mut::<xr_sys::BaseOutStructure>();
        let mut property_ptr = properties as *mut xr_sys::BaseOutStructure;
        while !property_ptr.is_null() {
            let property = unsafe { &mut *property_ptr };

            println!("--> get_system_properties {property:#?}");

            // Process the properties.
            match property.ty {
                xr_sys::StructureType::SYSTEM_EYE_GAZE_INTERACTION_PROPERTIES_EXT => {
                    let property = unsafe {
                        &mut *(property_ptr as *mut xr_sys::SystemEyeGazeInteractionPropertiesEXT)
                    };
                    property.supports_eye_gaze_interaction = true.into();

                    hidden_properties.push(property_ptr);
                }

                xr_sys::StructureType::SYSTEM_FACE_TRACKING_PROPERTIES2_FB => {
                    let property = unsafe {
                        &mut *(property_ptr as *mut xr_sys::SystemFaceTrackingProperties2FB)
                    };
                    property.supports_visual_face_tracking = true.into();
                    property.supports_audio_face_tracking = false.into();
                }

                xr_sys::StructureType::SYSTEM_EYE_TRACKING_PROPERTIES_FB => {
                    let property = unsafe {
                        &mut *(property_ptr as *mut xr_sys::SystemEyeTrackingPropertiesFB)
                    };
                    property.supports_eye_tracking = true.into();
                }

                _ => {}
            }

            // Hide the properties from the runtime.
            match property.ty {
                xr_sys::StructureType::SYSTEM_EYE_GAZE_INTERACTION_PROPERTIES_EXT
                | xr_sys::StructureType::SYSTEM_FACE_TRACKING_PROPERTIES2_FB
                | xr_sys::StructureType::SYSTEM_EYE_TRACKING_PROPERTIES_FB => {
                    hidden_properties.push(property_ptr);

                    if !prev_property_ptr.is_null() {
                        unsafe {
                            (*prev_property_ptr).next = property.next;
                        }
                    }
                }

                _ => {}
            }

            prev_property_ptr = property_ptr;
            property_ptr = property.next;
        }

        let result =
            unsafe { self.get_system_properties.unwrap()(instance, system_id, properties) };
        if result != xr_sys::Result::SUCCESS {
            println!("get_system_properties result: {result:?}");
            return result;
        }

        // Find the end of the chain.
        let mut prev_property_ptr = ptr::null_mut::<xr_sys::BaseOutStructure>();
        let mut property_ptr = properties as *mut xr_sys::BaseOutStructure;
        while !property_ptr.is_null() {
            let property = unsafe { &mut *property_ptr };
            prev_property_ptr = property_ptr;
            property_ptr = property.next;
        }

        // Add the hidden properties back.
        if !prev_property_ptr.is_null() {
            for hidden_property_ptr in hidden_properties {
                property_ptr = hidden_property_ptr;
                unsafe {
                    (*prev_property_ptr).next = property_ptr;
                }
                prev_property_ptr = property_ptr;
            }
            unsafe {
                (*prev_property_ptr).next = ptr::null_mut();
            }
        }

        println!("<-- get_system_properties");
        xr_sys::Result::SUCCESS
    }

    pub unsafe fn suggest_interaction_profile_bindings(
        &mut self,
        instance: xr_sys::Instance,
        suggested_bindings: *const xr_sys::InteractionProfileSuggestedBinding,
    ) -> xr_sys::Result {
        let xr_instance = self.instance.as_mut().unwrap();

        let suggested_bindings = unsafe { &*suggested_bindings };

        let interaction_profile = xr_instance
            .path_to_string(suggested_bindings.interaction_profile)
            .unwrap();

        println!(
            "suggest_interaction_profile_bindings {:?} {}",
            suggested_bindings, interaction_profile
        );

        if interaction_profile != "/interaction_profiles/ext/eye_gaze_interaction" {
            return unsafe {
                self.suggest_interaction_profile_bindings.unwrap()(instance, suggested_bindings)
            };
        }

        let suggested_bindings = unsafe {
            std::slice::from_raw_parts(
                suggested_bindings.suggested_bindings,
                suggested_bindings
                    .count_suggested_bindings
                    .try_into()
                    .unwrap(),
            )
        };

        for suggested_binding in suggested_bindings {
            let binding = xr_instance
                .path_to_string(suggested_binding.binding)
                .unwrap();
            println!("suggest_interaction_profile_bindings binding path {binding}");
            if binding == "/user/eyes_ext/input/gaze_ext/pose" {
                self.eye_gaze_action = Some(suggested_binding.action);
                println!(
                    "suggest_interaction_profile_bindings saved eye gaze action {:?}",
                    suggested_binding.action
                );

                assert_eq!(
                    self.possible_spaces
                        .keys()
                        .filter(|(action, _)| action == &suggested_binding.action)
                        .count(),
                    1,
                    "more than one subaction paths exist for binding `/user/eyes_ext/input/gaze_ext/pose`"
                );

                let gaze_space = *self.possible_spaces
                        .get(&(suggested_binding.action, xr::Path::NULL))
                        .expect("eye tracking interaction profile binding suggested, but no corresponding action space was found");
                println!("gaze space found {gaze_space:?}");

                self.eye_gaze_space = Some(gaze_space);

                self.possible_spaces.clear();
            }
        }

        xr_sys::Result::SUCCESS
    }

    pub unsafe fn create_action_space(
        &mut self,
        session: xr_sys::Session,
        create_info: *const xr_sys::ActionSpaceCreateInfo,
        space: *mut xr_sys::Space,
    ) -> xr_sys::Result {
        unsafe {
            println!("--> create_action_space {:?}", *create_info);
            let result = self.create_action_space.unwrap()(session, create_info, space);
            if result != xr_sys::Result::SUCCESS {
                return result;
            }

            // Spaced are created before actions, so save them all and choose later when the action is known.
            let create_info = &*create_info;
            self.possible_spaces
                .insert((create_info.action, create_info.subaction_path), *space);

            println!("<-- create_action_space");
            xr_sys::Result::SUCCESS
        }
    }

    pub unsafe fn get_action_state_pose(
        &self,
        session: xr_sys::Session,
        get_info: *const xr_sys::ActionStateGetInfo,
        state: *mut xr_sys::ActionStatePose,
    ) -> xr_sys::Result {
        unsafe {
            if self
                .eye_gaze_action
                .is_some_and(|eye_gaze_action| eye_gaze_action != (*get_info).action)
            {
                return self.get_action_state_pose.unwrap()(session, get_info, state);
            }

            // println!("--> get_action_state_pose {:?}", (*get_info).subaction_path);

            let state = &mut *state;

            // Report tracking as disabled if there is no data incoming.
            state.is_active = OPENXR_OUTPUT_BRIDGE
                .get()
                .map(|mutex| mutex.lock().expect("failed to lock OpenXR output bridge"))
                // False if there's no bridge yet.
                .is_some_and(|mut bridge| {
                    // False if there has been no data yet.
                    bridge.get_eyes_state().is_some_and(|gaze| {
                        gaze.timestamp.elapsed().unwrap() < std::time::Duration::from_millis(50)
                    })
                })
                .into();

            // println!("<-- get_action_state_pose");
            xr_sys::Result::SUCCESS
        }
    }

    pub unsafe fn locate_space(
        &self,
        space: xr_sys::Space,
        base_space: xr_sys::Space,
        time: xr_sys::Time,
        location: *mut xr_sys::SpaceLocation,
    ) -> xr_sys::Result {
        unsafe {
            // If the requested space is not eye tracking space, pass through the call.
            if self
                .eye_gaze_space
                .is_some_and(|eye_gaze_space| eye_gaze_space != space)
            {
                return self.locate_space.unwrap()(space, base_space, time, location);
            }

            trace!("--> locate_space {:?} {:?} {:?}", space, base_space, time);

            // Determine where the VIEW space is in relation to the requested `base_space`.
            let base_from_view_space = {
                let mut view_location = xr_sys::SpaceLocation {
                    ty: xr_sys::StructureType::SPACE_LOCATION,
                    next: std::ptr::null_mut(),
                    location_flags: xr_sys::SpaceLocationFlags::EMPTY,
                    pose: xr_sys::Posef {
                        orientation: xr_sys::Quaternionf {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                            w: 1.0,
                        },
                        position: xr_sys::Vector3f {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                    },
                };
                let result = self.locate_space.unwrap()(
                    self.view_reference_space.as_ref().unwrap().as_raw(),
                    base_space,
                    time,
                    &mut view_location,
                );
                if result != xr_sys::Result::SUCCESS {
                    return result;
                }
                view_location.pose
            };

            let location: &mut openxr_sys::SpaceLocation = &mut *location;

            let Some(eyes_state) = OPENXR_OUTPUT_BRIDGE
                .get()
                .expect("requested for gaze, but bridge was not initialized yet")
                .lock()
                .expect("failed to lock OpenXR output bridge")
                .get_eyes_state()
            else {
                location.location_flags &= !xr_sys::SpaceLocationFlags::POSITION_TRACKED;
                location.location_flags &= !xr_sys::SpaceLocationFlags::ORIENTATION_TRACKED;
                return xr_sys::Result::SUCCESS;
            };

            location.location_flags |= xr_sys::SpaceLocationFlags::POSITION_TRACKED;
            location.location_flags |= xr_sys::SpaceLocationFlags::ORIENTATION_TRACKED;

            let q_gaze_in_view = quat_from_pitch_yaw(eyes_state.gaze_pitch, eyes_state.gaze_yaw);
            let q_base_in_view: quat::Quaternion<f32> = (
                base_from_view_space.orientation.w,
                [
                    base_from_view_space.orientation.x,
                    base_from_view_space.orientation.y,
                    base_from_view_space.orientation.z,
                ],
            );

            // Convert gaze from the VIEW space into `base_space`.
            let q_gaze_in_base = quat::mul(q_base_in_view, q_gaze_in_view);

            location.pose.position = Vector3f {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            location.pose.orientation = Quaternionf {
                w: q_gaze_in_base.0,
                x: q_gaze_in_base.1[0],
                y: q_gaze_in_base.1[1],
                z: q_gaze_in_base.1[2],
            };

            // println!("locate_space {:?}", location);

            if !location.next.is_null() {
                let eye_gaze_sample_time =
                    &mut *(location.next as *mut xr_sys::EyeGazeSampleTimeEXT);
                eye_gaze_sample_time.time = xr_sys::Time::from_nanos(0);
                // println!("locate_space {:?}", eye_gaze_sample_time);
            }

            xr_sys::Result::SUCCESS
        }
    }

    pub unsafe fn create_face_tracker2(
        &self,
        _session: xr_sys::Session,
        create_info: *const xr_sys::FaceTrackerCreateInfo2FB,
        face_tracker: *mut xr_sys::FaceTracker2FB,
    ) -> xr_sys::Result {
        unsafe {
            println!("--> create_face_tracker2 {:#?}", *create_info);

            *face_tracker = xr_sys::FaceTracker2FB::from_raw(99999990);
        }
        xr_sys::Result::SUCCESS
    }

    pub unsafe fn destroy_face_tracker2(
        &self,
        face_tracker: xr_sys::FaceTracker2FB,
    ) -> xr_sys::Result {
        println!("--> destroy_face_tracker2 {:#?}", face_tracker);
        xr_sys::Result::SUCCESS
    }

    pub unsafe fn get_face_expression_weights2(
        &self,
        _face_tracker: xr_sys::FaceTracker2FB,
        expression_info: *const xr_sys::FaceExpressionInfo2FB,
        expression_weights: *mut xr_sys::FaceExpressionWeights2FB,
    ) -> xr_sys::Result {
        unsafe {
            let expression_info = &*expression_info;
            let expression_weights = &mut *expression_weights;

            // println!("--> get_face_expression_weights2 {:?}", expression_info);

            if expression_weights.weight_count != FaceExpression2FB::COUNT.into_raw() as u32 {
                return xr_sys::Result::ERROR_VALIDATION_FAILURE;
            }
            let face_expressions = std::slice::from_raw_parts_mut(
                expression_weights.weights,
                expression_weights.weight_count as usize,
            );

            if expression_weights.confidence_count != FaceConfidence2FB::COUNT.into_raw() as u32 {
                return xr_sys::Result::ERROR_VALIDATION_FAILURE;
            }
            let face_confidences = std::slice::from_raw_parts_mut(
                expression_weights.confidences,
                expression_weights.confidence_count as usize,
            );

            // Retrieve the gaze data.
            let eyes_state = OPENXR_OUTPUT_BRIDGE
                .get()
                .expect("requested for gaze, but bridge was not initialized yet")
                .lock()
                .expect("failed to lock OpenXR output bridge")
                .get_eyes_state();

            let Some(eyes_state) = eyes_state else {
                expression_weights.is_valid = false.into();
                return xr_sys::Result::SUCCESS;
            };

            // Very confident...
            face_confidences.fill(1.0);

            let remap = |value: f32, low1: f32, high1: f32, low2: f32, high2: f32| {
                (low2 + (value - low1) * (high2 - low2) / (high1 - low1)).clamp(0.0, 1.0)
            };

            for (i, mut _face_expression) in face_expressions.iter_mut().enumerate() {
                *_face_expression = match FaceExpression2FB::from_raw(i as i32) {
                    FaceExpression2FB::UPPER_LID_RAISER_L => {
                        remap(eyes_state.l_eyelid, 0.75, 1.0, 0.0, 1.0)
                    }
                    FaceExpression2FB::UPPER_LID_RAISER_R => {
                        remap(eyes_state.r_eyelid, 0.75, 1.0, 0.0, 1.0)
                    }

                    FaceExpression2FB::EYES_CLOSED_L => {
                        remap(eyes_state.l_eyelid, 0.75, 0.0, 0.0, 1.0)
                    }
                    FaceExpression2FB::EYES_CLOSED_R => {
                        remap(eyes_state.r_eyelid, 0.75, 0.0, 0.0, 1.0)
                    }

                    FaceExpression2FB::EYES_LOOK_LEFT_L => {
                        remap(eyes_state.l_yaw, 0.0, -45.0, 0.0, 1.0)
                    }
                    FaceExpression2FB::EYES_LOOK_RIGHT_L => {
                        remap(eyes_state.l_yaw, 0.0, 45.0, 0.0, 1.0)
                    }
                    FaceExpression2FB::EYES_LOOK_UP_L => {
                        remap(eyes_state.pitch, 0.0, 45.0, 0.0, 1.0)
                    }
                    FaceExpression2FB::EYES_LOOK_DOWN_L => {
                        remap(eyes_state.pitch, 0.0, -45.0, 0.0, 1.0)
                    }

                    FaceExpression2FB::EYES_LOOK_LEFT_R => {
                        remap(eyes_state.r_yaw, 0.0, -45.0, 0.0, 1.0)
                    }
                    FaceExpression2FB::EYES_LOOK_RIGHT_R => {
                        remap(eyes_state.r_yaw, 0.0, 45.0, 0.0, 1.0)
                    }
                    FaceExpression2FB::EYES_LOOK_UP_R => {
                        remap(eyes_state.pitch, 0.0, 45.0, 0.0, 1.0)
                    }
                    FaceExpression2FB::EYES_LOOK_DOWN_R => {
                        remap(eyes_state.pitch, 0.0, -45.0, 0.0, 1.0)
                    }

                    _ => 0.0,
                };

                // println!(
                //     "{:?} = {}",
                //     FaceExpression2FB::from_raw(i as i32),
                //     _face_expression
                // );
            }

            expression_weights.data_source = xr_sys::FaceTrackingDataSource2FB::VISUAL;
            expression_weights.is_eye_following_blendshapes_valid = true.into();
            expression_weights.is_valid = true.into();
            expression_weights.time = expression_info.time;

            // println!("{expression_weights:#?}");
        }
        xr_sys::Result::SUCCESS
    }

    // XR_FB_eye_tracking_social

    pub unsafe fn create_eye_tracker_fb(
        &self,
        _session: xr_sys::Session,
        create_info: *const xr_sys::EyeTrackerCreateInfoFB,
        eye_tracker: *mut xr_sys::EyeTrackerFB,
    ) -> xr_sys::Result {
        unsafe {
            debug!("--> create_eye_tracker_fb {:#?}", *create_info);

            *eye_tracker = xr_sys::EyeTrackerFB::from_raw(99999991);
        }
        xr_sys::Result::SUCCESS
    }

    pub unsafe fn destroy_eye_tracker(&self, eye_tracker: xr_sys::EyeTrackerFB) -> xr_sys::Result {
        debug!("--> destroy_eye_tracker {:#?}", eye_tracker);
        xr_sys::Result::SUCCESS
    }

    pub unsafe fn get_eye_gazes_fb(
        &self,
        _eye_tracker: xr_sys::EyeTrackerFB,
        gaze_info: *const xr_sys::EyeGazesInfoFB,
        eye_gazes: *mut xr_sys::EyeGazesFB,
    ) -> xr_sys::Result {
        unsafe {
            let gaze_info = &*gaze_info;
            trace!("--> get_eye_gazes_fb {gaze_info:?}, eye_gazes: {eye_gazes:?}");
            let eye_gazes = &mut *eye_gazes;

            // Determine where the VIEW space is in relation to the requested `base_space`.
            let base_from_view_space = {
                let mut view_location = xr_sys::SpaceLocation {
                    ty: xr_sys::StructureType::SPACE_LOCATION,
                    next: std::ptr::null_mut(),
                    location_flags: xr_sys::SpaceLocationFlags::EMPTY,
                    pose: xr_sys::Posef {
                        orientation: xr_sys::Quaternionf {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                            w: 1.0,
                        },
                        position: xr_sys::Vector3f {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                    },
                };
                let result = self.locate_space.unwrap()(
                    self.view_reference_space.as_ref().unwrap().as_raw(),
                    gaze_info.base_space,
                    gaze_info.time,
                    &mut view_location,
                );
                if result != xr_sys::Result::SUCCESS {
                    return result;
                }
                view_location.pose
            };

            // Those are not defined in `xr_sys`.
            const EYE_POSITION_LEFT_FB: usize = 0;
            const EYE_POSITION_RIGHT_FB: usize = 1;

            let eyes_state = OPENXR_OUTPUT_BRIDGE
                .get()
                .expect("requested for gaze, but bridge was not initialized yet")
                .lock()
                .expect("failed to lock OpenXR output bridge")
                .get_eyes_state();

            let pitch_yaw_to_pose = |pitch: f32, yaw: f32, is_left: bool| {
                let mut q_gaze_in_view = quat_from_pitch_yaw(pitch, yaw);

                let q_base_in_view: quat::Quaternion<f32> = (
                    base_from_view_space.orientation.w,
                    [
                        base_from_view_space.orientation.x,
                        base_from_view_space.orientation.y,
                        base_from_view_space.orientation.z,
                    ],
                );

                // Convert gaze from the VIEW space into `base_space`.
                let q_gaze_in_base = quat::mul(q_base_in_view, q_gaze_in_view);

                xr_sys::Posef {
                    orientation: Quaternionf {
                        w: q_gaze_in_base.0,
                        x: q_gaze_in_base.1[0],
                        y: q_gaze_in_base.1[1],
                        z: q_gaze_in_base.1[2],
                    },
                    position: Vector3f {
                        // TODO: can't find anything in the specs about this, but QPro seems to include IPD
                        // (at least some, idk if it's actual IPD) to position eye gaze origins.
                        // Steam Link refuses to work without this.
                        x: if is_left { -0.0325 } else { 0.0325 },
                        y: 0.0,
                        z: 0.0,
                    },
                }
            };

            let Some(eyes_state) = eyes_state else {
                eye_gazes.gaze[EYE_POSITION_LEFT_FB].is_valid = false.into();
                eye_gazes.gaze[EYE_POSITION_RIGHT_FB].is_valid = false.into();

                return xr_sys::Result::SUCCESS;
            };

            eye_gazes.gaze[EYE_POSITION_LEFT_FB] = openxr_sys::EyeGazeFB {
                is_valid: true.into(),
                gaze_pose: pitch_yaw_to_pose(eyes_state.pitch, eyes_state.l_yaw, true),
                gaze_confidence: 1.0,
            };
            eye_gazes.gaze[EYE_POSITION_RIGHT_FB] = openxr_sys::EyeGazeFB {
                is_valid: true.into(),
                gaze_pose: pitch_yaw_to_pose(eyes_state.pitch, eyes_state.r_yaw, false),
                gaze_confidence: 1.0,
            };
            eye_gazes.time = gaze_info.time;
        }

        // println!("{:#?}", unsafe { *eye_gazes });
        xr_sys::Result::SUCCESS
    }

    /*
    pub unsafe fn locate_views(
        &self,
        session: xr_sys::Session,
        view_locate_info: *const xr_sys::ViewLocateInfo,
        view_state: *mut xr_sys::ViewState,
        view_capacity_input: u32,
        view_count_output: *mut u32,
        views: *mut xr_sys::View,
    ) -> xr_sys::Result {
        unsafe {
            let res = self.locate_views.unwrap()(
                session,
                view_locate_info,
                view_state,
                view_capacity_input,
                view_count_output,
                views,
            );

            if res != xr_sys::Result::SUCCESS {
                return res;
            }

            if (*view_locate_info).view_configuration_type
                != xr_sys::ViewConfigurationType::PRIMARY_STEREO
            {
                return xr_sys::Result::SUCCESS;
            }

            let views =
                std::slice::from_raw_parts_mut(views, (*view_count_output).try_into().unwrap());

            let apply_pupil_offset = |view: &mut xr_sys::View, is_left: bool| {
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

            xr_sys::Result::SUCCESS
        }
    }
    */
}
