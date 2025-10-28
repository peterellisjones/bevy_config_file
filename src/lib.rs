//! A Bevy plugin for loading configuration from YAML files with environment variable overrides.
//!
//! This crate provides a simple way to load configuration from YAML files into Bevy resources,
//! with support for runtime overrides via environment variables. This is useful for game settings,
//! input mappings, and other configurable parameters.
//!
//! # Features
//!
//! - Load configuration from YAML files at startup
//! - Override configuration values using environment variables with JSON
//! - Automatic resource registration with Bevy's reflection system
//! - Type-safe configuration with serde deserialization
//!
//! # Quick Start
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use bevy_config_file::{ConfigFile, config_file_plugin};
//! # use serde::{Deserialize, Serialize};
//! #
//! #[derive(Resource, Reflect, Debug, Serialize, Deserialize)]
//! #[reflect(Resource)]
//! pub struct CameraSettings {
//!     pub pan_speed: f32,
//!     pub zoom_speed: f32,
//! }
//!
//! impl ConfigFile for CameraSettings {
//!     const PATH: &'static str = "assets/config/camera_settings.yaml";
//! }
//!
//! # fn main() {
//! App::new()
//!     .add_plugins(config_file_plugin::<CameraSettings>)
//!     .run();
//! # }
//! ```
//!
//! # Environment Variable Overrides
//!
//! You can override configuration values at runtime using environment variables:
//!
//! ```bash
//! CONFIG_CameraSettings='{"pan_speed": 2000.0}' ./game
//! ```
//!
//! The environment variable name is `CONFIG_{TypeName}` where `TypeName` is the last
//! component of the type's fully qualified name.

use bevy::{prelude::*, reflect::GetTypeRegistration};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{env, fs};

/// Errors that can occur when loading configuration files.
#[allow(dead_code)]
#[derive(Debug)]
pub enum LoadConfigError {
    /// Error parsing YAML content
    Yaml(serde_yaml::Error),
    /// Error parsing or serializing JSON content
    Json(serde_json::Error),
    /// Error reading the configuration file
    Io(std::io::Error),
}

impl std::fmt::Display for LoadConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadConfigError::Yaml(e) => write!(f, "YAML parsing error: {}", e),
            LoadConfigError::Json(e) => write!(f, "JSON parsing error: {}", e),
            LoadConfigError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for LoadConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadConfigError::Yaml(e) => Some(e),
            LoadConfigError::Json(e) => Some(e),
            LoadConfigError::Io(e) => Some(e),
        }
    }
}

/// Trait for types that can be loaded from a configuration file.
///
/// Implement this trait on your configuration resource types to specify
/// the file path where the configuration should be loaded from.
///
/// # Example
///
/// ```rust
/// use bevy_config_file::ConfigFile;
///
/// struct GameSettings {
///     difficulty: String,
/// }
///
/// impl ConfigFile for GameSettings {
///     const PATH: &'static str = "assets/config/game_settings.yaml";
/// }
/// ```
pub trait ConfigFile: 'static {
    /// The file path to load the configuration from.
    ///
    /// This should typically be a path relative to your game's root directory,
    /// such as "assets/config/settings.yaml".
    const PATH: &'static str;
}

/// Creates a Bevy plugin that loads a configuration resource from a file at startup.
///
/// This function registers the type with Bevy's reflection system and adds a startup
/// system that loads the configuration from the file specified in the `ConfigFile` trait.
///
/// # Type Parameters
///
/// * `T` - The configuration type to load. Must implement `Resource`, `Deserialize`,
///   `Serialize`, `ConfigFile`, `Reflect`, and `GetTypeRegistration`.
///
/// # Panics
///
/// The startup system will panic if:
/// - The configuration file cannot be read
/// - The YAML content is invalid
/// - An environment variable override contains invalid JSON
///
/// # Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_config_file::{ConfigFile, config_file_plugin};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Resource, Reflect, Debug, Serialize, Deserialize)]
/// #[reflect(Resource)]
/// struct GameSettings {
///     volume: f32,
/// }
///
/// impl ConfigFile for GameSettings {
///     const PATH: &'static str = "assets/config/game.yaml";
/// }
///
/// App::new()
///     .add_plugins(config_file_plugin::<GameSettings>)
///     .run();
/// ```
pub fn config_file_plugin<T>(app: &mut App)
where
    T: Resource
        + for<'de> Deserialize<'de>
        + Serialize
        + ConfigFile
        + Reflect
        + GetTypeRegistration,
{
    app.register_type::<T>();
    app.add_systems(Startup, load_resource_from_config_file::<T>);
}

/// Loads a configuration resource from a file and inserts it into Bevy's ECS.
///
/// This is a lower-level function that can be called directly from a Bevy system.
/// Most users should prefer using [`config_file_plugin`] instead, which handles
/// the system registration automatically.
///
/// # Type Parameters
///
/// * `T` - The configuration type to load. Must implement `Resource`, `Deserialize`,
///   `Serialize`, and `ConfigFile`.
///
/// # Returns
///
/// * `Ok(())` - If the configuration was successfully loaded and inserted
/// * `Err(BevyError)` - If any error occurs during loading or parsing
///
/// # Errors
///
/// Returns a Bevy error if the configuration file cannot be loaded or parsed.
/// See [`load_config_file`] for details on the loading process and potential error conditions.
/// The error will be handled by Bevy's error handler (by default, this will panic).
///
/// # Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_config_file::{ConfigFile, load_resource_from_config_file};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Resource, Debug, Serialize, Deserialize)]
/// struct Settings {
///     value: i32,
/// }
///
/// impl ConfigFile for Settings {
///     const PATH: &'static str = "assets/config/settings.yaml";
/// }
///
/// fn setup(commands: Commands) -> bevy::ecs::error::Result {
///     load_resource_from_config_file::<Settings>(commands)
/// }
/// ```
pub fn load_resource_from_config_file<T>(mut commands: Commands) -> bevy::ecs::error::Result
where
    T: Resource + for<'de> Deserialize<'de> + Serialize + ConfigFile,
{
    let config_path = T::PATH;

    match load_config_file::<T>() {
        Ok(config) => {
            info!("loaded config from {}", config_path);
            commands.insert_resource(config);
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

/// Loads configuration from a YAML file with optional environment variable overrides.
///
/// This function performs a two-stage loading process:
/// 1. Loads the base configuration from the YAML file specified in `T::PATH`
/// 2. Applies any overrides from an environment variable (if present)
///
/// # Environment Variable Overrides
///
/// The environment variable name is `CONFIG_{TypeName}` where `TypeName` is the last
/// component of the type's fully qualified name. For example, for a type
/// `my_game::config::CameraSettings`, the environment variable would be
/// `CONFIG_CameraSettings`.
///
/// The environment variable should contain a JSON object with the fields to override.
/// Only top-level fields are overridden; nested objects are replaced entirely, not merged.
///
/// # Type Parameters
///
/// * `T` - The configuration type to load. Must implement `Deserialize`, `Serialize`,
///   and `ConfigFile`.
///
/// # Returns
///
/// * `Ok(T)` - The loaded and potentially overridden configuration
/// * `Err(LoadConfigError)` - If any error occurs during loading or parsing
///
/// # Errors
///
/// Returns an error if:
/// - The configuration file cannot be read (`LoadConfigError::Io`)
/// - The YAML content is invalid (`LoadConfigError::Yaml`)
/// - The environment variable contains invalid JSON (`LoadConfigError::Json`)
/// - The deserialization fails (`LoadConfigError::Json`)
///
/// # Example
///
/// ```no_run
/// use bevy_config_file::{ConfigFile, load_config_file};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize)]
/// struct AudioSettings {
///     volume: f32,
///     muted: bool,
/// }
///
/// impl ConfigFile for AudioSettings {
///     const PATH: &'static str = "assets/config/audio.yaml";
/// }
///
/// // Load configuration
/// let config = load_config_file::<AudioSettings>().expect("Failed to load config");
///
/// // Or with environment variable override:
/// // CONFIG_AudioSettings='{"volume": 0.5}' cargo run
/// ```
pub fn load_config_file<T>() -> Result<T, LoadConfigError>
where
    T: for<'de> Deserialize<'de> + Serialize + ConfigFile,
{
    let config_path = T::PATH;

    // Load base config from YAML file
    let base_config = match fs::read_to_string(config_path) {
        Ok(yaml_content) => match serde_yaml::from_str::<T>(&yaml_content) {
            Ok(config) => config,
            Err(e) => return Err(LoadConfigError::Yaml(e)),
        },
        Err(e) => return Err(LoadConfigError::Io(e)),
    };

    // Check for environment variable override (used to disable autosaves in test)
    let type_name = std::any::type_name::<T>().split("::").last().unwrap();
    let env_var_name = format!("CONFIG_{type_name}");

    if let Ok(json_override) = env::var(&env_var_name) {
        let json_override: JsonValue = match serde_json::from_str(&json_override) {
            Ok(v) => v,
            Err(e) => return Err(LoadConfigError::Json(e)),
        };

        // Convert base config to JSON value for merging
        let mut base_json = match serde_json::to_value(&base_config) {
            Ok(v) => v,
            Err(e) => return Err(LoadConfigError::Json(e)),
        };

        // Override top-level values from JSON override
        if let (JsonValue::Object(base_map), JsonValue::Object(override_map)) =
            (&mut base_json, json_override)
        {
            for (key, value) in override_map {
                base_map.insert(key, value);
            }
        }

        // Convert back to T
        match serde_json::from_value(base_json) {
            Ok(config) => Ok(config),
            Err(e) => Err(LoadConfigError::Json(e)),
        }
    } else {
        Ok(base_config)
    }
}
