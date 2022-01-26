extern crate glob;

use std::process::Command;
use std::env;
use std::path::Path;

use glob::glob;

fn main() {
    println!("cargo:rerun-if-changed=shaders");
    println!("Compiling shaders");
    let out_dir = env::var("OUT_DIR").unwrap();

    std::fs::create_dir_all(&format!("{}/shaders", out_dir));
    println!("Placing shaders at {}", out_dir);

    for entry in glob("shaders/*.vert")
        .unwrap()
        .chain(glob("shaders/*.frag").unwrap()) {
        println!("Found entry");
        match entry {
            Ok(shader_source) => {
                let shader_path = shader_source.as_path();
                let shader_name = shader_path.file_stem()
                    .expect("Unknown shader name")
                    .to_str().unwrap();
                let shader_ext = shader_path.extension()
                    .expect("Couldn't determine shader extension")
                    .to_str().unwrap();
                Command::new("glslc").args(&[shader_path.to_str().unwrap(), "-o"])
                    .arg(&format!("{}/shaders/{}-{}.spv", out_dir, shader_name, shader_ext))
                    .status()
                    .expect("Error compiling shader");
            },
            Err(e) => {}
        }
    }
}