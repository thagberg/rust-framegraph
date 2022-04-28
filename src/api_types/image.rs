use ash::vk;

pub struct ImageWrapper {
    pub image: vk::Image,
    pub view: Option<vk::ImageView>
}

impl ImageWrapper {
    pub fn new(image: vk::Image) -> ImageWrapper {
        ImageWrapper {
            image,
            view: None
        }
    }

    pub fn get(&self) -> vk::Image { self.image }
    pub fn get_view(&self) -> Option<vk::ImageView> { self.view }
}