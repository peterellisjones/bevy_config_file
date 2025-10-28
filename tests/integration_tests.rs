//! Integration tests for the config file loading functionality.

use bevy::app::App;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::Resource;
use bevy_config_file::{load_resource_from_config_file, ConfigFile};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use tempfile::TempDir;

// Shared test directory for all tests
static TEST_DIR: OnceLock<TempDir> = OnceLock::new();

fn get_test_dir() -> &'static TempDir {
    TEST_DIR.get_or_init(|| TempDir::new().unwrap())
}

#[derive(Resource, Debug, Serialize, Deserialize, PartialEq)]
struct ValidConfig {
    value: i32,
    name: String,
}

impl ConfigFile for ValidConfig {
    const PATH: &'static str = "valid_config.yaml";
}

#[derive(Resource, Debug, Serialize, Deserialize, PartialEq)]
struct MissingConfig {
    value: i32,
    name: String,
}

impl ConfigFile for MissingConfig {
    const PATH: &'static str = "missing_config.yaml";
}

#[derive(Resource, Debug, Serialize, Deserialize, PartialEq)]
struct InvalidYamlConfig {
    value: i32,
    name: String,
}

impl ConfigFile for InvalidYamlConfig {
    const PATH: &'static str = "invalid_yaml_config.yaml";
}

#[derive(Resource, Debug, Serialize, Deserialize, PartialEq)]
struct EnvOverrideConfig {
    value: i32,
    name: String,
}

impl ConfigFile for EnvOverrideConfig {
    const PATH: &'static str = "env_override_config.yaml";
}

#[derive(Resource, Debug, Serialize, Deserialize, PartialEq)]
struct InvalidEnvOverrideConfig {
    value: i32,
    name: String,
}

impl ConfigFile for InvalidEnvOverrideConfig {
    const PATH: &'static str = "invalid_env_override_config.yaml";
}

fn create_test_config_file(filename: &str, content: &str) -> PathBuf {
    let test_dir = get_test_dir();
    let config_path = test_dir.path().join(filename);
    fs::write(&config_path, content).unwrap();
    config_path
}

#[test]
fn test_load_valid_config() {
    let test_dir = get_test_dir();
    let _config_path = create_test_config_file(
        "valid_config.yaml",
        "value: 42\nname: test\n",
    );

    // Change to test directory so ConfigFile::PATH works
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(test_dir.path()).unwrap();

    let mut app = App::new();
    let result: Result<Result<(), _>, _> = app.world_mut().run_system_once(load_resource_from_config_file::<ValidConfig>);

    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok());
    let inner_result = result.unwrap();
    assert!(inner_result.is_ok());

    let config = app.world().get_resource::<ValidConfig>();
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.value, 42);
    assert_eq!(config.name, "test");
}

#[test]
fn test_load_missing_config_file() {
    let test_dir = get_test_dir();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(test_dir.path()).unwrap();

    let mut app = App::new();
    let result: Result<Result<(), _>, _> = app.world_mut().run_system_once(load_resource_from_config_file::<MissingConfig>);

    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok());
    let inner_result = result.unwrap();
    assert!(inner_result.is_err());
    let config = app.world().get_resource::<MissingConfig>();
    assert!(config.is_none());
}

#[test]
fn test_load_invalid_yaml() {
    let test_dir = get_test_dir();
    let _config_path = create_test_config_file(
        "invalid_yaml_config.yaml",
        "invalid: yaml: content:\n  - this is bad\n  missing bracket",
    );

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(test_dir.path()).unwrap();

    let mut app = App::new();
    let result: Result<Result<(), _>, _> = app.world_mut().run_system_once(load_resource_from_config_file::<InvalidYamlConfig>);

    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok());
    let inner_result = result.unwrap();
    assert!(inner_result.is_err());
    let config = app.world().get_resource::<InvalidYamlConfig>();
    assert!(config.is_none());
}

#[test]
fn test_load_with_env_override() {
    let test_dir = get_test_dir();
    let _config_path = create_test_config_file(
        "env_override_config.yaml",
        "value: 42\nname: test\n",
    );

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(test_dir.path()).unwrap();

    // Set environment variable override
    unsafe { std::env::set_var("CONFIG_EnvOverrideConfig", r#"{"value": 100}"#); }

    let mut app = App::new();
    let result: Result<Result<(), _>, _> = app.world_mut().run_system_once(load_resource_from_config_file::<EnvOverrideConfig>);

    unsafe { std::env::remove_var("CONFIG_EnvOverrideConfig"); }
    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok());
    let inner_result = result.unwrap();
    assert!(inner_result.is_ok());

    let config = app.world().get_resource::<EnvOverrideConfig>();
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.value, 100); // Overridden value
    assert_eq!(config.name, "test"); // Original value
}

#[test]
fn test_load_with_invalid_env_override() {
    let test_dir = get_test_dir();
    let _config_path = create_test_config_file(
        "invalid_env_override_config.yaml",
        "value: 42\nname: test\n",
    );

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(test_dir.path()).unwrap();

    // Set invalid JSON in environment variable
    unsafe { std::env::set_var("CONFIG_InvalidEnvOverrideConfig", r#"{"value": invalid json}"#); }

    let mut app = App::new();
    let result: Result<Result<(), _>, _> = app.world_mut().run_system_once(load_resource_from_config_file::<InvalidEnvOverrideConfig>);

    unsafe { std::env::remove_var("CONFIG_InvalidEnvOverrideConfig"); }
    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok());
    let inner_result = result.unwrap();
    assert!(inner_result.is_err());
    let config = app.world().get_resource::<InvalidEnvOverrideConfig>();
    assert!(config.is_none());
}
