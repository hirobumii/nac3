#![deny(clippy::all)]
#![warn(clippy::cargo, clippy::pedantic, clippy::nursery)]
#![allow(clippy::cargo_common_metadata)]

fn main() {
    #[cfg(not(windows))]
    println!("cargo:rustc-link-arg=-rdynamic");
}
