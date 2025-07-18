// FIXME: Apparently this is undefined behavior, figure this out.
#![warn(static_mut_refs)]

use std::ffi::CStr;
use std::ffi::c_char;

use crate::openxr_layer::layer::LAYER;

use openxr::{self as xr};

use openxr_sys::FrameBeginInfo;
use openxr_sys::FrameEndInfo;
use openxr_sys::LoaderInitInfoBaseHeaderKHR;
use openxr_sys::Session;
use openxr_sys::SessionCreateInfo;
use openxr_sys::Swapchain;
use openxr_sys::SwapchainCreateInfo;
use openxr_sys::{Instance, Result, pfn};

use openxr_sys::{InstanceCreateInfo, loader::ApiLayerCreateInfo};

use openxr_sys::{
    loader,
    loader::{XrNegotiateApiLayerRequest, XrNegotiateLoaderInfo},
};

use crate::openxr_layer::dispatch;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn xrNegotiateLoaderApiLayerInterface(
    loader_info_ptr: *mut XrNegotiateLoaderInfo,
    _api_layer_name: *mut c_char,
    api_layer_request_ptr: *mut XrNegotiateApiLayerRequest,
) -> openxr_sys::Result {
    unsafe {
        println!("--> xrNegotiateLoaderApiLayerInterface");

        // if (apiLayerName && std::string_view(apiLayerName) != LAYER_NAME) {
        //     ErrorLog(fmt::format("Invalid apiLayerName \"{}\"\n", apiLayerName));
        //     return XR_ERROR_INITIALIZATION_FAILED;
        // }

        // let loader_info = loader_info

        assert!(!loader_info_ptr.is_null());
        assert!(!api_layer_request_ptr.is_null());

        // if loader_info_ptr.is_null() || api_layer_request_ptr.is_null() {
        //     println!("xrNegotiateLoaderApiLayerInterface validation failed");
        //     return Result::ERROR_INITIALIZATION_FAILED;
        // }

        let loader_info = &mut *loader_info_ptr;
        let api_layer_request = &mut *api_layer_request_ptr;

        assert!(loader_info.ty == XrNegotiateLoaderInfo::TYPE);
        assert!(loader_info.struct_version == XrNegotiateLoaderInfo::VERSION);
        assert!(loader_info.struct_size == std::mem::size_of::<XrNegotiateLoaderInfo>());
        assert!(api_layer_request.ty == XrNegotiateApiLayerRequest::TYPE);
        assert!(api_layer_request.struct_version == XrNegotiateApiLayerRequest::VERSION);
        assert!(api_layer_request.struct_size == std::mem::size_of::<XrNegotiateApiLayerRequest>());
        assert!(loader_info.min_interface_version <= loader::CURRENT_LOADER_API_LAYER_VERSION);
        assert!(loader_info.max_interface_version >= loader::CURRENT_LOADER_API_LAYER_VERSION);
        assert!(loader_info.max_interface_version <= loader::CURRENT_LOADER_API_LAYER_VERSION);
        assert!(loader_info.max_api_version >= openxr_sys::CURRENT_API_VERSION);
        assert!(loader_info.min_api_version <= openxr_sys::CURRENT_API_VERSION);

        // if loader_info.ty != XrNegotiateLoaderInfo::TYPE
        //     || loader_info.struct_version != XrNegotiateLoaderInfo::VERSION
        //     || loader_info.struct_size != std::mem::size_of::<XrNegotiateLoaderInfo>()
        //     || api_layer_request.ty != XrNegotiateApiLayerRequest::TYPE
        //     || api_layer_request.struct_version != XrNegotiateApiLayerRequest::VERSION
        //     || api_layer_request.struct_size != std::mem::size_of::<XrNegotiateApiLayerRequest>()
        //     || loader_info.min_interface_version > loader::CURRENT_LOADER_API_LAYER_VERSION
        //     || loader_info.max_interface_version < loader::CURRENT_LOADER_API_LAYER_VERSION
        //     || loader_info.max_interface_version > loader::CURRENT_LOADER_API_LAYER_VERSION
        //     || loader_info.max_api_version < openxr_sys::CURRENT_API_VERSION
        //     || loader_info.min_api_version > openxr_sys::CURRENT_API_VERSION
        // {
        //     println!("xrNegotiateLoaderApiLayerInterface validation failed");
        //     return Result::ERROR_INITIALIZATION_FAILED;
        // }

        // Setup our layer to intercept OpenXR calls.
        api_layer_request.layer_interface_version = loader::CURRENT_LOADER_API_LAYER_VERSION;
        api_layer_request.layer_api_version = openxr_sys::CURRENT_API_VERSION;
        api_layer_request.get_instance_proc_addr = Some(dispatch::xr_get_instance_proc_addr);
        api_layer_request.create_api_layer_instance = Some(dispatch::xr_create_api_layer_instance);
        // apiLayerRequest->getInstanceProcAddr = reinterpret_cast<PFN_xrGetInstanceProcAddr>(xrGetInstanceProcAddr);
        // apiLayerRequest->createApiLayerInstance = reinterpret_cast<PFN_xrCreateApiLayerInstance>(xrCreateApiLayerInstance);

        println!("<-- xrNegotiateLoaderApiLayerInterface");

        openxr_sys::Result::SUCCESS
    }
}

pub unsafe extern "system" fn xr_create_api_layer_instance(
    instance_create_info_ptr: *const InstanceCreateInfo,
    api_layer_info_ptr: *const ApiLayerCreateInfo,
    instance: *mut Instance,
) -> Result {
    unsafe {
        println!("--> xr_create_api_layer_instance");

        // Call the chain to create the instance.
        let mut chain_instance_create_info = *instance_create_info_ptr;

        // Hide our extension from the list assuming it's in the beginning.
        // Reduce the extension count by one and move the pointer one forward.
        // This is to avoid an `ERROR_EXTENSION_NOT_PRESENT` error from the runtime.
        chain_instance_create_info.enabled_extension_count -= 1;
        chain_instance_create_info.enabled_extension_names =
            chain_instance_create_info.enabled_extension_names.add(1);

        let api_layer_info = *api_layer_info_ptr;
        let mut chain_api_layer_info = api_layer_info;
        chain_api_layer_info.next_info = (*api_layer_info.next_info).next;
        let result = ((*api_layer_info.next_info).next_create_api_layer_instance)(
            &chain_instance_create_info,
            &chain_api_layer_info,
            instance,
        );

        println!("xr_create_api_layer_instance result: {result:?}");

        if result != Result::SUCCESS {
            return result;
        }

        let get_instance_proc_addr = (*api_layer_info.next_info).next_get_instance_proc_addr;

        let entry = xr::Entry::from_get_instance_proc_addr(get_instance_proc_addr).unwrap();
        let instance = xr::Instance::from_raw(
            entry.clone(),
            *instance,
            xr::InstanceExtensions::load(&entry, *instance, &xr::ExtensionSet::default()).unwrap(),
        )
        .unwrap();

        let layer = &mut LAYER;
        layer.get_instance_proc_addr = Some(get_instance_proc_addr);
        layer.instance = Some(instance);

        crate::android::main();

        println!("<-- xr_create_api_layer_instance");

        Result::SUCCESS
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn xrGetInstanceProcAddr(
    instance: Instance,
    name_ptr: *const c_char,
    function: *mut Option<pfn::VoidFunction>,
) -> openxr_sys::Result {
    unsafe { xr_get_instance_proc_addr(instance, name_ptr, function) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn xrInitializeLoaderKHR(
    loader_init_info: *const LoaderInitInfoBaseHeaderKHR,
) -> openxr_sys::Result {
    unsafe { xr_initialize_loader_khr(loader_init_info) }
}

pub unsafe extern "system" fn xr_get_instance_proc_addr(
    instance: Instance,
    name_ptr: *const c_char,
    function: *mut Option<pfn::VoidFunction>,
) -> Result {
    unsafe {
        let api_name = CStr::from_ptr(name_ptr).to_string_lossy().to_string();
        if instance == Instance::NULL
            && !(api_name == "xrEnumerateInstanceExtensionProperties"
                || api_name == "xrEnumerateApiLayerProperties"
                || api_name == "xrCreateInstance")
        {
            return Result::ERROR_HANDLE_INVALID;
        }

        println!(
            "xr_get_instance_proc_addr {:?} {}",
            instance,
            CStr::from_ptr(name_ptr).to_str().unwrap()
        );

        let layer = &mut LAYER;

        let result = layer.get_instance_proc_addr.unwrap()(instance, name_ptr, function);

        if result != openxr_sys::Result::SUCCESS {
            return result;
        }

        if api_name == "xrCreateSession" {
            layer.create_session = Some(
                std::mem::transmute::<pfn::VoidFunction, pfn::CreateSession>((*function).unwrap()),
            );
            *function = Some(
                std::mem::transmute::<pfn::CreateSession, pfn::VoidFunction>(xr_create_session),
            );
        }

        if api_name == "xrInitializeLoaderKHR" {
            layer.initalize_loader_khr = Some(std::mem::transmute::<
                pfn::VoidFunction,
                pfn::InitializeLoaderKHR,
            >((*function).unwrap()));
            *function = Some(std::mem::transmute::<
                pfn::InitializeLoaderKHR,
                pfn::VoidFunction,
            >(xr_initialize_loader_khr));
        }

        if api_name == "xrCreateSwapchain" {
            layer.create_swapchain = Some(std::mem::transmute::<
                pfn::VoidFunction,
                pfn::CreateSwapchain,
            >((*function).unwrap()));
            *function = Some(
                std::mem::transmute::<pfn::CreateSwapchain, pfn::VoidFunction>(xr_create_swapchain),
            );
        }

        if api_name == "xrBeginFrame" {
            layer.begin_frame = Some(std::mem::transmute::<pfn::VoidFunction, pfn::BeginFrame>(
                (*function).unwrap(),
            ));
            *function = Some(std::mem::transmute::<pfn::BeginFrame, pfn::VoidFunction>(
                xr_begin_frame,
            ));
        }

        if api_name == "xrEndFrame" {
            layer.end_frame = Some(std::mem::transmute::<pfn::VoidFunction, pfn::EndFrame>(
                (*function).unwrap(),
            ));
            *function = Some(std::mem::transmute::<pfn::EndFrame, pfn::VoidFunction>(
                xr_end_frame,
            ));
        }

        if api_name == "xrCreateReferenceSpace" {
            layer.create_reference_space = Some(std::mem::transmute::<
                pfn::VoidFunction,
                pfn::CreateReferenceSpace,
            >((*function).unwrap()));
        }

        openxr_sys::Result::SUCCESS
    }
}

unsafe extern "system" fn xr_create_session(
    instance: Instance,
    create_info: *const SessionCreateInfo,
    session: *mut Session,
) -> Result {
    unsafe { LAYER.create_session(instance, create_info, session) }
}

unsafe extern "system" fn xr_initialize_loader_khr(
    loader_init_info: *const LoaderInitInfoBaseHeaderKHR,
) -> Result {
    unsafe { LAYER.initalize_loader_khr(loader_init_info) }
}

unsafe extern "system" fn xr_create_swapchain(
    session: Session,
    create_info: *const SwapchainCreateInfo,
    swapchain: *mut Swapchain,
) -> Result {
    unsafe { LAYER.create_swapchain(session, create_info, swapchain) }
}

unsafe extern "system" fn xr_begin_frame(
    session: Session,
    frame_begin_info: *const FrameBeginInfo,
) -> Result {
    unsafe { LAYER.begin_frame(session, frame_begin_info) }
}

unsafe extern "system" fn xr_end_frame(
    session: Session,
    frame_end_info: *const FrameEndInfo,
) -> Result {
    unsafe { LAYER.end_frame(session, frame_end_info) }
}

/*
unsafe extern "system" fn xr_enumerate_instance_extension_properties(
    layer_name: *const c_char,
    property_capacity_input: u32,
    property_count_output: *mut u32,
    properties: *mut ExtensionProperties,
) -> Result {
    INSTANCE.enumerate_instance_extension_properties(
        layer_name,
        property_capacity_input,
        property_count_output,
        properties,
    )
}

unsafe extern "system" fn xr_get_system_properties(
    instance: Instance,
    system_id: SystemId,
    properties: *mut SystemProperties,
) -> Result {
    INSTANCE.get_system_properties(instance, system_id, properties)
}

unsafe extern "system" fn xr_suggest_interaction_profile_bindings(
    instance: Instance,
    suggested_bindings: *const InteractionProfileSuggestedBinding,
) -> Result {
    INSTANCE.suggest_interaction_profile_bindings(instance, suggested_bindings)
}

unsafe extern "system" fn xr_create_action_space(
    session: Session,
    create_info: *const ActionSpaceCreateInfo,
    space: *mut Space,
) -> Result {
    INSTANCE.create_action_space(session, create_info, space)
}

unsafe extern "system" fn xr_get_action_state_pose(
    session: Session,
    get_info: *const ActionStateGetInfo,
    state: *mut ActionStatePose,
) -> Result {
    INSTANCE.get_action_state_pose(session, get_info, state)
}

unsafe extern "system" fn xr_locate_space(
    space: Space,
    base_space: Space,
    time: Time,
    location: *mut SpaceLocation,
) -> Result {
    INSTANCE.locate_space(space, base_space, time, location)
}

unsafe extern "system" fn xr_locate_views(
    session: Session,
    view_locate_info: *const ViewLocateInfo,
    view_state: *mut ViewState,
    view_capacity_input: u32,
    view_count_output: *mut u32,
    views: *mut View,
) -> Result {
    INSTANCE.locate_views(
        session,
        view_locate_info,
        view_state,
        view_capacity_input,
        view_count_output,
        views,
    )
}
*/
