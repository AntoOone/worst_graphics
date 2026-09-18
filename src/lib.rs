use anyhow::{Error, Result};

use ash::vk;
use std::ptr;

pub struct Renderer {
    entry: ash::Entry,
    pub instance: ash::Instance,
}

impl Renderer {
    pub fn new(validation_layers: bool) -> Result<Self> {
        let entry = unsafe { ash::Entry::load()? };

        let app_info = vk::ApplicationInfo {
            api_version: vk::API_VERSION_1_3,
            p_engine_name: c"Worse Graphics".as_ptr(),
            p_application_name: c"Hello !".as_ptr(),
            application_version: 0,
            ..Default::default()
        };

        let mut enabled_layers = vec![];
        if validation_layers {
            let layer_name = c"VK_LAYER_KHRONOS_validation";
            let layers = unsafe { entry.enumerate_instance_layer_properties() };
            layers?
                .iter()
                .filter(|layer| layer.layer_name_as_c_str().is_ok_and(|p| p == layer_name))
                .next()
                .ok_or(Error::msg(format!(
                    "The layer {} is not available",
                    layer_name.to_string_lossy()
                )))?;
            enabled_layers.push(layer_name.as_ptr());
        }
        dbg!(&enabled_layers);

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_layer_names(&enabled_layers);
        let instance = unsafe { entry.create_instance(&create_info, None)? };
        Ok(Self { entry, instance })
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}
