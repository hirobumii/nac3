use regex::Regex;
use std::{
    env,
    fs::File,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let irrt_dir = Path::new("irrt");

    let irrt_cpp_path = irrt_dir.join("irrt.cpp");

    /*
     * HACK: Sadly, clang doesn't let us emit generic LLVM bitcode.
     * Compiling for WASM32 and filtering the output with regex is the closest we can get.
     */
    let flags: &[&str] = &[
        "--target=wasm32",
        "-x",
        "c++",
        "-fno-discard-value-names",
        "-fno-exceptions",
        "-fno-rtti",
        match env::var("PROFILE").as_deref() {
            Ok("debug") => "-O0",
            Ok("release") => "-O3",
            flavor => panic!("Unknown or missing build flavor {flavor:?}"),
        },
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

    // Tell Cargo to rerun if any file under `irrt_dir` (recursive) changes
    println!("cargo:rerun-if-changed={}", irrt_dir.to_str().unwrap());

    // Compile IRRT and capture the LLVM IR output
    let output = Command::new("clang-irrt")
        .args(flags)
        .output()
        .map(|o| {
            assert!(o.status.success(), "{}", std::str::from_utf8(&o.stderr).unwrap());
            o
        })
        .unwrap();

    // https://github.com/rust-lang/regex/issues/244
    let output = std::str::from_utf8(&output.stdout).unwrap().replace("\r\n", "\n");
    let mut filtered_output = String::with_capacity(output.len());

    // Filter out irrelevant IR
    //
    // Regex:
    // - `(?ms:^define.*?\}$)` captures LLVM `define` blocks
    // - `(?m:^declare.*?$)` captures LLVM `declare` lines
    // - `(?m:^%.+?=\s*type\s*\{.+?\}$)` captures LLVM `type` declarations
    // - `(?m:^@.+?=.+$)` captures global constants
    let regex_filter = Regex::new(
        r"(?ms:^define.*?\}$)|(?m:^declare.*?$)|(?m:^%.+?=\s*type\s*\{.+?\}$)|(?m:^@.+?=.+$)",
    )
    .unwrap();
    for f in regex_filter.captures_iter(&output) {
        assert_eq!(f.len(), 1);
        filtered_output.push_str(&f[0]);
        filtered_output.push('\n');
    }

    let filtered_output = Regex::new("(#\\d+)|(, *![0-9A-Za-z.]+)|(![0-9A-Za-z.]+)|(!\".*?\")")
        .unwrap()
        .replace_all(&filtered_output, "");

    // For debugging
    // Doing `DEBUG_DUMP_IRRT=1 cargo build -p nac3core` dumps the LLVM IR generated
    const DEBUG_DUMP_IRRT: &str = "DEBUG_DUMP_IRRT";
    println!("cargo:rerun-if-env-changed={DEBUG_DUMP_IRRT}");
    if env::var(DEBUG_DUMP_IRRT).is_ok() {
        let mut file = File::create(out_dir.join("irrt.ll")).unwrap();
        file.write_all(output.as_bytes()).unwrap();

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
