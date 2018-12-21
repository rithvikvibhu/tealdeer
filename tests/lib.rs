//! Integration tests.

use std::fs::File;
use std::io::Write;
use std::process::Command;

use assert_cmd::prelude::*;
use tempdir::TempDir;
use predicates::boolean::PredicateBooleanExt;
use predicates::prelude::predicate::str::{contains, is_empty, similar};

struct TestEnv {
    pub cache_dir: TempDir,
    pub config_dir: TempDir,
    pub input_dir: TempDir,
    pub default_features: bool,
}

impl TestEnv {
    fn new() -> Self {
        TestEnv {
            cache_dir: TempDir::new(".tldr.test.cache").unwrap(),
            config_dir: TempDir::new(".tldr.test.config").unwrap(),
            input_dir: TempDir::new(".tldr.test.input").unwrap(),
            default_features: true,
        }
    }

    /// Disable default features.
    fn no_default_features(mut self) -> Self {
        self.default_features = false;
        self
    }

    /// Return a new `Command` with env vars set.
    fn command(&self) -> Command {
        let mut build = escargot::CargoBuild::new()
            .bin("tldr")
            .current_release()
            .current_target();
        if !self.default_features {
            build = build
                .arg("--no-default-features")
                .target_dir("target/no-default-features");
        }
        let run = build.run().unwrap();
        let mut cmd = run.command();
        cmd.env("TEALDEER_CACHE_DIR", self.cache_dir.path().to_str().unwrap());
        cmd.env("TEALDEER_CONFIG_DIR", self.config_dir.path().to_str().unwrap());
        cmd
    }
}

#[test]
fn test_missing_cache() {
    TestEnv::new()
        .command()
        .args(&["sl"])
        .assert()
        .failure()
        .stderr(contains("Cache not found. Please run `tldr --update`."));
}

#[test]
fn test_update_cache() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(&["sl"])
        .assert()
        .failure()
        .stderr(contains("Cache not found. Please run `tldr --update`."));

    testenv
        .command()
        .args(&["--update"])
        .assert()
        .success()
        .stdout(contains("Successfully updated cache."));

    testenv
        .command()
        .args(&["sl"])
        .assert()
        .success();
}

#[test]
fn test_quiet_cache() {
    let testenv = TestEnv::new();
    testenv
        .command()
        .args(&["--update", "--quiet"])
        .assert()
        .success()
        .stdout(is_empty());

    testenv
        .command()
        .args(&["--clear-cache", "--quiet"])
        .assert()
        .success()
        .stdout(is_empty());
}

#[test]
fn test_quiet_failures() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(&["--update", "-q"])
        .assert()
        .success()
        .stdout(is_empty());

    testenv
        .command()
        .args(&["fakeprogram", "-q"])
        .assert()
        .failure()
        .stdout(is_empty());
}

#[test]
fn test_quiet_old_cache() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(&["--update", "-q"])
        .assert()
        .success()
        .stdout(is_empty());

    let _ = utime::set_file_times(testenv.cache_dir.path().join("tldr-master"), 1, 1).unwrap();

    testenv
        .command()
        .args(&["tldr"])
        .assert()
        .success()
        .stdout(contains("Cache wasn't updated for more than "));

    testenv
        .command()
        .args(&["tldr", "--quiet"])
        .assert()
        .success()
        .stdout(contains("Cache wasn't updated for more than ").not());
}

#[test]
fn test_setup_seed_config() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(&["--seed-config"])
        .assert()
        .success()
        .stdout(contains("Successfully created seed config file"));
}

#[test]
fn test_show_config_path() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(&["--config-path"])
        .assert()
        .success()
        .stdout(contains(format!(
            "Config path is: {}/config.toml",
            testenv.config_dir.path().to_str().unwrap(),
        )));
}

fn _test_correct_rendering(input_file: &str, filename: &str) {
    let testenv = TestEnv::new();

    // Create input file
    let file_path = testenv.input_dir.path().join(filename);
    println!("Testfile path: {:?}", &file_path);
    let mut file = File::create(&file_path).unwrap();
    file.write_all(input_file.as_bytes()).unwrap();

    // Load expected output
    let expected = include_str!("inkscape-default.expected");

    testenv
        .command()
        .args(&["-f", &file_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(similar(expected));
}

/// An end-to-end integration test for direct file rendering (v1 syntax).
#[test]
fn test_correct_rendering_v1() {
    _test_correct_rendering(include_str!("inkscape-v1.md"), "inkscape-v1.md");
}

/// An end-to-end integration test for direct file rendering (v2 syntax).
#[test]
fn test_correct_rendering_v2() {
    _test_correct_rendering(include_str!("inkscape-v2.md"), "inkscape-v2.md");
}

/// An end-to-end integration test for rendering with constom syntax config.
#[test]
fn test_correct_rendering_with_config() {
    let testenv = TestEnv::new();

    // Setup config file
    // TODO should be config::CONFIG_FILE_NAME
    let config_file_path = testenv.config_dir.path().join("config.toml");
    println!("Config path: {:?}", &config_file_path);

    let mut config_file = File::create(&config_file_path).unwrap();
    config_file
        .write(include_str!("config.toml").as_bytes())
        .unwrap();

    // Create input file
    let file_path = testenv.input_dir.path().join("inkscape-v2.md");
    println!("Testfile path: {:?}", &file_path);

    let mut file = File::create(&file_path).unwrap();
    file.write_all(include_str!("inkscape-v2.md").as_bytes()).unwrap();

    // Load expected output
    let expected = include_str!("inkscape-with-config.expected");

    testenv
        .command()
        .args(&["-f", &file_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(similar(expected));
}

/// Updating from a network URL should not be possible when networking support
/// is not enabled.
#[test]
fn test_update_from_no_networking() {
    let testenv = TestEnv::new();
    testenv
        .no_default_features()  // Disable networking
        .command()
        .args(&["--update-from", "https://github.com/tldr-pages/tldr/archive/master.tar.gz"])
        .assert()
        .failure()
        .stderr(contains("compiled without networking support"))
        .stderr(contains("cannot update the cache from a network URL"));
}

/// Updating from an invalid URL should result in an error message.
#[test]
fn test_update_from_invalid_url() {
    let testenv = TestEnv::new();
    testenv
        .command()
        .args(&["--update-from", "httpsss:github.com/tldr-pages/tldr/archive/master.tar.gz"])
        .assert()
        .failure()
        .stderr(contains("Could not update cache: HTTP error"))
        .stderr(contains("URL scheme is not allowed"));
}

/// Updating from the wrong (non-gzip-archive) URL should result in an error.
#[test]
fn test_update_from_wrong_url() {
    let testenv = TestEnv::new();
    testenv
        .command()
        .args(&["--update-from", "https://tldr.sh/"])
        .assert()
        .failure()
        .stderr(contains("Could not update cache: Could not unpack compressed data"));
}

/// When a path is specified that does not exist, an error message should be shown.
#[test]
fn test_update_from_missing_path() {
    let testenv = TestEnv::new();
    testenv
        .no_default_features()
        .command()
        .args(&["--update-from", "ajsdfasjdkfljasdf"])  // Invalid path
        .assert()
        .failure()
        .stderr(contains("Could not update cache: Could not open file:"))
        .stderr(contains("No such file or directory"));
}
