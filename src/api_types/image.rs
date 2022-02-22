use ash::vk;
use ash::vk::Image;

pub struct ImageWrapper {
    image: vk::Image
}

impl ImageWrapper {
    pub fn new(image: vk::Image) -> ImageWrapper {
        ImageWrapper {
            image
        }
    }

    pub fn get(&self) -> vk::Image { self.image }
}