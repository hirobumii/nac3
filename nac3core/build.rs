#![deny(clippy::all)]
#![warn(clippy::cargo, clippy::pedantic, clippy::nursery)]
#![allow(clippy::cargo_common_metadata)]

use std::{
    env,
    fs::File,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use regex::Regex;

fn compile_irrt(target: &str, flags: &[&str]) -> String {
    let output = Command::new("clang-irrt")
        .arg(format!("--target={target}"))
        .args(flags)
        .output()
        .inspect(|o| {
            assert!(o.status.success(), "{}", std::str::from_utf8(&o.stderr).unwrap());
        })
        .unwrap();
    // https://github.com/rust-lang/regex/issues/244
    std::str::from_utf8(&output.stdout).unwrap().replace("\r\n", "\n")
}

// Returns true if an IR `define` block is a 64-bit IRRT function.
// extern "C" 64-bit functions end with "64" (e.g. __nac3_ndarray_len64).
// C++ template helpers using _BitInt(64) are mangled by Clang as "IDB64".
fn is_64bit_define(block: &str, name_re: &Regex) -> bool {
    if !block.starts_with("define") {
        return false;
    }
    if let Some(cap) = name_re.captures(block) {
        let name = &cap[1];
        return name.ends_with("64") || name.contains("IDB64");
    }
    false
}

fn main() {
    // For debugging
    // Doing `DEBUG_DUMP_IRRT=1 cargo build -p nac3core` dumps the LLVM IR generated
    const DEBUG_DUMP_IRRT: &str = "DEBUG_DUMP_IRRT";

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let irrt_dir = Path::new("irrt");

    let irrt_cpp_path = irrt_dir.join("irrt.cpp");

    /*
     * HACK: Sadly, clang doesn't let us emit generic LLVM bitcode.
     * Compiling for WASM (wasm32 for 32-bit, wasm64 for 64-bit) and
     * filtering the output with regex is the closest we can get.
     */
    let mut flags: Vec<&str> = vec![
        "-x",
        "c++",
        "-std=c++20",
        "-fno-discard-value-names",
        "-fno-exceptions",
        "-fno-rtti",
        "-emit-llvm",
        "-S",
        "-Wall",
        "-Wextra",
        "-o",
        "-",
        "-I",
        irrt_dir.to_str().unwrap(),
        irrt_cpp_path.to_str().unwrap(),
    ];

    match env::var("PROFILE").as_deref() {
        Ok("debug") => {
            flags.push("-g");
            flags.push("-O0");
            flags.push("-DIRRT_DEBUG_ASSERT");
        }
        Ok("release") => {
            flags.push("-O3");
        }
        flavor => panic!("Unknown or missing build flavor {flavor:?}"),
    }

    // Tell Cargo to rerun if any file under `irrt_dir` (recursive) changes
    println!("cargo:rerun-if-changed={}", irrt_dir.to_str().unwrap());

    let wasm32_output = compile_irrt("wasm32", &flags);
    let wasm64_output = compile_irrt("wasm64", &flags);
    let mut filtered_output = String::with_capacity(wasm32_output.len() + wasm64_output.len());

    // Filter out irrelevant IR
    //
    // Regex:
    // - `(?ms:^define.*?\}$)` captures LLVM `define` blocks
    // - `(?m:^declare.*?$)` captures LLVM `declare` lines
    // - `(?m:^%.+?=\s*type\s*(?:\{.+?\}|opaque)$)` captures LLVM `type` declarations
    // - `(?m:^\$.+=\s*comdat.+$)` captures COMDAT entries (only in debug mode)
    // - `(?m:^@.+?=.+$)` captures global constants
    // - `(?m:^!.+?=.+$)` captures metadata (only in debug mode)
    // - `(?m:^attributes #\d+\s*=.+$)` captures attribute groups (only in debug mode)
    let regex_filter = match env::var("PROFILE").as_deref() {
        Ok("debug") => {
            Regex::new(
                r"(?ms:^define.*?\}$)|(?m:^declare.*?$)|(?m:^%.+?=\s*type\s*(?:\{.+?\}|opaque)$)|(?m:^\$.+=\s*comdat.+$)|(?m:^@.+?=.+$)|(?m:^!.+?=.+$)|(?m:^attributes #\d+\s*=.+$)",
            ).unwrap()
        },
        Ok("release") => Regex::new(
            r"(?ms:^define.*?\}$)|(?m:^declare.*?$)|(?m:^%.+?=\s*type\s*(?:\{.+?\}|opaque)$)|(?m:^@.+?=.+$)",
        )
        .unwrap(),
        _ => unreachable!(),
    };

    // Regex to extract the function name from a `define` block header
    let name_re = Regex::new(r"@(\w+)\(").unwrap();

    // From wasm32: keep everything except 64-bit define blocks
    for f in regex_filter.captures_iter(&wasm32_output) {
        assert_eq!(f.len(), 1);
        if is_64bit_define(&f[0], &name_re) {
            continue;
        }
        filtered_output.push_str(&f[0]);
        filtered_output.push('\n');
    }

    // From wasm64: keep only 64-bit define blocks
    for f in regex_filter.captures_iter(&wasm64_output) {
        assert_eq!(f.len(), 1);
        if is_64bit_define(&f[0], &name_re) {
            filtered_output.push_str(&f[0]);
            filtered_output.push('\n');
        }
    }

    let filtered_output = match env::var("PROFILE").as_deref() {
        Ok("debug") => Regex::new("(\"target-features\"=\".*\")").unwrap(),
        Ok("release") => {
            Regex::new("(#\\d+)|(, *![0-9A-Za-z.]+)|(![0-9A-Za-z.]+)|(!\".*?\")").unwrap()
        }
        _ => unreachable!(),
    }
    .replace_all(&filtered_output, "");

    if env::var(DEBUG_DUMP_IRRT).is_ok() {
        let mut file = File::create(out_dir.join("irrt-wasm32.ll")).unwrap();
        file.write_all(wasm32_output.as_bytes()).unwrap();

        let mut file = File::create(out_dir.join("irrt-wasm64.ll")).unwrap();
        file.write_all(wasm64_output.as_bytes()).unwrap();

        let mut file = File::create(out_dir.join("irrt-filtered.ll")).unwrap();
        file.write_all(filtered_output.as_bytes()).unwrap();
    }

    let mut llvm_as = Command::new("llvm-as-irrt")
        .stdin(Stdio::piped())
        .arg("-o")
        .arg(out_dir.join("irrt.bc"))
        .spawn()
        .unwrap();
    llvm_as.stdin.as_mut().unwrap().write_all(filtered_output.as_bytes()).unwrap();
    assert!(llvm_as.wait().unwrap().success());
}
