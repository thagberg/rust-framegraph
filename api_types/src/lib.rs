pub mod surface;
pub mod device;
pub mod instance;
pub mod image;
pub mod buffer;
pub mod swapchain;
pub mod framebuffer;
pub mod shader;
pub mod pipeline;
pub mod renderpass;
pub mod handle;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
