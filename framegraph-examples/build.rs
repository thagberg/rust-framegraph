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
                    .arg(&format!("{}/{}-{}.spv", out_dir, shader_name, shader_ext))
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

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=shaders");
    println!("cargo:rerun-if-changed='../passes/shaders'");
    println!("Compiling shaders");
    // let bin_dir = env::var("CARGO_BIN_EXE_".to_string() + &env::var("CARGO_BIN_NAME").expect("Couldn't get bin name")).expect(("Couldn't get bin directory"));
    // let out_dir = env::var("CARGO_BIN_EXE_".to_string() + &env::var("CARGO_BIN_NAME").expect("Couldn't get bin name")).expect(("Couldn't get bin directory"));
    // let out_dir = "target/".to_owned() + &env::var("PROFILE").expect("Couldn't get profile");
    // let out_dir = env::var("OUT_DIR").expect("Couldn't get output dir");
    let out_dir = "shaders/build";
    println!("Shader output directory: {}", out_dir);
    std::fs::create_dir_all(&out_dir)
        .expect("Failed to create shader output directory");
    println!("Placing shaders at {}", &out_dir);

    let vert_shaders = glob("shaders/*.vert").expect("No vert shaders found");
    let frag_shaders = glob("shaders/*.frag").expect("No frag shaders found");
    let compute_shaders = glob("shaders/*.comp").expect("No compute shaders found");

    let pass_vert_shaders = glob("../passes/shaders/*.vert")
        .expect("No pass vert shaders");
    let pass_frag_shaders = glob("../passes/shaders/*.frag")
        .expect("No pass frag shaders");
    let pass_compute_shaders = glob("../passes/shaders/*.comp")
        .expect("No pass compute shaders");

    compile_shaders(vert_shaders, &out_dir);
    compile_shaders(frag_shaders, &out_dir);
    compile_shaders(compute_shaders, &out_dir);
    compile_shaders(pass_vert_shaders, &out_dir);
    compile_shaders(pass_frag_shaders, &out_dir);
    compile_shaders(pass_compute_shaders, &out_dir);
}
