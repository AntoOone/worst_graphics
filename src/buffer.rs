use anyhow::{Error, Result};
use ash::{self, vk};

use crate::{PerFrameInFlight, find_memory_type};

pub struct MappedBuffer<T> {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    data: std::ptr::NonNull<T>,
}

#[allow(unused)]
impl<T> MappedBuffer<T> {
    pub unsafe fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<Self> {
        let size = (size_of::<T>()) as u64;
        let (buffer, memory) =
            unsafe { create_buffer(instance, physical_device, device, size, usage, properties) }?;
        let data =
            unsafe { device.map_memory(memory, 0, size, vk::MemoryMapFlags::empty()) }?.cast();
        let data = std::ptr::NonNull::new(data).ok_or(Error::msg("Failed map data"))?;
        Ok(Self {
            buffer,
            memory,
            data,
        })
    }

    pub unsafe fn new_frames_in_flight(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
        frames_in_flight: u32,
    ) -> Result<PerFrameInFlight<Self>> {
        Ok((0..frames_in_flight)
            .map(|_| unsafe { Self::new(instance, physical_device, device, usage, properties) })
            .collect::<Result<_>>()?)
    }

    pub unsafe fn destroy_ressources(&self, device: &ash::Device) {
        unsafe {
            device.unmap_memory(self.memory);
            device.free_memory(self.memory, None);
            device.destroy_buffer(self.buffer, None);
        }
    }

    pub unsafe fn get_ref(&self) -> &T {
        unsafe { self.data.as_ref() }
    }

    pub unsafe fn get_mut(&mut self) -> &mut T {
        unsafe { self.data.as_mut() }
    }

    pub fn get_raw_buffer(&self) -> vk::Buffer {
        self.buffer
    }
}

pub struct MappedVec<T> {
    buffer: MappedBuffer<T>,
    capacity: usize,
}

#[allow(unused)]
impl<T> MappedVec<T> {
    pub unsafe fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
        capacity: usize,
    ) -> Result<Self> {
        let size = (capacity * size_of::<T>()) as u64;
        let (buffer, memory) =
            unsafe { create_buffer(instance, physical_device, device, size, usage, properties) }?;
        let data =
            unsafe { device.map_memory(memory, 0, size, vk::MemoryMapFlags::empty()) }?.cast();
        let data = std::ptr::NonNull::new(data).ok_or(Error::msg("Failed map data"))?;
        let buffer = MappedBuffer {
            buffer,
            memory,
            data,
        };
        Ok(Self { buffer, capacity })
    }

    pub unsafe fn destroy_ressources(&self, device: &ash::Device) {
        unsafe {
            self.buffer.destroy_ressources(device);
        }
    }

    pub unsafe fn get_ref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.buffer.data.as_ptr(), self.capacity) }
    }

    pub unsafe fn as_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.buffer.data.as_ptr(), self.capacity) }
    }

    pub fn get_raw_buffer(&self) -> vk::Buffer {
        self.buffer.get_raw_buffer()
    }
}

unsafe fn create_buffer(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    size: u64,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory)> {
    let buffer_info = vk::BufferCreateInfo {
        size,
        usage,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        ..Default::default()
    };
    let buffer = unsafe { device.create_buffer(&buffer_info, None) }?;
    let mem_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memory_allocate_info = vk::MemoryAllocateInfo {
        allocation_size: mem_requirements.size,
        memory_type_index: find_memory_type(
            &instance,
            physical_device,
            mem_requirements.memory_type_bits,
            properties,
        )
        .ok_or(Error::msg(
            "Failed to find a valid memory type for this memory allocation",
        ))?,
        ..Default::default()
    };

    let memory = unsafe { device.allocate_memory(&memory_allocate_info, None) }?;
    unsafe { device.bind_buffer_memory(buffer, memory, 0) }?;
    Ok((buffer, memory))
}
