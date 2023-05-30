pub mod blit;
pub mod imgui_draw;
pub mod blur;

extern crate imgui;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
