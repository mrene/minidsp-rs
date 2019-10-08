// pro example: https://github.com/rust-lang/git2-rs/blob/master/libgit2-sys/build.rs
// https://github.com/r-darwish/docker-libcec-rpi/blob/master/Dockerfile
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const P8_PLATFORM_DIR_ENV: &str = "p8-platform_DIR";

fn main() {
    if !Path::new("libcec/.git").exists() {
        let _ = Command::new("git")
            .args(&["submodule", "update", "--init"])
            .status();
    }
    if !Path::new("libcec/src/platform/.git").exists() {
        let _ = Command::new("git")
            .current_dir(Path::new("libcec/"))
            .args(&["submodule", "update", "--init"])
            .status();
    }
    let dst = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let platform_build = dst.join("platform_build");
    fs::create_dir_all(&platform_build).unwrap();

    // RUN mkdir build && cd build &&
    // cmake ..
    // && make -j4
    cmake::Config::new("libcec/src/platform")
        .out_dir(&platform_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .build();
    Command::new("make")
        .current_dir(&platform_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .status()
        .expect("failed to make libcec platform!");

    // TODO: no install, need to add INCLUDE, LIB locations to laater make call

    let libcec_build = dst.join("libcec_build");
    fs::create_dir_all(&libcec_build).unwrap();
    cmake::Config::new("libcec/")
        .out_dir(&libcec_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .build();

    // -DRPI_INCLUDE_DIR=/opt/vc/include
    // -DRPI_LIB_DIR=/opt/vc/lib
    //  -DCMAKE_INSTALL_PREFIX=/opt/libcec
    // make -j4 && make install
    Command::new("make")
        .current_dir(&libcec_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .status()
        .expect("failed to make libcec!");
}
