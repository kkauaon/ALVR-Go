use alvr_common::lazy_static;
use futures::executor::block_on;
use wgpu::Adapter;

lazy_static! {
    static ref GPU_ADAPTERS: Vec<Adapter> = {
        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_desc.backends = wgpu::Backends::PRIMARY;

        let instance = wgpu::Instance::new(instance_desc);

        block_on(instance.enumerate_adapters(wgpu::Backends::PRIMARY))
    };
}

pub enum GpuVendor {
    Nvidia,
    Amd,
    Other,
}

pub fn get_gpu_vendor() -> GpuVendor {
    match GPU_ADAPTERS[0].get_info().vendor {
        0x10de => GpuVendor::Nvidia,
        0x1002 => GpuVendor::Amd,
        _ => GpuVendor::Other,
    }
}

pub fn get_gpu_names() -> Vec<String> {
    GPU_ADAPTERS
        .iter()
        .map(|a| a.get_info().name)
        .collect::<Vec<_>>()
}
