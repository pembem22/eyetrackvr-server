use std::ffi::CStr;
use std::ffi::c_char;

use crate::openxr_layer::layer::INSTANCE;

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
) -> Result { unsafe {
    println!("--> xr_create_api_layer_instance");

    // Call the chain to create the instance.
    let mut chain_instance_create_info = *instance_create_info_ptr;

    // Hide our extension from the list assuming it's in the beginning.
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

    if result == Result::SUCCESS {
        // Create our layer.
        INSTANCE.get_instance_proc_addr =
            Some((*api_layer_info.next_info).next_get_instance_proc_addr);
        INSTANCE.instance = Some(*instance);
    }

    println!("<-- xr_create_api_layer_instance");

    result
}}

pub unsafe extern "system" fn xr_get_instance_proc_addr(
    instance: Instance,
    name_ptr: *const c_char,
    function: *mut Option<pfn::VoidFunction>,
) -> Result { unsafe {
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

    let result = INSTANCE.get_instance_proc_addr.unwrap()(instance, name_ptr, function);

    /*
    if api_name == "xrEnumerateInstanceExtensionProperties" {
        INSTANCE.enumerate_instance_extensions_properties = Some(std::mem::transmute::<
            pfn::VoidFunction,
            pfn::EnumerateInstanceExtensionProperties,
        >((*function).unwrap()));
        *function = Some(std::mem::transmute::<
            pfn::EnumerateInstanceExtensionProperties,
            pfn::VoidFunction,
        >(xr_enumerate_instance_extension_properties));
    }

    if api_name == "xrGetSystemProperties" {
        INSTANCE.get_system_properties = Some(std::mem::transmute::<
            pfn::VoidFunction,
            pfn::GetSystemProperties,
        >((*function).unwrap()));
        *function = Some(std::mem::transmute::<
            pfn::GetSystemProperties,
            pfn::VoidFunction,
        >(xr_get_system_properties));
    }

    if api_name == "xrSuggestInteractionProfileBindings" {
        INSTANCE.suggest_interaction_profile_bindings = Some(std::mem::transmute::<
            pfn::VoidFunction,
            pfn::SuggestInteractionProfileBindings,
        >((*function).unwrap()));
        *function = Some(std::mem::transmute::<
            pfn::SuggestInteractionProfileBindings,
            pfn::VoidFunction,
        >(xr_suggest_interaction_profile_bindings));
    }

    if api_name == "xrCreateActionSpace" {
        INSTANCE.create_action_space = Some(std::mem::transmute::<
            pfn::VoidFunction,
            pfn::CreateActionSpace,
        >((*function).unwrap()));
        *function = Some(std::mem::transmute::<
            pfn::CreateActionSpace,
            pfn::VoidFunction,
        >(xr_create_action_space));
    }

    if api_name == "xrGetActionStatePose" {
        INSTANCE.get_action_state_pose = Some(std::mem::transmute::<
            pfn::VoidFunction,
            pfn::GetActionStatePose,
        >((*function).unwrap()));
        *function = Some(std::mem::transmute::<
            pfn::GetActionStatePose,
            pfn::VoidFunction,
        >(xr_get_action_state_pose));
    }

    if api_name == "xrLocateSpace" {
        INSTANCE.locate_space = Some(std::mem::transmute::<pfn::VoidFunction, pfn::LocateSpace>(
            (*function).unwrap(),
        ));
        *function = Some(std::mem::transmute::<pfn::LocateSpace, pfn::VoidFunction>(
            xr_locate_space,
        ));
    }

    if api_name == "xrLocateViews" {
        INSTANCE.locate_views = Some(std::mem::transmute::<pfn::VoidFunction, pfn::LocateViews>(
            (*function).unwrap(),
        ));
        *function = Some(std::mem::transmute::<pfn::LocateViews, pfn::VoidFunction>(
            xr_locate_views,
        ));
    }
    */

    if api_name == "xrPathToString" {
        INSTANCE.path_to_string = Some(
            std::mem::transmute::<pfn::VoidFunction, pfn::PathToString>((*function).unwrap()),
        );
    }

    result
}}

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
