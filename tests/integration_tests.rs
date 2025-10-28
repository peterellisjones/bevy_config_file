//! Integration tests for the config file loading functionality.

use bevy::app::App;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::Resource;
use bevy_config_file::{load_resource_from_config_file, ConfigFile};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Mutex;
use tempfile::TempDir;

// Mutex to serialize all tests since they change current_dir which is process-global
static TEST_MUTEX: Mutex<()> = Mutex::new(());

/// Generic test config that can be reused across all tests
#[derive(Resource, Debug, Serialize, Deserialize, PartialEq)]
struct TestConfig {
    value: i32,
    name: String,
}

impl ConfigFile for TestConfig {
    const PATH: &'static str = "config.yaml";
}

/// Helper function to run a test with proper locking and cleanup.
///
/// This manages:
/// - Acquiring the TEST_MUTEX to serialize tests
/// - Creating an isolated temp directory
/// - Optionally writing a config file
/// - Optionally setting environment variables
/// - Changing to the temp directory and restoring it after
/// - Running the config loading system and passing the app + load result to the callback
fn run_config_test<T, F>(
    config_content: Option<&str>,
    env_vars: Vec<(&str, &str)>,
    test_fn: F,
) where
    T: Resource + for<'de> Deserialize<'de> + Serialize + ConfigFile,
    F: FnOnce(App, Result<(), bevy::prelude::BevyError>),
{
    // Lock to serialize tests that change current_dir and env vars
    let _lock = TEST_MUTEX.lock().unwrap();

    // Create isolated temp directory
    let test_dir = TempDir::new().unwrap();

    // Optionally write config file
    if let Some(content) = config_content {
        let config_path = test_dir.path().join(T::PATH);
        fs::write(&config_path, content).unwrap();
    }

    // Save original directory and change to test directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(test_dir.path()).unwrap();

    // Set environment variables
    for (key, value) in &env_vars {
        unsafe { std::env::set_var(key, value); }
    }

    // Run the config loading system
    let mut app = App::new();
    let system_result = app.world_mut().run_system_once(load_resource_from_config_file::<T>);

    // Unwrap the outer Result (system execution) - panic if system fails to run
    let load_result = system_result.expect("System failed to execute");

    // Pass app and the inner loading result to the test callback
    test_fn(app, load_result);

    // Cleanup: remove env vars and restore directory
    for (key, _) in &env_vars {
        unsafe { std::env::remove_var(key); }
    }
    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_load_valid_config() {
    run_config_test::<TestConfig, _>(
        Some("value: 42\nname: test\n"),
        vec![],
        |app, load_result| {
            assert!(load_result.is_ok());

            let config = app.world().get_resource::<TestConfig>();
            assert!(config.is_some());
            let config = config.unwrap();
            assert_eq!(config.value, 42);
            assert_eq!(config.name, "test");
        },
    );
}

#[test]
fn test_load_missing_config_file() {
    run_config_test::<TestConfig, _>(
        None, // No config file written
        vec![],
        |app, load_result| {
            assert!(load_result.is_err());
            let config = app.world().get_resource::<TestConfig>();
            assert!(config.is_none());
        },
    );
}

#[test]
fn test_load_invalid_yaml() {
    run_config_test::<TestConfig, _>(
        Some("invalid: yaml: content:\n  - this is bad\n  missing bracket"),
        vec![],
        |app, load_result| {
            assert!(load_result.is_err());
            let config = app.world().get_resource::<TestConfig>();
            assert!(config.is_none());
        },
    );
}

#[test]
fn test_load_with_env_override() {
    run_config_test::<TestConfig, _>(
        Some("value: 42\nname: test\n"),
        vec![("CONFIG_TestConfig", r#"{"value": 100}"#)],
        |app, load_result| {
            assert!(load_result.is_ok());

            let config = app.world().get_resource::<TestConfig>();
            assert!(config.is_some());
            let config = config.unwrap();
            assert_eq!(config.value, 100); // Overridden value
            assert_eq!(config.name, "test"); // Original value
        },
    );
}

#[test]
fn test_load_with_invalid_env_override() {
    run_config_test::<TestConfig, _>(
        Some("value: 42\nname: test\n"),
        vec![("CONFIG_TestConfig", r#"{"value": invalid json}"#)],
        |app, load_result| {
            assert!(load_result.is_err());
            let config = app.world().get_resource::<TestConfig>();
            assert!(config.is_none());
        },
    );
}
