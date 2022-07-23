pub struct InstanceWrapper {
    instance: ash::Instance
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