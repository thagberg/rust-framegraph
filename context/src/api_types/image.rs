use ash::vk;

pub struct ImageCreateInfo {
    create_info: vk::ImageCreateInfo,
    name: String
}

impl ImageCreateInfo {
    pub fn new(create_info: vk::ImageCreateInfo, name: String) -> Self {
        ImageCreateInfo {
            create_info,
            name
        }
    }

    pub fn get_create_info(&self) -> &vk::ImageCreateInfo {
        &self.create_info
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone)]
pub struct ImageWrapper {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub layout: vk::ImageLayout,
    pub extent: vk::Extent3D,
    pub sampler: Option<vk::Sampler>,
    pub is_swapchain_image: bool,
    pub format: vk::Format
}

impl ImageWrapper {
    pub fn new(
        image: vk::Image,
        view: vk::ImageView,
        layout: vk::ImageLayout,
        extent: vk::Extent3D,
        is_swapchain_image: bool,
        format: vk::Format,
        sampler: Option<vk::Sampler>) -> ImageWrapper {
        ImageWrapper {
            image,
            view,
            layout,
            extent,
            sampler,
            format,
            is_swapchain_image
        }
    }

    pub fn get(&self) -> vk::Image { self.image }
    pub fn get_view(&self) -> vk::ImageView { self.view }
    pub fn get_layout(&self) -> vk::ImageLayout { self.layout }
    pub fn get_sampler(&self) -> Option<vk::Sampler> { self.sampler }
}