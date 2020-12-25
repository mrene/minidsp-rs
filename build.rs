// pro example: https://github.com/rust-lang/git2-rs/blob/master/libgit2-sys/build.rs
// https://github.com/r-darwish/docker-libcec-rpi/blob/master/Dockerfile
use copy_dir::copy_dir;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const P8_PLATFORM_DIR_ENV: &str = "p8-platform_DIR";
const LIBCEC_BUILD: &str = "libcec_build";
const PLATFORM_BUILD: &str = "platform_build";
const LIBCEC_SRC: &str = "vendor";

fn prepare_build(dst: &Path) {
    let dst_src = dst.join(LIBCEC_SRC);
    copy_dir(LIBCEC_SRC, &dst_src).unwrap();
}

fn compile_platform(dst: &Path) {
    let platform_build = dst.join(PLATFORM_BUILD);
    // let tmp_libcec_src = dst.join(LIBCEC_SRC);
    fs::create_dir_all(&platform_build).unwrap();
    cmake::Config::new(dst.join(LIBCEC_SRC).join("src").join("platform"))
        .out_dir(&platform_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .build();
    Command::new("make")
        .current_dir(&platform_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .status()
        .expect("failed to make libcec platform!");
}

fn compile_libcec(dst: &Path) {
    let platform_build = dst.join(PLATFORM_BUILD);
    let libcec_build = dst.join(LIBCEC_BUILD);
    fs::create_dir_all(&libcec_build).unwrap();
    cmake::Config::new(&dst.join(LIBCEC_SRC))
        .out_dir(&libcec_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .build();

    Command::new("make")
        .current_dir(&libcec_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .status()
        .expect("failed to make libcec!");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let cmakelists = format!("{}/CMakeLists.txt", LIBCEC_SRC);
    let libcec_git_dir = Path::new(&cmakelists);
    if !libcec_git_dir.exists() {
        panic!(
            "git submodules (tested {}, working dir {}) are not properly initialized! Aborting.",
            libcec_git_dir.display(),
            env::current_dir()
                .expect("Unknown working directory")
                .display()
        )
    }
    let dst = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    println!("Building libcec from local source");
    prepare_build(&dst);
    compile_platform(&dst);
    compile_libcec(&dst);
    println!(
        "cargo:rustc-link-search=native={}",
        dst.join(LIBCEC_BUILD).join("lib").display()
    );
    println!("cargo:rustc-link-lib=cec");
}
