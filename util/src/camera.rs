use glm;
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

        let proj = glm::Mat4::from_columns(&[
            glm::Vec4::new(1.0 / (aspect * (0.5 * vertical_fov).tan()), 0.0, 0.0, 0.0),
            glm::Vec4::new(0.0, 1.0 / (0.5 * vertical_fov).tan(), 0.0, 0.0),
            glm::Vec4::new(0.0, 0.0, (far + near) / (near - far), ((2.0 * far * near))/ (near - far)),
            glm::Vec4::new(0.0, 0.0, -1.0, 0.0)
        ]).transpose();
        Camera {
            // projection: glm::perspective(
            //     aspect,
            //     vertical_fov,
            //     near,
            //     far
            // ),
            projection: proj,
            view
        }
    }

    pub fn get_view(&self) -> glm::Mat4 {
        self.view.try_inverse().unwrap()
    }
}