use std::collections::BTreeSet;
use std::fs::{read_dir, read_to_string};
use std::path::Path;
use std::time::Duration;

use assert_cmd::Command;
use tempfile::NamedTempFile;

const TESTED_BIN: &str = "flang";
const TIMEOUT: Duration = Duration::from_secs(2);
const SAMPLES_DIR: &str = "tests/samples";
const EXPECTED_DIR: &str = "tests/expected";
const ACTIONS: &[&str] = &["tokens", "derivation", "ast", "opt-ast", "stack", "asm-amd64"];

#[test]
fn test_compiler_stages() {
    for sample in read_dir(SAMPLES_DIR).unwrap_or_else(|_| panic!("Failed to read SAMPLES_DIR {SAMPLES_DIR}")) {
        let sample_path = sample.unwrap().path();

        let sample_name = sample_path.file_stem().unwrap().to_str().unwrap();
        let sample_expected_dir = Path::new(EXPECTED_DIR).join(sample_name);

        for action in ACTIONS {
            let expected_path = sample_expected_dir.join(action);
            let expected_out = read_to_string(&expected_path)
                .unwrap_or_else(|_| panic!("Failed to read expected_path {expected_path:?}"));

            println!("Testing {sample_path:?} with action {action:?}");
            Command::cargo_bin(TESTED_BIN)
                .unwrap()
                .args(["--action", action, "--input"])
                .arg(&sample_path)
                .timeout(TIMEOUT)
                .assert()
                .success()
                .stdout(expected_out);
        }
    }
}

#[test]
fn test_compiled_execution() {
    for sample in read_dir(SAMPLES_DIR).unwrap_or_else(|_| panic!("Failed to read SAMPLES_DIR {SAMPLES_DIR}")) {
        let sample_path = sample.unwrap().path();

        let sample_name = sample_path.file_stem().unwrap().to_str().unwrap();
        let sample_expected_dir = Path::new(EXPECTED_DIR).join(sample_name);

        let executable_path = {
            let executable = NamedTempFile::new().unwrap();
            executable.path().to_owned()
        };

        println!("Compiling {sample_path:?}");
        Command::cargo_bin(TESTED_BIN)
            .unwrap()
            .args(["--action", "build-amd64", "--input"])
            .arg(&sample_path)
            .arg("--output")
            .arg(&executable_path)
            .timeout(TIMEOUT)
            .assert()
            .success();

        for (subdir, expected_exit_code) in [("success", 0), ("fail", 1)] {
            let test_cases_path = sample_expected_dir.join(subdir);

            let test_cases = read_dir(&test_cases_path)
                .unwrap_or_else(|_| panic!("Failed to read test_cases_dir_path {test_cases_path:?}"))
                .map(|x| x.unwrap().path().file_stem().unwrap().to_str().unwrap().to_owned())
                .filter(|x| !x.starts_with('.'))
                .collect::<BTreeSet<_>>();

            for test_case in test_cases {
                let (in_path, out_path) = (
                    test_cases_path.join(format!("{test_case}.in")),
                    test_cases_path.join(format!("{test_case}.out")),
                );
                let (in_str, out_str) = (
                    read_to_string(&in_path).unwrap_or_else(|_| panic!("Falied to read in {in_path:?}")),
                    read_to_string(&out_path).unwrap_or_else(|_| panic!("Falied to read out {out_path:?}")),
                );

                println!("Testing {in_path:?} against {out_path:?}");
                Command::new(&executable_path)
                    .write_stdin(in_str)
                    .timeout(TIMEOUT)
                    .assert()
                    .stdout(out_str)
                    .code(expected_exit_code);
            }
        }
    }
}
