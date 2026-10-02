use anyhow::{Error, Result};

use ash::{khr, prelude::VkResult, vk};
use glam::*;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{ffi::CStr, mem::offset_of, sync::Arc};

use crate::buffer::{MappedBuffer, MappedVec};

const API_VERSION: u32 = vk::API_VERSION_1_3;
type Index = u16;
pub type PerFrameInFlight<T> = Vec<T>;
type PerSwapchainImage<T> = Vec<T>;

const INDEX_BUFFERS_SIZE: usize = 1 << 8;
const VERTEX_BUFFERS_SIZE: usize = 1 << 8;
const INSTANCE_BUFFERS_SIZE: usize = 1 << 8;

pub struct Renderer<W>
where
    W: HasDisplayHandle + HasWindowHandle + Send + Sync,
{
    _window: Arc<W>,
    _entry: ash::Entry,
    instance: ash::Instance,
    khr_instance: khr::surface::Instance,
    khr_device: khr::swapchain::Device,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,

    graphics_queue: vk::Queue,
    _graphics_queue_index: u32,

    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_format: vk::SurfaceFormatKHR,
    swapchain_extent: vk::Extent2D,
    swapchain_image_views: Vec<vk::ImageView>,

    graphics_pipeline_layout: vk::PipelineLayout,
    graphics_pipeline: vk::Pipeline,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_sets: Vec<vk::DescriptorSet>,

    command_pool: vk::CommandPool,
    command_buffer: PerFrameInFlight<vk::CommandBuffer>,

    present_complete_semaphore: PerFrameInFlight<vk::Semaphore>,
    render_finished_semaphores: PerSwapchainImage<vk::Semaphore>,
    draw_fence: PerFrameInFlight<vk::Fence>,

    current_frame: usize,

    vertices: Vec<Vertex>,
    vertex_buffers: PerFrameInFlight<MappedVec<Vertex>>,

    instances: Vec<Instance>,
    instance_buffers: PerFrameInFlight<MappedVec<Instance>>,

    indices: Vec<u16>,
    index_buffers: PerFrameInFlight<MappedVec<Index>>,

    uniform_buffers: PerFrameInFlight<MappedBuffer<UniformBuffer>>,

    meshes: Vec<Mesh>,
    camera: Camera,
}

unsafe fn create_instance(
    entry: &ash::Entry,
    validation_layers: bool,
    required_extensions: &[*const i8],
) -> Result<ash::Instance> {
    let app_info = vk::ApplicationInfo {
        api_version: API_VERSION,
        p_engine_name: c"Worse Graphics".as_ptr(),
        p_application_name: c"Hello !".as_ptr(),
        application_version: 0,
        ..Default::default()
    };

    let available_extensions = unsafe { entry.enumerate_instance_extension_properties(None) }?;
    for ext in required_extensions {
        let ext = unsafe { CStr::from_ptr(*ext) };
        available_extensions
            .iter()
            .find(|&ext_prop| {
                ext_prop
                    .extension_name_as_c_str()
                    .is_ok_and(|available_ext| available_ext == ext)
            })
            .is_some()
            .ok_or(Error::msg(format!(
                "Required extension {} is not available",
                ext.to_string_lossy()
            )))?;
    }

    let mut enabled_layers = vec![];

    if validation_layers {
        let layer_name = c"VK_LAYER_KHRONOS_validation";
        let layers = unsafe { entry.enumerate_instance_layer_properties() }?;
        layers
            .iter()
            .find(|layer| layer.layer_name_as_c_str().is_ok_and(|p| p == layer_name))
            .ok_or(Error::msg(format!(
                "The layer {} is not available",
                layer_name.to_string_lossy()
            )))?;
        enabled_layers.push(layer_name.as_ptr());
        #[cfg(debug_assertions)]
        println!(
            "Enabled layers : {:?}",
            enabled_layers
                .iter()
                .map(|l| unsafe { CStr::from_ptr(*l) }.to_string_lossy())
                .collect::<Vec<_>>()
        )
    }

    let create_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_layer_names(&enabled_layers)
        .enabled_extension_names(required_extensions);
    Ok(unsafe { entry.create_instance(&create_info, None)? })
}

unsafe fn is_device_suitable(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    required_extensions: &[&CStr],
) -> bool {
    let properties = unsafe { instance.get_physical_device_properties(physical_device) };
    let family_properties =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let supports_api_version = properties.api_version >= API_VERSION;
    let supports_graphics = family_properties
        .iter()
        .find(|qf| qf.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .is_some();
    let available_extensions =
        if let Ok(a) = unsafe { instance.enumerate_device_extension_properties(physical_device) } {
            a
        } else {
            return false;
        };
    let support_required_extensions = required_extensions.iter().all(|&req_ext_name| {
        available_extensions
            .iter()
            .find(|&ext| {
                let mut ext_name = [0i8; vk::MAX_EXTENSION_NAME_SIZE + 1];
                ext_name[0..vk::MAX_EXTENSION_NAME_SIZE].copy_from_slice(&ext.extension_name);
                let ext_name = unsafe { CStr::from_ptr(ext_name.as_ptr()) };
                req_ext_name == ext_name
            })
            .is_some()
    });

    let mut vulkan_ext = vk::PhysicalDeviceExtendedDynamicStateFeaturesEXT::default();
    let mut vulkan_13_features = vk::PhysicalDeviceVulkan13Features::default();
    let mut vulkan_11_features = vk::PhysicalDeviceVulkan11Features::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut vulkan_ext)
        .push_next(&mut vulkan_13_features)
        .push_next(&mut vulkan_11_features);
    unsafe {
        instance.get_physical_device_features2(physical_device, &mut features2);
    };
    let support_required_features = vulkan_11_features.shader_draw_parameters != 0
        && vulkan_13_features.dynamic_rendering != 0
        && vulkan_13_features.synchronization2 != 0
        && vulkan_ext.extended_dynamic_state != 0;

    let a = supports_api_version
        && supports_graphics
        && support_required_extensions
        && support_required_features;
    if !a {
        dbg!(&properties.device_name_as_c_str());
        dbg!(&support_required_extensions);
    }
    a
}

unsafe fn evaluate_physical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> u32 {
    let properties = unsafe { instance.get_physical_device_properties(physical_device) };

    match properties.device_type {
        vk::PhysicalDeviceType::DISCRETE_GPU => 4,
        vk::PhysicalDeviceType::INTEGRATED_GPU => 3,
        vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
        vk::PhysicalDeviceType::CPU => 1,
        vk::PhysicalDeviceType::OTHER => 0,
        _ => 0,
    }
}

unsafe fn pick_physical_device(
    instance: &ash::Instance,
    required_extensions: &[&CStr],
) -> Result<vk::PhysicalDevice> {
    let physical_devices = unsafe { instance.enumerate_physical_devices() }?;
    physical_devices
        .into_iter()
        .filter(|d| unsafe { is_device_suitable(instance, *d, required_extensions) })
        .max_by_key(|d| unsafe { evaluate_physical_device(instance, *d) })
        .ok_or(Error::msg("Failed to find a physical device"))
}

unsafe fn create_logical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device_extensions: &[&CStr],
    khr_instance: &khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<ash::Device> {
    let graphics_queue_index =
        unsafe { get_graphics_queue_index(instance, physical_device, khr_instance, surface) }
            .ok_or(Error::msg(
                "Failed to find a valid queue on the physical device",
            ))?;
    let extensions = device_extensions
        .iter()
        .map(|e| e.as_ptr())
        .collect::<Vec<_>>();
    let mut features_ext =
        vk::PhysicalDeviceExtendedDynamicStateFeaturesEXT::default().extended_dynamic_state(true);
    let mut features13 = vk::PhysicalDeviceVulkan13Features::default()
        .dynamic_rendering(true)
        .synchronization2(true);
    let mut features11 = vk::PhysicalDeviceVulkan11Features::default().shader_draw_parameters(true);
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut features_ext)
        .push_next(&mut features13)
        .push_next(&mut features11);
    let queue_create_info = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(graphics_queue_index)
        .queue_priorities(&[0.5f32])];
    let create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_create_info)
        .enabled_extension_names(&extensions)
        .push_next(&mut features2);
    Ok(unsafe { instance.create_device(physical_device, &create_info, None) }?)
}

unsafe fn get_graphics_queue_index(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    khr_instance: &khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> Option<u32> {
    let queue_family_properties =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
    let graphics_queue_index = queue_family_properties
        .iter()
        .enumerate()
        .find(|&(i, q)| {
            let supports_graphics = q.queue_flags.contains(vk::QueueFlags::GRAPHICS);
            let supports_surface_khr = unsafe {
                khr_instance.get_physical_device_surface_support(physical_device, i as u32, surface)
            }
            .unwrap_or(false);
            supports_graphics && supports_surface_khr
        })
        .map(|(i, _)| i as u32)?;
    Some(graphics_queue_index)
}

struct SwapchainCreation {
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_format: vk::SurfaceFormatKHR,
    swapchain_extent: vk::Extent2D,
}

unsafe fn create_swapchain(
    physical_device: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    khr_instance: &khr::surface::Instance,
    khr_device: &khr::swapchain::Device,
    recomended_extent: vk::Extent2D,
) -> Result<SwapchainCreation> {
    let available_formats =
        unsafe { khr_instance.get_physical_device_surface_formats(physical_device, surface) }?;

    let surface_format = available_formats
        .into_iter()
        .max_by_key(|f| match f {
            vk::SurfaceFormatKHR {
                format: vk::Format::B8G8R8A8_SRGB,
                color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            } => 1,
            _ => 0,
        })
        .ok_or(Error::msg("There are no surface format available"))?;
    let available_present_modes = unsafe {
        khr_instance.get_physical_device_surface_present_modes(physical_device, surface)
    }?;
    let present_mode = available_present_modes
        .into_iter()
        .max_by_key(|&p| match p {
            vk::PresentModeKHR::MAILBOX => 1,
            vk::PresentModeKHR::FIFO => 2,
            _ => 0,
        })
        .ok_or(Error::msg("There are no presentation modes available"))?;

    let surface_capabilities_khr =
        unsafe { khr_instance.get_physical_device_surface_capabilities(physical_device, surface) }?;
    let extent = vk::Extent2D {
        width: recomended_extent.width.clamp(
            surface_capabilities_khr.min_image_extent.width,
            surface_capabilities_khr.max_image_extent.width,
        ),
        height: recomended_extent.height.clamp(
            surface_capabilities_khr.min_image_extent.height,
            surface_capabilities_khr.max_image_extent.height,
        ),
    };

    let mut min_image_count = 3.max(surface_capabilities_khr.min_image_count);
    if surface_capabilities_khr.max_image_count > 0 {
        min_image_count = min_image_count.max(surface_capabilities_khr.max_image_count);
    }

    let create_info = vk::SwapchainCreateInfoKHR {
        surface,
        min_image_count,
        image_format: surface_format.format,
        image_color_space: surface_format.color_space,
        image_extent: extent,
        image_array_layers: 1,
        image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        image_sharing_mode: vk::SharingMode::EXCLUSIVE,
        pre_transform: surface_capabilities_khr.current_transform,
        composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
        present_mode,
        clipped: vk::TRUE,
        ..Default::default()
    };

    let swapchain = unsafe { khr_device.create_swapchain(&create_info, None) }?;
    let images = unsafe { khr_device.get_swapchain_images(swapchain) }?;

    Ok(SwapchainCreation {
        swapchain,
        swapchain_images: images,
        swapchain_format: surface_format,
        swapchain_extent: extent,
    })
}

unsafe fn create_swapchain_image_views(
    swapchain_format: vk::SurfaceFormatKHR,
    swapchain_images: &[vk::Image],
    device: &ash::Device,
) -> Result<Vec<vk::ImageView>> {
    let mut create_info = vk::ImageViewCreateInfo {
        view_type: vk::ImageViewType::TYPE_2D,
        format: swapchain_format.format,
        subresource_range: vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        },
        ..Default::default()
    };

    Ok(swapchain_images
        .iter()
        .map(|image| {
            create_info.image = *image;
            unsafe { device.create_image_view(&create_info, None) }
        })
        .collect::<VkResult<_>>()?)
}

fn get_shader_code() -> &'static [u32] {
    const BYTES: &[u8] = include_bytes!("../target/shader.spv");
    const N: usize = BYTES.len();
    const _: () = assert!(N.is_multiple_of(4));

    #[repr(align(4))]
    struct Aligned([u8; N]);

    static ALIGNED: Aligned = Aligned(*include_bytes!("../target/shader.spv"));

    assert!(size_of_val(&ALIGNED).is_multiple_of(4));

    unsafe { std::slice::from_raw_parts(ALIGNED.0.as_ptr() as *const u32, N / 4) }
}

unsafe fn create_descriptor_pool(
    device: &ash::Device,
    frames_in_flight: u32,
) -> Result<vk::DescriptorPool> {
    let pool_sizes = [vk::DescriptorPoolSize {
        ty: vk::DescriptorType::UNIFORM_BUFFER,
        descriptor_count: frames_in_flight,
    }];
    let pool_create_info = vk::DescriptorPoolCreateInfo::default()
        .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
        .max_sets(frames_in_flight)
        .pool_sizes(&pool_sizes);
    Ok(unsafe { device.create_descriptor_pool(&pool_create_info, None) }?)
}

unsafe fn create_descriptor_sets(
    device: &ash::Device,
    uniform_buffers: &[MappedBuffer<UniformBuffer>],
    descriptor_set_layouts: &[vk::DescriptorSetLayout],
    descriptor_pool: vk::DescriptorPool,
) -> Result<Vec<vk::DescriptorSet>> {
    let alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(descriptor_set_layouts);
    let sets = unsafe { device.allocate_descriptor_sets(&alloc_info) }?;

    for (b, set) in uniform_buffers.iter().zip(&sets) {
        let buffer_info = [vk::DescriptorBufferInfo {
            buffer: b.get_raw_buffer(),
            offset: 0,
            range: size_of::<UniformBuffer>() as u64,
        }];
        let write = [vk::WriteDescriptorSet::default()
            .buffer_info(&buffer_info)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_count(1)
            .dst_set(*set)];
        unsafe { device.update_descriptor_sets(&write, &[]) };
    }

    Ok(sets)
}

unsafe fn create_graphics_pipeline(
    device: &ash::Device,
    swapchain_format: vk::SurfaceFormatKHR,
) -> Result<(vk::Pipeline, vk::PipelineLayout, vk::DescriptorSetLayout)> {
    let shader_code = get_shader_code();
    let create_info = vk::ShaderModuleCreateInfo::default().code(shader_code);

    let module = unsafe { device.create_shader_module(&create_info, None) }?;

    let vertex_stage_create_info = vk::PipelineShaderStageCreateInfo {
        stage: vk::ShaderStageFlags::VERTEX,
        module,
        p_name: c"vertMain".as_ptr(),
        ..Default::default()
    };
    let fragment_stage_create_info = vk::PipelineShaderStageCreateInfo {
        stage: vk::ShaderStageFlags::FRAGMENT,
        module,
        p_name: c"fragMain".as_ptr(),
        ..Default::default()
    };

    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_states_create_info =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let shader_stages = [vertex_stage_create_info, fragment_stage_create_info];

    let binding_description = [
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        },
        vk::VertexInputBindingDescription {
            binding: 1,
            stride: size_of::<Instance>() as u32,
            input_rate: vk::VertexInputRate::INSTANCE,
        },
    ];
    let attributes_descriptions = [
        vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: offset_of!(Vertex, pos) as u32,
        },
        vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: offset_of!(Vertex, color) as u32,
        },
        vk::VertexInputAttributeDescription {
            location: 2,
            binding: 1,
            format: vk::Format::R32G32_SFLOAT,
            offset: offset_of!(Instance, pos) as u32,
        },
    ];

    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&binding_description)
        .vertex_attribute_descriptions(&attributes_descriptions);

    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);

    let rasterizer = vk::PipelineRasterizationStateCreateInfo {
        depth_clamp_enable: vk::FALSE,
        rasterizer_discard_enable: vk::FALSE,
        polygon_mode: vk::PolygonMode::FILL,
        cull_mode: vk::CullModeFlags::BACK,
        front_face: vk::FrontFace::COUNTER_CLOCKWISE,
        depth_bias_enable: vk::FALSE,
        line_width: 1f32,
        ..Default::default()
    };

    let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
        .sample_shading_enable(false);

    let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(false)
        .color_write_mask(vk::ColorComponentFlags::RGBA);

    let attachements = [color_blend_attachment];
    let color_blend = vk::PipelineColorBlendStateCreateInfo::default()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(&attachements);

    let descriptor_set_layout = [unsafe { create_descriptor_set_layout(device) }?];
    let pipeline_layout_create_info =
        vk::PipelineLayoutCreateInfo::default().set_layouts(&descriptor_set_layout);
    let pipeline_layout =
        unsafe { device.create_pipeline_layout(&pipeline_layout_create_info, None) }?;

    let attachement_formats = [swapchain_format.format];
    let mut pipeline_rendering_create_info =
        vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&attachement_formats);

    let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&shader_stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterizer)
        .multisample_state(&multisampling)
        .color_blend_state(&color_blend)
        .dynamic_state(&dynamic_states_create_info)
        .layout(pipeline_layout)
        .push_next(&mut pipeline_rendering_create_info);

    let pipeline = unsafe {
        device.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_create_info], None)
    }
    .map_err(|(_, e)| Error::msg(format!("Failed to create pipeline : {}", e)))?[0];

    unsafe { device.destroy_shader_module(module, None) };
    Ok((pipeline, pipeline_layout, descriptor_set_layout[0]))
}

unsafe fn create_command_pool(
    device: &ash::Device,
    graphics_queue_index: u32,
) -> Result<vk::CommandPool> {
    let command_pool_info = vk::CommandPoolCreateInfo::default()
        .queue_family_index(graphics_queue_index)
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    Ok(unsafe { device.create_command_pool(&command_pool_info, None) }?)
}

#[allow(clippy::too_many_arguments)]
unsafe fn transition_image_layout(
    device: &ash::Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
    src_access_mask: vk::AccessFlags2,
    dst_access_mask: vk::AccessFlags2,
    src_stage_mask: vk::PipelineStageFlags2,
    dst_stage_mask: vk::PipelineStageFlags2,
) {
    let barrier = vk::ImageMemoryBarrier2 {
        src_stage_mask,
        src_access_mask,
        dst_stage_mask,
        dst_access_mask,
        old_layout,
        new_layout,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image,
        subresource_range: vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        },
        ..Default::default()
    };
    let barriers = [barrier];
    let dependency_info = vk::DependencyInfo::default().image_memory_barriers(&barriers);
    unsafe { device.cmd_pipeline_barrier2(command_buffer, &dependency_info) };
}

pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl From<winit::dpi::PhysicalSize<u32>> for WindowSize {
    fn from(value: winit::dpi::PhysicalSize<u32>) -> Self {
        Self {
            width: value.width,
            height: value.height,
        }
    }
}
impl From<&winit::dpi::PhysicalSize<u32>> for WindowSize {
    fn from(value: &winit::dpi::PhysicalSize<u32>) -> Self {
        Self {
            width: value.width,
            height: value.height,
        }
    }
}

impl From<WindowSize> for vk::Extent2D {
    fn from(value: WindowSize) -> Self {
        Self {
            width: value.width,
            height: value.height,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pos: Vec2,
    color: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pos: Vec2,
}

pub fn find_memory_type(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    type_filter: u32,
    properties: vk::MemoryPropertyFlags,
) -> Option<u32> {
    let mem_propreties = unsafe { instance.get_physical_device_memory_properties(physical_device) };
    for i in 0..mem_propreties.memory_type_count {
        let suitable_memory = type_filter & (1 << i) != 0;
        let suitable_properties = mem_propreties.memory_types[i as usize]
            .property_flags
            .contains(properties);
        if suitable_memory && suitable_properties {
            return Some(i);
        }
    }
    None
}

unsafe fn create_index_buffer(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    frames_in_flight: u32,
) -> Result<PerFrameInFlight<MappedVec<Index>>> {
    (0..frames_in_flight)
        .map(|_| unsafe {
            MappedVec::new(
                instance,
                physical_device,
                device,
                vk::BufferUsageFlags::INDEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                INDEX_BUFFERS_SIZE,
            )
        })
        .collect::<Result<_>>()
}

unsafe fn create_descriptor_set_layout(device: &ash::Device) -> Result<vk::DescriptorSetLayout> {
    let bindings = [vk::DescriptorSetLayoutBinding {
        binding: 0,
        descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
        descriptor_count: 1,
        stage_flags: vk::ShaderStageFlags::VERTEX,
        ..Default::default()
    }];
    let create_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
    Ok(unsafe { device.create_descriptor_set_layout(&create_info, None) }?)
}

#[repr(C)]
struct UniformBuffer {
    model: Mat4,
    view: Mat4,
    proj: Mat4,
}

unsafe fn create_uniform_buffers(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    frames_in_flight: u32,
) -> Result<PerFrameInFlight<MappedBuffer<UniformBuffer>>> {
    unsafe {
        MappedBuffer::new_frames_in_flight(
            instance,
            physical_device,
            device,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            frames_in_flight,
        )
    }
}

unsafe fn create_vertex_buffers(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    frames_in_flight: u32,
) -> Result<PerFrameInFlight<MappedVec<Vertex>>> {
    (0..frames_in_flight)
        .map(|_| unsafe {
            MappedVec::new(
                instance,
                physical_device,
                device,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                VERTEX_BUFFERS_SIZE,
            )
        })
        .collect::<Result<_>>()
}

unsafe fn create_instance_buffers(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    frames_in_flight: u32,
) -> Result<PerFrameInFlight<MappedVec<Instance>>> {
    (0..frames_in_flight)
        .map(|_| unsafe {
            MappedVec::new(
                instance,
                physical_device,
                device,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                INSTANCE_BUFFERS_SIZE,
            )
        })
        .collect::<Result<_>>()
}

impl<W> Renderer<W>
where
    W: HasDisplayHandle + HasWindowHandle + Send + Sync,
{
    pub fn new(
        window: Arc<W>,
        window_size: WindowSize,
        validation_layers: bool,
        frames_in_flight: u32,
    ) -> Result<Self> {
        let entry = unsafe { ash::Entry::load()? };
        let rdh = window.display_handle()?.as_raw();
        let rwh = window.window_handle()?.as_raw();
        let required_extensions = ash_window::enumerate_required_extensions(rdh)?;
        let instance = unsafe { create_instance(&entry, validation_layers, required_extensions)? };
        let khr_instance = khr::surface::Instance::new(&entry, &instance);
        let surface = unsafe { ash_window::create_surface(&entry, &instance, rdh, rwh, None)? };
        let device_extensions = [vk::KHR_SWAPCHAIN_NAME];
        let physical_device = unsafe { pick_physical_device(&instance, &device_extensions) }?;
        let properties = unsafe { instance.get_physical_device_properties(physical_device) };
        #[cfg(debug_assertions)]
        println!(
            "physical device : {}",
            properties.device_name_as_c_str()?.to_string_lossy()
        );
        let device = unsafe {
            create_logical_device(
                &instance,
                physical_device,
                &device_extensions,
                &khr_instance,
                surface,
            )
        }?;
        let graphics_queue_index = unsafe {
            get_graphics_queue_index(&instance, physical_device, &khr_instance, surface).ok_or(
                Error::msg("The queue is not available on this physical device"),
            )
        }?;
        let graphics_queue = unsafe { device.get_device_queue(graphics_queue_index, 0) };
        let khr_device = khr::swapchain::Device::new(&instance, &device);
        let SwapchainCreation {
            swapchain,
            swapchain_images,
            swapchain_format,
            swapchain_extent,
        } = unsafe {
            create_swapchain(
                physical_device,
                surface,
                &khr_instance,
                &khr_device,
                window_size.into(),
            )
        }?;

        let swapchain_image_views =
            unsafe { create_swapchain_image_views(swapchain_format, &swapchain_images, &device) }?;

        let (graphics_pipeline, graphics_pipeline_layout, descriptor_set_layout) =
            unsafe { create_graphics_pipeline(&device, swapchain_format) }?;

        // TODO : all the layouts are the same
        let descriptor_set_layouts: Vec<_> = (0..frames_in_flight)
            .map(|_| descriptor_set_layout)
            .collect();

        let descriptor_pool = unsafe { create_descriptor_pool(&device, frames_in_flight) }?;

        let uniform_buffers = unsafe {
            create_uniform_buffers(&instance, physical_device, &device, frames_in_flight)
        }?;

        let command_pool = unsafe { create_command_pool(&device, graphics_queue_index) }?;

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(frames_in_flight);
        let command_buffer = unsafe { device.allocate_command_buffers(&alloc_info) }?;

        let present_complete_semaphore = (0..frames_in_flight)
            .map(|_| unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) })
            .collect::<VkResult<_>>()?;
        let render_finished_semaphores = swapchain_images
            .iter()
            .map(|_| unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) })
            .collect::<VkResult<_>>()?;
        let draw_fence = (0..frames_in_flight)
            .map(|_| unsafe {
                device.create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )
            })
            .collect::<VkResult<_>>()?;

        let vertex_buffers = unsafe {
            create_vertex_buffers(&instance, physical_device, &device, frames_in_flight)
        }?;

        let instance_buffers = unsafe {
            create_instance_buffers(&instance, physical_device, &device, frames_in_flight)
        }?;

        let index_buffers =
            unsafe { create_index_buffer(&instance, physical_device, &device, frames_in_flight) }?;

        let descriptor_sets = unsafe {
            create_descriptor_sets(
                &device,
                &uniform_buffers,
                &descriptor_set_layouts,
                descriptor_pool,
            )
        }?;

        let vertices = vec![
            Vertex {
                pos: [-0.5, -0.5].into(),
                color: [1.0, 0.0, 0.0].into(),
            },
            Vertex {
                pos: [0.5, -0.5].into(),
                color: [0.0, 1.0, 0.0].into(),
            },
            Vertex {
                pos: [0.5, 0.5].into(),
                color: [0.0, 0.0, 1.0].into(),
            },
            Vertex {
                pos: [-0.5, 0.5].into(),
                color: [1.0, 1.0, 1.0].into(),
            },
        ];
        let indices = Vec::new();
        let instances = Vec::new();
        let meshes = Vec::new();
        let camera = Camera::default();

        Ok(Self {
            _window: window,
            _entry: entry,
            instance,
            khr_instance,
            surface,
            physical_device,
            device,
            graphics_queue,
            _graphics_queue_index: graphics_queue_index,
            swapchain,
            khr_device,
            swapchain_images,
            swapchain_format,
            swapchain_extent,
            swapchain_image_views,
            graphics_pipeline_layout,
            graphics_pipeline,
            command_pool,
            command_buffer,
            present_complete_semaphore,
            render_finished_semaphores,
            draw_fence,
            current_frame: 0,
            vertices,
            vertex_buffers,
            instances,
            instance_buffers,
            indices,
            index_buffers,
            uniform_buffers,
            descriptor_set_layout,
            descriptor_pool,
            descriptor_sets,
            meshes,
            camera,
        })
    }

    unsafe fn record_command_buffer(&mut self, image_index: usize) -> Result<()> {
        let begin_info = vk::CommandBufferBeginInfo::default();
        let command_buffer = self.command_buffer[self.current_frame];
        unsafe {
            self.device
                .begin_command_buffer(command_buffer, &begin_info)?;

            transition_image_layout(
                &self.device,
                command_buffer,
                self.swapchain_images[image_index],
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::ATTACHMENT_OPTIMAL,
                vk::AccessFlags2::empty(),
                vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
                vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            );
        }

        let clear_color = vk::ClearColorValue {
            float32: [0f32, 0f32, 0f32, 1f32],
        };

        let attachment_info = [vk::RenderingAttachmentInfo {
            image_view: self.swapchain_image_views[image_index],
            image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            clear_value: vk::ClearValue { color: clear_color },
            ..Default::default()
        }];

        let rendering_info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.swapchain_extent,
            })
            .layer_count(1)
            .color_attachments(&attachment_info);

        let viewport = vk::Viewport {
            x: 0f32,
            y: 0f32,
            width: self.swapchain_extent.width as f32,
            height: self.swapchain_extent.height as f32,
            min_depth: 0f32,
            max_depth: 1f32,
        };
        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: self.swapchain_extent,
        };

        unsafe {
            self.device
                .cmd_begin_rendering(command_buffer, &rendering_info);

            self.device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.graphics_pipeline,
            );
            // update_uniform_buffer(&mut self.uniform_buffers[self.current_frame]);
            let data = self.uniform_buffers[self.current_frame].get_mut();

            *data = UniformBuffer {
                model: self.camera.model,
                view: self.camera.view,
                proj: self.camera.proj,
            };

            self.device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.graphics_pipeline_layout,
                0,
                &[self.descriptor_sets[self.current_frame]],
                &[],
            );

            let vertex_buffer = &mut self.vertex_buffers[self.current_frame];
            if vertex_buffer.capacity() >= self.vertices.len() {
                vertex_buffer.as_mut()[0..self.vertices.len()].copy_from_slice(&self.vertices);
            } else {
                eprintln!("The vertex limit {} was exceeded", VERTEX_BUFFERS_SIZE);
                let capacity = vertex_buffer.capacity();
                vertex_buffer
                    .as_mut()
                    .copy_from_slice(&self.vertices[0..capacity]);
            }

            let instance_buffer = &mut self.instance_buffers[self.current_frame];
            if instance_buffer.capacity() >= self.instances.len() {
                instance_buffer.as_mut()[0..self.instances.len()].copy_from_slice(&self.instances);
            } else {
                eprintln!("The instance limit {} was exceeded", INSTANCE_BUFFERS_SIZE);
                let capacity = instance_buffer.capacity();
                instance_buffer
                    .as_mut()
                    .copy_from_slice(&self.instances[0..capacity]);
            }

            let index_buffer = &mut self.index_buffers[self.current_frame];
            if index_buffer.capacity() >= self.indices.len() {
                index_buffer.as_mut()[0..self.indices.len()].copy_from_slice(&self.indices);
            } else {
                eprintln!("The index limit {} was exceeded", INDEX_BUFFERS_SIZE);
                let capacity = index_buffer.capacity();
                index_buffer
                    .as_mut()
                    .copy_from_slice(&self.indices[0..capacity]);
            }

            self.device.cmd_set_viewport(command_buffer, 0, &[viewport]);
            self.device.cmd_set_scissor(command_buffer, 0, &[scissor]);

            self.device.cmd_bind_vertex_buffers(
                command_buffer,
                0,
                &[
                    vertex_buffer.get_raw_buffer(),
                    instance_buffer.get_raw_buffer(),
                ],
                &[0, 0],
            );
            self.device.cmd_bind_index_buffer(
                command_buffer,
                index_buffer.get_raw_buffer(),
                0,
                vk::IndexType::UINT16,
            );

            for m in &self.meshes {
                self.device.cmd_draw_indexed(
                    command_buffer,
                    m.index_count,
                    m.instance_count,
                    m.first_index,
                    0,
                    m.first_instance,
                );
            }
            self.device.cmd_end_rendering(command_buffer);
        }

        unsafe {
            transition_image_layout(
                &self.device,
                command_buffer,
                self.swapchain_images[image_index],
                vk::ImageLayout::ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
                vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
                vk::AccessFlags2::empty(),
                vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
            );

            self.device.end_command_buffer(command_buffer)?;
        }

        Ok(())
    }

    fn draw_frame(&mut self) -> Result<()> {
        unsafe {
            self.device
                .wait_for_fences(&[self.draw_fence[self.current_frame]], true, u64::MAX)?;

            let (image_index, _) = match self.khr_device.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.present_complete_semaphore[self.current_frame],
                vk::Fence::null(),
            ) {
                Ok(value) => value,
                Err(err) => {
                    if err == vk::Result::ERROR_OUT_OF_DATE_KHR {
                        self.recreate_swapchain(self.swapchain_extent)?;
                        return Ok(());
                    } else {
                        return Err(err.into());
                    }
                }
            };

            self.device
                .reset_fences(&[self.draw_fence[self.current_frame]])?;

            let pcs = [self.present_complete_semaphore[self.current_frame]];
            let cb = [self.command_buffer[self.current_frame]];
            let rfs = [self.render_finished_semaphores[image_index as usize]];
            let wait_dst_storage_mask = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let mut submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&pcs)
                .signal_semaphores(&rfs)
                .wait_dst_stage_mask(&wait_dst_storage_mask);

            let record_command_buffer_result = self.record_command_buffer(image_index as usize);
            match record_command_buffer_result {
                Err(err) => {
                    eprint!("Error when recording the command buffer : {}", err);
                }
                Ok(_) => {
                    submit_info = submit_info.command_buffers(&cb);
                }
            }

            self.device.queue_submit(
                self.graphics_queue,
                &[submit_info],
                self.draw_fence[self.current_frame],
            )?;

            let swapchain = [self.swapchain];
            let image_indices = [image_index];
            let present_info_khr = vk::PresentInfoKHR::default()
                .wait_semaphores(&rfs)
                .swapchains(&swapchain)
                .image_indices(&image_indices);

            let suboptimal = self
                .khr_device
                .queue_present(self.graphics_queue, &present_info_khr)?;

            if suboptimal {
                self.recreate_swapchain(self.swapchain_extent)?;
            }

            self.current_frame = (self.current_frame + 1) % self.command_buffer.len();
        }
        Ok(())
    }

    unsafe fn cleanup_swapchain(&self) {
        unsafe {
            for image_view in &self.swapchain_image_views {
                self.device.destroy_image_view(*image_view, None);
            }
            self.khr_device.destroy_swapchain(self.swapchain, None);
        }
    }

    pub fn window_resized(&mut self, new_size: WindowSize) -> Result<()> {
        unsafe { self.recreate_swapchain(new_size.into()) }
    }

    unsafe fn recreate_swapchain(&mut self, new_extent: vk::Extent2D) -> Result<()> {
        unsafe {
            self.device.device_wait_idle()?;
            self.cleanup_swapchain();
            let SwapchainCreation {
                swapchain,
                swapchain_images,
                swapchain_format: _,
                swapchain_extent,
            } = create_swapchain(
                self.physical_device,
                self.surface,
                &self.khr_instance,
                &self.khr_device,
                new_extent,
            )?;

            (self.swapchain_images, self.swapchain_extent, self.swapchain) =
                (swapchain_images, swapchain_extent, swapchain);

            self.swapchain_image_views = create_swapchain_image_views(
                self.swapchain_format,
                &self.swapchain_images,
                &self.device,
            )?;
        }

        Ok(())
    }

    pub fn begin_drawing<'a>(&'a mut self) -> DrawingTicket<'a, W> {
        self.vertices.clear();
        self.instances.clear();
        self.indices.clear();
        self.meshes.clear();
        DrawingTicket(self)
    }
}

impl<W> Drop for Renderer<W>
where
    W: HasDisplayHandle + HasWindowHandle + Send + Sync,
{
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();

            if let Err(e) = self
                .device
                .free_descriptor_sets(self.descriptor_pool, &self.descriptor_sets)
            {
                println!("Failed to free descriptor sets : {}", e);
            }

            for buffer in &self.uniform_buffers {
                buffer.destroy_ressources(&self.device);
            }

            for index_buffer in &self.index_buffers {
                index_buffer.destroy_ressources(&self.device);
            }

            for instance_buffer in &self.instance_buffers {
                instance_buffer.destroy_ressources(&self.device);
            }

            for vertex_buffer in &self.vertex_buffers {
                vertex_buffer.destroy_ressources(&self.device);
            }

            for fence in &self.draw_fence {
                self.device.destroy_fence(*fence, None);
            }
            for s in &self.present_complete_semaphore {
                self.device.destroy_semaphore(*s, None);
            }
            for s in &self.render_finished_semaphores {
                self.device.destroy_semaphore(*s, None);
            }

            self.device
                .free_command_buffers(self.command_pool, &self.command_buffer);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device.destroy_pipeline(self.graphics_pipeline, None);

            self.device
                .destroy_pipeline_layout(self.graphics_pipeline_layout, None);

            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);

            self.cleanup_swapchain();
            self.device.destroy_device(None);
            self.khr_instance.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[allow(unused)]
pub struct Mesh {
    first_index: u32,
    index_count: u32,
    first_instance: u32,
    instance_count: u32,
}

pub struct DrawingTicket<'a, W>(&'a mut Renderer<W>)
where
    W: HasDisplayHandle + HasWindowHandle + Send + Sync;

pub struct Point {
    pub pos: [f32; 2],
    pub color: [f32; 3],
}

impl<'a, W> DrawingTicket<'a, W>
where
    W: HasDisplayHandle + HasWindowHandle + Send + Sync,
{
    pub fn end_drawing(self) -> Result<()> {
        self.0.draw_frame()
    }

    pub fn draw_triangle(&mut self, points: &[Point; 3], instances: &[Vec2]) {
        let first_index = self.0.indices.len() as u32;
        let index_count = 3;
        let instance_count = instances.len() as u32;
        let first_instance = self.0.instances.len() as u32;
        for Point { pos, color } in points {
            self.0.indices.push(self.0.vertices.len() as u16);
            self.0.vertices.push(Vertex {
                pos: (*pos).into(),
                color: (*color).into(),
            });
        }
        for instance in instances {
            self.0.instances.push(Instance { pos: *instance });
        }
        let mesh = Mesh {
            first_index,
            index_count,
            first_instance,
            instance_count,
        };
        self.0.meshes.push(mesh);
    }

    pub fn draw(&mut self, mesh: &Mesh) {
        self.0.meshes.push(*mesh);
    }

    pub fn get_camera(&mut self) -> &mut Camera {
        &mut self.0.camera
    }

    pub fn width(&self) -> u32 {
        self.0.swapchain_extent.width
    }

    pub fn height(&self) -> u32 {
        self.0.swapchain_extent.height
    }
}

#[derive(Default)]
pub struct Camera {
    pub model: Mat4,
    pub view: Mat4,
    pub proj: Mat4,
}
