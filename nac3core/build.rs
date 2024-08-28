use std::ffi::OsStr;
use std::{
    env,
    fs::File,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use itertools::Itertools;
use regex::Regex;

struct IRRTCompilation<'a> {
    pub file: &'a str,
    pub gcc_options: Vec<&'a str>,
    pub cargo_instructions: Vec<&'a str>,
}

/// Extracts the extension-less filename from a [`Path`].
fn path_to_extless_filename(path: &Path) -> &str {
    path.file_name().map(Path::new).and_then(Path::file_stem).and_then(OsStr::to_str).unwrap()
}

/// Compiles a source C file into LLVM bitcode.
fn compile_file_to_ir(compile_opts: &IRRTCompilation) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);
    let irrt_dir = Path::new("irrt");

    let path = Path::new(compile_opts.file);
    let filename_without_ext = path_to_extless_filename(path);

    /*
     * HACK: Sadly, clang doesn't let us emit generic LLVM bitcode.
     * Compiling for WASM32 and filtering the output with regex is the closest we can get.
     */
    let mut flags: Vec<&str> = vec![
        "--target=wasm32",
        "-x",
        "c++",
        "-std=c++20",
        "-fno-discard-value-names",
        "-fno-exceptions",
        "-fno-rtti",
        "-emit-llvm",
        "-Xclang",
        "-no-opaque-pointers",
        "-S",
        "-Wall",
        "-Wextra",
        "-o",
        "-",
        "-I",
        irrt_dir.to_str().unwrap(),
    ];

    // Apply custom flags from IRRTCompilation
    flags.extend_from_slice(&compile_opts.gcc_options);

    match env::var("PROFILE").as_deref() {
        Ok("debug") => flags.extend_from_slice(&["-O0", "-DIRRT_DEBUG_ASSERT"]),
        Ok("release") => flags.push("-O3"),
        flavor => panic!("Unknown or missing build flavor {flavor:?}"),
    }

    flags.push(path.to_str().unwrap());

    // Tell Cargo to rerun if the main IRRT source is changed
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
    compile_opts.cargo_instructions.iter().for_each(|inst| println!("cargo::{inst}"));

    // Compile IRRT and capture the LLVM IR output
    let output = Command::new("clang-irrt")
        .args(flags)
        .output()
        .inspect(|o| {
            assert!(o.status.success(), "{}", std::str::from_utf8(&o.stderr).unwrap());
        })
        .unwrap();

    let output = std::str::from_utf8(&output.stdout).unwrap();
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
    for f in regex_filter.captures_iter(output) {
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
    if env::var("DEBUG_DUMP_IRRT").is_ok() {
        let mut file = File::create(out_path.join(format!("{filename_without_ext}.ll"))).unwrap();
        file.write_all(output.as_bytes()).unwrap();

        let mut file =
            File::create(out_path.join(format!("{filename_without_ext}-filtered.ll"))).unwrap();
        file.write_all(filtered_output.as_bytes()).unwrap();
    }

    let mut llvm_as = Command::new("llvm-as-irrt")
        .stdin(Stdio::piped())
        .arg("-opaque-pointers=0")
        .arg("-o")
        .arg(out_path.join(format!("{filename_without_ext}.bc")))
        .spawn()
        .unwrap();
    llvm_as.stdin.as_mut().unwrap().write_all(filtered_output.as_bytes()).unwrap();
    assert!(llvm_as.wait().unwrap().success());
}

fn main() {
    let irrt_compilations: &[IRRTCompilation] = &[
        IRRTCompilation {
            file: "irrt/irrt.cpp",
            gcc_options: Vec::default(),
            cargo_instructions: vec!["rerun-if-changed=irrt/irrt"],
        },
        IRRTCompilation {
            file: "irrt/tracert.cpp",
            gcc_options: Vec::default(),
            cargo_instructions: Vec::default(),
        },
    ];

    assert!(
        irrt_compilations
            .iter()
            .map(|comp| comp.file)
            .map(Path::new)
            .map(path_to_extless_filename)
            .all_unique()
    );

    for path in irrt_compilations {
        compile_file_to_ir(path)
    }
}
