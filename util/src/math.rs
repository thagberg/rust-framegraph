use glm;

pub struct DecomposedMatrix {
    pub translation: glm::Vec3,
    pub rotation: glm::Mat4,
    pub scale: glm::Vec3
}

impl DecomposedMatrix {
    pub fn new(
        translation: glm::Vec3,
        rotation: glm::Mat4,
        scale: glm::Vec3
    ) -> Self {
        DecomposedMatrix {
            translation,
            rotation,
            scale
        }
    }

    pub fn resolve(&self) -> glm::Mat4 {
        // glm::translate(&glm::scale(&self.rotation, &self.scale), &self.translation)
        glm::translate(&glm::scale(&self.rotation, &self.scale), &self.translation)
    }
}