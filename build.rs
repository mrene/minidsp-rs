// pro example: https://github.com/rust-lang/git2-rs/blob/master/libgit2-sys/build.rs
// https://github.com/r-darwish/docker-libcec-rpi/blob/master/Dockerfile
use fs_extra::dir::{copy, CopyOptions};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const P8_PLATFORM_DIR_ENV: &str = "p8-platform_DIR";

fn main() {
    if !Path::new("libcec/.git").exists() {
        panic!("git submodules are not properly initialized! Aborting.")
    }
    let mut copy_options: CopyOptions = CopyOptions::new();
    copy_options.overwrite = true;
    let dst = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let tmp_libcec_src = dst.join("libcec_src");
    let tmp_libcec = tmp_libcec_src.join("libcec");
    copy("libcec", &tmp_libcec, &copy_options).unwrap();
    let platform_build = dst.join("platform_build");
    fs::create_dir_all(&platform_build).unwrap();
    cmake::Config::new(tmp_libcec.join("src").join("platform"))
        .out_dir(&platform_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .build();
    Command::new("make")
        .current_dir(&platform_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .status()
        .expect("failed to make libcec platform!");

    let libcec_build = dst.join("libcec_build");
    fs::create_dir_all(&libcec_build).unwrap();
    cmake::Config::new(&tmp_libcec)
        .out_dir(&libcec_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .build();

    Command::new("make")
        .current_dir(&libcec_build)
        .env(P8_PLATFORM_DIR_ENV, &platform_build)
        .status()
        .expect("failed to make libcec!");
}
