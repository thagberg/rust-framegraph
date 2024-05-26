use glm;
#[derive(Clone)]
pub struct Camera {
    pub projection: glm::TMat4<f32>,
    pub view: glm::TMat4<f32>
}

impl Camera {
    pub fn new(
        aspect: f32,
        vertical_fov: f32,
        near: f32,
        far: f32,
        eye: &glm::TVec3<f32>,
        center: &glm::TVec3<f32>,
        up: &glm::TVec3<f32>) -> Self {

        Camera {
            projection: glm::perspective(
                aspect,
                vertical_fov,
                near,
                far
            ),

            view: glm::look_at(
                eye,
                center,
                up
            )
        }
    }

    pub fn new_from_view(
        aspect: f32,
        vertical_fov: f32,
        near: f32,
        far: f32,
        view: glm::TMat4<f32>) -> Self {

        Camera {
            projection: glm::perspective(
                aspect,
                vertical_fov,
                near,
                far
            ),
            view
        }
    }

    pub fn get_view(&self) -> glm::Mat4 {
        self.view.try_inverse().unwrap()
    }
}