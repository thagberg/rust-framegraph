use std::fmt::{Debug, Formatter};

pub struct InstanceWrapper {
    instance: ash::Instance
}

impl Debug for InstanceWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceWrapper")
            .finish()
    }
}

impl Drop for InstanceWrapper {
    fn drop(&mut self) {
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}

impl InstanceWrapper {
    pub fn new(instance: ash::Instance) -> InstanceWrapper {
        InstanceWrapper {
            instance
        }
    }

    pub fn get(&self) -> &ash::Instance {
        &self.instance
    }
}