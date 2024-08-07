pub mod camera;
pub mod math;
pub mod image;

extern crate nalgebra_glm as glm;

#[macro_export]
macro_rules! enter_span {
    ($level:expr, $name:expr, $($fields:tt)*) => {
        let span = tracing::span!($level, $name, $($fields)*);
        let _enter = span.enter();
    };

    ($level:expr, $name:expr) => {
        enter_span!($level, $name,)
    };
}

pub fn add(left: usize, right: usize) -> usize {
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
