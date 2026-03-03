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

#[cfg(feature = "yaml")]
mod yaml_tests {
    use super::*;

    #[derive(Resource, Debug, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        value: i32,
        name: String,
    }

    impl ConfigFile for TestConfig {
        const PATH: &'static str = "config.yaml";
    }

    #[derive(Resource, Debug, Serialize, Deserialize, PartialEq)]
    struct TestYmlConfig {
        value: i32,
        name: String,
    }

    impl ConfigFile for TestYmlConfig {
        const PATH: &'static str = "config.yml";
    }

    #[test]
    fn test_load_valid_yml_config() {
        run_config_test::<TestYmlConfig, _>(
            Some("value: 42\nname: test\n"),
            vec![],
            |app, load_result| {
                assert!(load_result.is_ok());
                let config = app.world().get_resource::<TestYmlConfig>().unwrap();
                assert_eq!(config.value, 42);
                assert_eq!(config.name, "test");
            },
        );
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
}

#[cfg(feature = "json")]
mod json_tests {
    use super::*;

    #[derive(Resource, Debug, Serialize, Deserialize, PartialEq)]
    struct TestJsonConfig {
        value: i32,
        name: String,
    }

    impl ConfigFile for TestJsonConfig {
        const PATH: &'static str = "config.json";
    }

    #[test]
    fn test_load_valid_json_config() {
        run_config_test::<TestJsonConfig, _>(
            Some(r#"{"value": 42, "name": "test"}"#),
            vec![],
            |app, load_result| {
                assert!(load_result.is_ok());
                let config = app.world().get_resource::<TestJsonConfig>().unwrap();
                assert_eq!(config.value, 42);
                assert_eq!(config.name, "test");
            },
        );
    }

    #[test]
    fn test_load_missing_json_config_file() {
        run_config_test::<TestJsonConfig, _>(
            None,
            vec![],
            |app, load_result| {
                assert!(load_result.is_err());
                assert!(app.world().get_resource::<TestJsonConfig>().is_none());
            },
        );
    }

    #[test]
    fn test_load_invalid_json_config() {
        run_config_test::<TestJsonConfig, _>(
            Some("not valid json {{{"),
            vec![],
            |app, load_result| {
                assert!(load_result.is_err());
                assert!(app.world().get_resource::<TestJsonConfig>().is_none());
            },
        );
    }

    #[test]
    fn test_load_json_with_env_override() {
        run_config_test::<TestJsonConfig, _>(
            Some(r#"{"value": 42, "name": "test"}"#),
            vec![("CONFIG_TestJsonConfig", r#"{"value": 100}"#)],
            |app, load_result| {
                assert!(load_result.is_ok());
                let config = app.world().get_resource::<TestJsonConfig>().unwrap();
                assert_eq!(config.value, 100);
                assert_eq!(config.name, "test");
            },
        );
    }

    #[test]
    fn test_load_json_with_invalid_env_override() {
        run_config_test::<TestJsonConfig, _>(
            Some(r#"{"value": 42, "name": "test"}"#),
            vec![("CONFIG_TestJsonConfig", r#"{"value": invalid json}"#)],
            |app, load_result| {
                assert!(load_result.is_err());
                assert!(app.world().get_resource::<TestJsonConfig>().is_none());
            },
        );
    }
}

#[cfg(feature = "ron")]
mod ron_tests {
    use super::*;

    #[derive(Resource, Debug, Serialize, Deserialize, PartialEq)]
    struct TestRonConfig {
        value: i32,
        name: String,
    }

    impl ConfigFile for TestRonConfig {
        const PATH: &'static str = "config.ron";
    }

    #[test]
    fn test_load_valid_ron_config() {
        run_config_test::<TestRonConfig, _>(
            Some("(value: 42, name: \"test\")"),
            vec![],
            |app, load_result| {
                assert!(load_result.is_ok());
                let config = app.world().get_resource::<TestRonConfig>().unwrap();
                assert_eq!(config.value, 42);
                assert_eq!(config.name, "test");
            },
        );
    }

    #[test]
    fn test_load_missing_ron_config_file() {
        run_config_test::<TestRonConfig, _>(
            None,
            vec![],
            |app, load_result| {
                assert!(load_result.is_err());
                assert!(app.world().get_resource::<TestRonConfig>().is_none());
            },
        );
    }

    #[test]
    fn test_load_invalid_ron_config() {
        run_config_test::<TestRonConfig, _>(
            Some("not valid ron {{{"),
            vec![],
            |app, load_result| {
                assert!(load_result.is_err());
                assert!(app.world().get_resource::<TestRonConfig>().is_none());
            },
        );
    }

    #[test]
    fn test_load_ron_with_env_override() {
        run_config_test::<TestRonConfig, _>(
            Some("(value: 42, name: \"test\")"),
            vec![("CONFIG_TestRonConfig", r#"{"value": 100}"#)],
            |app, load_result| {
                assert!(load_result.is_ok());
                let config = app.world().get_resource::<TestRonConfig>().unwrap();
                assert_eq!(config.value, 100);
                assert_eq!(config.name, "test");
            },
        );
    }

    #[test]
    fn test_load_ron_with_invalid_env_override() {
        run_config_test::<TestRonConfig, _>(
            Some("(value: 42, name: \"test\")"),
            vec![("CONFIG_TestRonConfig", r#"{"value": invalid json}"#)],
            |app, load_result| {
                assert!(load_result.is_err());
                assert!(app.world().get_resource::<TestRonConfig>().is_none());
            },
        );
    }
}

mod unsupported_format_tests {
    use super::*;
    use bevy_config_file::{load_config_file, LoadConfigError};

    #[derive(Debug, Serialize, Deserialize)]
    struct TestTomlConfig {
        value: i32,
    }

    impl ConfigFile for TestTomlConfig {
        const PATH: &'static str = "config.toml";
    }

    #[test]
    fn test_unsupported_format_returns_error() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_dir = TempDir::new().unwrap();
        fs::write(test_dir.path().join("config.toml"), "value = 42").unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(test_dir.path()).unwrap();

        let result = load_config_file::<TestTomlConfig>();
        match result {
            Err(LoadConfigError::UnsupportedFormat(ext)) => assert_eq!(ext, "toml"),
            other => panic!("expected UnsupportedFormat, got {:?}", other),
        }

        std::env::set_current_dir(original_dir).unwrap();
    }
}
