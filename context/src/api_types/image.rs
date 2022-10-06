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
    pub layout: vk::ImageLayout
}

impl ImageWrapper {
    pub fn new(image: vk::Image, view: vk::ImageView, layout: vk::ImageLayout) -> ImageWrapper {
        ImageWrapper {
            image,
            view,
            layout
        }
    }

    pub fn get(&self) -> vk::Image { self.image }
    pub fn get_view(&self) -> vk::ImageView { self.view }
}