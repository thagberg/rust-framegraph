use ash::vk;

#[derive(Clone)]
pub struct ImageWrapper {
    pub image: vk::Image,
    pub view: vk::ImageView
}

impl ImageWrapper {
    pub fn new(image: vk::Image, view: vk::ImageView) -> ImageWrapper {
        ImageWrapper {
            image,
            view: view
        }
    }

    pub fn get(&self) -> vk::Image { self.image }
    pub fn get_view(&self) -> vk::ImageView { self.view }
}