#![deny(clippy::all)]
#![warn(clippy::cargo, clippy::pedantic, clippy::nursery)]
#![allow(clippy::cargo_common_metadata, clippy::negative_feature_names)]

fn main() {
    lalrpop::process_root().unwrap();
}
