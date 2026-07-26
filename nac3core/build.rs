#![deny(clippy::all)]
#![warn(clippy::cargo, clippy::pedantic, clippy::nursery)]
#![allow(clippy::cargo_common_metadata)]

use std::{
    env,
    fs::File,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    sync::LazyLock,
};

use build_rs::{input::cargo_feature, output::rerun_if_changed};
use regex::Regex;

// For debugging
// Doing `DEBUG_DUMP_IRRT=1 cargo build -p nac3core` dumps the LLVM IR generated
const DEBUG_DUMP_IRRT: &str = "DEBUG_DUMP_IRRT";

/// A regex that captures all LLVM IR declarations and definitions, used to extract meaningful
/// declarations and (in debug mode) metadata.
///
/// - `(?ms:^define.*?\}$)` captures LLVM `define` blocks
/// - `(?m:^declare.*?$)` captures LLVM `declare` lines
/// - `(?m:^%.+?=\s*type\s*(?:\{.+?\}|opaque)$)` captures LLVM `type` declarations
/// - `(?m:^\$.+=\s*comdat.+$)` captures COMDAT entries (only in debug builds)
/// - `(?m:^@.+?=.+$)` captures global constants
/// - `(?m:^!.+?=.+$)` captures metadata (only in debug builds)
/// - `(?m:^attributes #\d+\s*=.+$)` captures attribute groups (only in debug builds)
static LLVM_IR_EXTRACT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    match env::var("PROFILE").as_deref() {
        Ok("debug") => {
            Regex::new(
                r"(?ms:^define.*?\}$)|(?m:^declare.*?$)|(?m:^%.+?=\s*type\s*(?:\{.+?\}|opaque)$)|(?m:^\$.+=\s*comdat.+$)|(?m:^@.+?=.+$)|(?m:^!.+?=.+$)|(?m:^attributes #\d+\s*=.+$)",
            )
        },
        Ok("release") => Regex::new(
            r"(?ms:^define.*?\}$)|(?m:^declare.*?$)|(?m:^%.+?=\s*type\s*(?:\{.+?\}|opaque)$)|(?m:^@.+?=.+$)",
        ),
        _ => unreachable!(),
    }.unwrap()
});

/// A regex that matches all non-essential information, used to strip target-specific information
/// and other non-essential information from the LLVM IR output.
///
/// In debug builds:
///
/// - `(\"target-features\"=\".*\")` matches the `target-features` attribute
///
/// In release builds:
///
/// - `(#\d+)` matches attribute group references
/// - `(, *![0-9A-Za-z.]+)` matches metadata references in function definitions
/// - `(![0-9A-Za-z.]+)` matches metadata references in instructions
/// - `(!\".*?\")` matches metadata definitions
static LLVM_IR_SANITIZE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    match env::var("PROFILE").as_deref() {
        Ok("debug") => Regex::new("(\"target-features\"=\".*\")"),
        Ok("release") => Regex::new("(#\\d+)|(, *![0-9A-Za-z.]+)|(![0-9A-Za-z.]+)|(!\".*?\")"),
        _ => unreachable!(),
    }
    .unwrap()
});

/// A regex capturing the `target datalayout` string of the compiled IRRT module.
static LLVM_IR_DATALAYOUT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("(?m:^target datalayout = \"(.+)\"$)").unwrap());

/// Compiles the source file into LLVM IR for the given target and returns the output as a string.
fn compile_irrt(target: &str, flags: &[&str]) -> String {
    let output = Command::new("clang-irrt")
        .arg(format!("--target={target}"))
        .args(flags)
        .output()
        .inspect(|o| {
            assert!(o.status.success(), "{}", std::str::from_utf8(&o.stderr).unwrap());
        })
        .unwrap();
    String::from_utf8(output.stdout).unwrap()
}

/// Assembles the LLVM IR and writes the result into `output_file`.
fn assemble_irrt(ir: &str, output_file: &Path) {
    let mut llvm_as = Command::new("llvm-as-irrt")
        .stdin(Stdio::piped())
        .arg("-o")
        .arg(output_file)
        .spawn()
        .unwrap();
    llvm_as.stdin.as_mut().unwrap().write_all(ir.as_bytes()).unwrap();
    assert!(llvm_as.wait().unwrap().success());
}

/// Compiles an IRRT source file with the given `input` name (without the file extension) into LLVM
/// bitcode for the given target and writes it to the output directory as `{input}{suffix}.bc`.
fn compile_to_bc_with_target(target: &str, input: &str, suffix: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let irrt_dir = Path::new("irrt");

    let irrt_cpp_path = irrt_dir.join(format!("{input}.cpp"));

    /*
     * HACK: Sadly, clang doesn't let us emit generic LLVM bitcode.
     * Compiling for WASM (wasm32 for 32-bit, wasm64 for 64-bit) and
     * filtering the output with regex is the closest we can get.
     */
    let mut flags: Vec<&str> = vec![
        "-x",
        "c++",
        "-std=c++20",
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

    if cargo_feature("malloc") {
        flags.push("-DIRRT_MALLOC");
    }
    if cargo_feature("ctrc") {
        flags.push("-DIRRT_CTRC");
    }

    match env::var("PROFILE").as_deref() {
        Ok("debug") => {
            flags.push("-g");
            flags.push("-fno-discard-value-names");
            flags.push("-O0");
            flags.push("-DIRRT_DEBUG_ASSERT");
            flags.extend_from_slice(&["-Xclang", "-disable-O0-optnone"]);
        }
        Ok("release") => {
            flags.push("-O3");
        }
        flavor => panic!("Unknown or missing build flavor {flavor:?}"),
    }

    // Tell Cargo to rerun if any file under `irrt_dir` (recursive) changes
    rerun_if_changed(irrt_dir);

    let output = compile_irrt(target, &flags);

    // Record the datalayout IRRT was compiled with, so that the layout invariance tests can check
    // that no struct is laid out differently by the datalayout baked into the bitcode.
    let datalayout = LLVM_IR_DATALAYOUT_REGEX
        .captures(&output)
        .expect("IRRT output has no `target datalayout` line")
        .get(1)
        .map(|m| m.as_str())
        .unwrap();
    let mut file = File::create(out_dir.join(format!("{input}{suffix}.datalayout"))).unwrap();
    file.write_all(datalayout.as_bytes()).unwrap();

    let mut filtered_output = String::with_capacity(output.len());

    for f in LLVM_IR_EXTRACT_REGEX.captures_iter(&output) {
        assert_eq!(f.len(), 1);
        filtered_output.push_str(&f[0]);
        filtered_output.push('\n');
    }

    let filtered_output = LLVM_IR_SANITIZE_REGEX.replace_all(&filtered_output, "");

    if env::var(DEBUG_DUMP_IRRT).is_ok() {
        let mut file = File::create(out_dir.join(format!("{input}{suffix}.ll"))).unwrap();
        file.write_all(output.as_bytes()).unwrap();

        let mut file = File::create(out_dir.join(format!("{input}{suffix}-filtered.ll"))).unwrap();
        file.write_all(filtered_output.as_bytes()).unwrap();
    }

    assemble_irrt(&filtered_output, &out_dir.join(format!("{input}{suffix}.bc")));
}

/// Compiles an IRRT source file with the given `input` name (without the file extension) into LLVM
/// bitcode for both 32-bit and 64-bit targets.
///
/// The result is written into the output directory as `{input}32.bc` and `{input}64.bc`.
fn compile_to_bc(input: &str) {
    compile_to_bc_with_target("wasm32", input, "32");
    compile_to_bc_with_target("wasm64", input, "64");
}

fn main() {
    compile_to_bc("irrt");
}
