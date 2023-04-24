extern crate glob;

use std::process::Command;
use std::env;

use glob::{glob, Paths};

fn compile_shaders(paths: Paths, out_dir: &str) {
    for entry in paths {
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
                // Command::new("glslc").args(&[shader_path.to_str().unwrap(), "--target-env=vulkan1.1", "-o"])
                Command::new("glslangValidator").args(&[shader_path.to_str().unwrap(), "--target-env", "vulkan1.1", "-o"])
                    .arg(&format!("{}/shaders/{}-{}.spv", out_dir, shader_name, shader_ext))
                    .status()
                    .expect("Error compiling shader");
            },
            Err(e) => {
                println!("Failed to compile shaders");
                panic!("{}", e);
            }
        }
    }
}

fn main() {
    // println!("cargo:rerun-if-changed=shaders");
    println!("Compiling shaders");
    //let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir = std::env::current_dir().expect("Couldn't get current directory");

    std::fs::create_dir_all(&format!("{}/shaders", out_dir.display()))
        .expect("Failed to create shader output directory");
    println!("Placing shaders at {}", out_dir.display());

    let vert_shaders = glob("shaders/*.vert").expect("No vert shaders found");
    let frag_shaders = glob("shaders/*.frag").expect("No frag shaders found");

    let pass_vert_shaders = glob("../passes/shaders/*.vert")
        .expect("No pass vert shaders");
    let pass_frag_shaders = glob("../passes/shaders/*frag")
        .expect("No pass frag shaders");

    let out_dir_str = out_dir.display().to_string();
    compile_shaders(vert_shaders, &out_dir_str);
    compile_shaders(frag_shaders, &out_dir_str);
    compile_shaders(pass_vert_shaders, &out_dir_str);
    compile_shaders(pass_frag_shaders, &out_dir_str);
}