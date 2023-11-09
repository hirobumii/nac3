use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use parking_lot::Mutex;

/// [Mutex] for enforcing only a single `./run_demo.sh` is executing.
///
/// This is necessary as the command will generate temporary files (specifically `*.o`, `*.bc`, and
/// `demo`) in the filesystem.
static RUN_EXEC_MTX: Mutex<()> = Mutex::new(());

/// Dummy struct for automatically removing temporary files after test case execution completes.
struct CleanupDemoTempFiles;

impl Default for CleanupDemoTempFiles {

    fn default() -> Self {
        CleanupDemoTempFiles {}
    }
}

impl Drop for CleanupDemoTempFiles {

    fn drop(&mut self) {
        get_demo_dir().read_dir()
            .unwrap()
            .into_iter()
            .map(|dir_entry| dir_entry.unwrap().path().into_boxed_path())
            .filter(|p| {
                let ext = p.extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or_default();
                let file_name = p.file_name()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or_default();

                ["bc", "o"].contains(&ext) || file_name == "demo"
            })
            .for_each(|p| fs::remove_file(p).unwrap())
    }
}

/// Returns the absolute [Path] to the `demo` directory.
fn get_demo_dir() -> PathBuf {
    std::env::current_dir()
        .map(|p| p.join("demo"))
        .and_then(|p| p.canonicalize())
        .unwrap()
}

/// Collects all Python tests from `demo/src`.
fn collect_demo_tests() -> Vec<PathBuf> {
    get_demo_dir()
        .join("src")
        .read_dir()
        .unwrap()
        .into_iter()
        .filter_map(|dir_entry| {
            let p = dir_entry.unwrap().path();

            if p.is_file() && p.extension().and_then(|ext| ext.to_str()).unwrap_or_default() == "py" {
                Some(p)
            } else {
                None
            }
        })
        .collect()
}

/// Runs the interpreter on the `file` as if by invoking `interpret_demo.py`.
fn run_interpreter(file: &Path) -> io::Result<Output> {
    Command::new("./interpret_demo.py")
        .arg(file.to_str().unwrap())
        .current_dir(get_demo_dir())
        .output()
}

/// Runs NAC3 on the `file` as if by invoking `run_demo.sh`.
fn run_demo(file: &Path, nac3_args: &[&str]) -> io::Result<Output> {
    let _lk = RUN_EXEC_MTX.lock();

    Command::new("./run_demo.sh")
        .args(nac3_args)
        .arg(file.to_str().unwrap())
        .current_dir(get_demo_dir())
        .output()
}

#[test]
fn test_demos_optimized() {
    let _cleanup = CleanupDemoTempFiles::default();

    let tests = collect_demo_tests();

    for test in tests.iter() {
        let py_interpreted= run_interpreter(test).unwrap();
        let py_stdout = String::from_utf8(py_interpreted.stdout).unwrap();

        let executed = run_demo(test, &[]).unwrap();
        let exec_stdout = String::from_utf8(executed.stdout).unwrap();
        assert_eq!(py_stdout, exec_stdout, "Execution result mismatch for {}", test.file_name().and_then(|p| p.to_str()).unwrap());

        let bc_interpreted = run_demo(test, &["--lli"]).unwrap();
        let bc_stdout = String::from_utf8(bc_interpreted.stdout).unwrap();
        assert_eq!(py_stdout, bc_stdout, "Execution result mismatch for {}", test.file_name().and_then(|p| p.to_str()).unwrap());
    }
}

#[test]
fn test_demos_unoptimized() {
    let _cleanup = CleanupDemoTempFiles::default();

    let tests = collect_demo_tests();

    for test in tests.iter() {
        let py_interpreted= run_interpreter(test).unwrap();
        let py_stdout = String::from_utf8(py_interpreted.stdout).unwrap();

        let executed = run_demo(test, &["-O0"]).unwrap();
        let exec_stdout = String::from_utf8(executed.stdout).unwrap();
        assert_eq!(py_stdout, exec_stdout, "Execution result differs for {}", test.file_name().and_then(|p| p.to_str()).unwrap());

        let bc_interpreted= run_demo(test, &["--lli", "-O0"]).unwrap();
        let bc_stdout = String::from_utf8(bc_interpreted.stdout).unwrap();
        assert_eq!(py_stdout, bc_stdout, "Execution result mismatch for {}", test.file_name().and_then(|p| p.to_str()).unwrap());
    }
}

#[test]
fn test_demos_multithreaded() {
    let _cleanup = CleanupDemoTempFiles::default();

    let tests = collect_demo_tests();

    for test in tests.iter() {
        let py_interpreted= run_interpreter(test).unwrap();
        let py_stdout = String::from_utf8(py_interpreted.stdout).unwrap();

        let executed = run_demo(test, &["-T4"]).unwrap();
        let exec_stdout = String::from_utf8(executed.stdout).unwrap();
        assert_eq!(py_stdout, exec_stdout, "Execution result differs for {}", test.file_name().and_then(|p| p.to_str()).unwrap());

        let bc_interpreted= run_demo(test, &["--lli", "-T4"]).unwrap();
        let bc_stdout = String::from_utf8(bc_interpreted.stdout).unwrap();
        assert_eq!(py_stdout, bc_stdout, "Execution result mismatch for {}", test.file_name().and_then(|p| p.to_str()).unwrap());
    }
}