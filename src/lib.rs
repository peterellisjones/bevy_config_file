//! A Bevy plugin for loading configuration from YAML, JSON, or RON files with environment variable overrides.
//!
//! This crate provides a simple way to load configuration files into Bevy resources,
//! with support for runtime overrides via environment variables. This is useful for game settings,
//! input mappings, and other configurable parameters.
//!
//! # Features
//!
//! - Load configuration from YAML, JSON, or RON files at startup
//! - Format is detected automatically from the file extension in [`ConfigFile::PATH`]
//! - Override configuration values using environment variables (always JSON)
//! - Automatic resource registration with Bevy's reflection system
//! - Type-safe configuration with serde deserialization
//!
//! ## Cargo Features
//!
//! | Feature   | Default | Description                              |
//! |-----------|---------|------------------------------------------|
//! | `yaml`    | yes     | YAML config support (`.yaml`, `.yml`)    |
//! | `json`    | no      | JSON config support (`.json`)            |
//! | `ron`     | no      | RON config support (`.ron`)              |
//! | `logging` | yes     | Log config loading events                |
//!
//! At least one format feature must be enabled. To use multiple formats:
//! ```toml
//! bevy_config_file = { version = "0.1", features = ["yaml", "json", "ron"] }
//! ```
//!
//! # Quick Start
//!
//! The format is detected from the file extension in `ConfigFile::PATH`.
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
//! # Config File Formats
//!
//! The same struct can be loaded from any supported format by changing the file extension:
//!
//! **YAML** (`camera_settings.yaml`):
//! ```yaml
//! pan_speed: 1000.0
//! zoom_speed: 1.0
//! ```
//!
//! **JSON** (`camera_settings.json`):
//! ```json
//! { "pan_speed": 1000.0, "zoom_speed": 1.0 }
//! ```
//!
//! **RON** (`camera_settings.ron`):
//! ```ron
//! (pan_speed: 1000.0, zoom_speed: 1.0)
//! ```
//!
//! # Environment Variable Overrides
//!
//! You can override configuration values at runtime using environment variables.
//! Overrides are **always JSON**, regardless of the config file format:
//!
//! ```bash
//! CONFIG_CameraSettings='{"pan_speed": 2000.0}' ./game
//! ```
//!
//! The environment variable name is `CONFIG_{TypeName}` where `TypeName` is the last
//! component of the type's fully qualified name.

#[cfg(not(any(feature = "yaml", feature = "json", feature = "ron")))]
compile_error!(
    "At least one config format feature must be enabled (yaml, json, or ron). \
     Enable a format in your Cargo.toml: features = [\"yaml\"]"
);

use bevy::{prelude::*, reflect::GetTypeRegistration};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{env, fs};

/// Errors that can occur when loading configuration files.
#[derive(Debug)]
pub enum LoadConfigError {
    /// Error parsing YAML content
    #[cfg(feature = "yaml")]
    Yaml(serde_yml::Error),
    /// Error parsing or serializing JSON content
    Json(serde_json::Error),
    /// Error parsing RON content
    #[cfg(feature = "ron")]
    Ron(ron::error::SpannedError),
    /// Error reading the configuration file
    Io(std::io::Error),
    /// The file extension is not a supported config format
    UnsupportedFormat(String),
}

impl std::fmt::Display for LoadConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "yaml")]
            LoadConfigError::Yaml(e) => write!(f, "YAML parsing error: {}", e),
            LoadConfigError::Json(e) => write!(f, "JSON parsing error: {}", e),
            #[cfg(feature = "ron")]
            LoadConfigError::Ron(e) => write!(f, "RON parsing error: {}", e),
            LoadConfigError::Io(e) => write!(f, "IO error: {}", e),
            LoadConfigError::UnsupportedFormat(ext) => {
                write!(f, "Unsupported config file format: .{}", ext)
            }
        }
    }
}

impl std::error::Error for LoadConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(feature = "yaml")]
            LoadConfigError::Yaml(e) => Some(e),
            LoadConfigError::Json(e) => Some(e),
            #[cfg(feature = "ron")]
            LoadConfigError::Ron(e) => Some(e),
            LoadConfigError::Io(e) => Some(e),
            LoadConfigError::UnsupportedFormat(_) => None,
        }
    }
}

/// Compares two byte slices in a const context.
const fn bytes_equal(a: &[u8], a_start: usize, a_len: usize, b: &[u8]) -> bool {
    if a_len != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a_len {
        if a[a_start + i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Validates that the file extension in a config path corresponds to an enabled format feature.
///
/// This is evaluated at compile time via the `ConfigFile::_FORMAT_CHECK` associated constant.
/// If the extension requires a feature that isn't enabled, compilation fails with a clear message.
const fn validate_config_format(path: &str) {
    let bytes = path.as_bytes();
    let len = bytes.len();

    // Find last '.'
    let mut dot_pos = len;
    let mut i = len;
    while i > 0 {
        i -= 1;
        if bytes[i] == b'.' {
            dot_pos = i;
            break;
        }
    }

    if dot_pos == len {
        panic!("Config file path must have a file extension (.yaml, .yml, .json, or .ron)");
    }

    let ext_start = dot_pos + 1;
    let ext_len = len - ext_start;

    // Check yaml/yml
    if bytes_equal(bytes, ext_start, ext_len, b"yaml")
        || bytes_equal(bytes, ext_start, ext_len, b"yml")
    {
        if !cfg!(feature = "yaml") {
            panic!("YAML config requires the 'yaml' feature. Add features = [\"yaml\"] to your bevy_config_file dependency.");
        }
        return;
    }

    // Check json
    if bytes_equal(bytes, ext_start, ext_len, b"json") {
        if !cfg!(feature = "json") {
            panic!("JSON config requires the 'json' feature. Add features = [\"json\"] to your bevy_config_file dependency.");
        }
        return;
    }

    // Check ron
    if bytes_equal(bytes, ext_start, ext_len, b"ron") {
        if !cfg!(feature = "ron") {
            panic!("RON config requires the 'ron' feature. Add features = [\"ron\"] to your bevy_config_file dependency.");
        }
        return;
    }

    panic!("Unsupported config file extension. Supported: .yaml, .yml, .json, .ron");
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
    /// such as `"assets/config/settings.yaml"`.
    ///
    /// The file extension determines the format: `.yaml`/`.yml`, `.json`, or `.ron`.
    /// The corresponding feature must be enabled.
    const PATH: &'static str;

    /// Compile-time validation that the file extension matches an enabled format feature.
    /// Do not override this.
    const _FORMAT_CHECK: () = validate_config_format(Self::PATH);
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
/// - The file content is invalid for the detected format
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
    // Force compile-time evaluation of format validation
    #[allow(clippy::let_unit_value)]
    let _ = T::_FORMAT_CHECK;

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
    match load_config_file::<T>() {
        Ok(config) => {
            #[cfg(feature = "logging")]
            info!("loaded config from {}", T::PATH);
            commands.insert_resource(config);
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

/// Loads configuration from a file with optional environment variable overrides.
///
/// The format is determined by the file extension: `.yaml`/`.yml`, `.json`, or `.ron`.
/// The corresponding feature must be enabled.
///
/// This function performs a two-stage loading process:
/// 1. Loads the base configuration from the file specified in `T::PATH`
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
/// - The JSON content is invalid (`LoadConfigError::Json`)
/// - The RON content is invalid (`LoadConfigError::Ron`)
/// - The file extension is not supported (`LoadConfigError::UnsupportedFormat`)
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

    // Load file content
    let content = fs::read_to_string(config_path).map_err(LoadConfigError::Io)?;

    // Parse based on file extension
    let ext = config_path.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let base_config: T = match ext {
        #[cfg(feature = "yaml")]
        "yaml" | "yml" => serde_yml::from_str(&content).map_err(LoadConfigError::Yaml)?,
        #[cfg(feature = "json")]
        "json" => serde_json::from_str(&content).map_err(LoadConfigError::Json)?,
        #[cfg(feature = "ron")]
        "ron" => ron::from_str(&content).map_err(LoadConfigError::Ron)?,
        other => return Err(LoadConfigError::UnsupportedFormat(other.to_string())),
    };

    // Apply environment variable overrides (always JSON)
    let type_name = std::any::type_name::<T>()
        .split("::")
        .last()
        .expect("type name should have at least one component");
    let env_var_name = format!("CONFIG_{type_name}");

    if let Ok(json_override) = env::var(&env_var_name) {
        let json_override: JsonValue =
            serde_json::from_str(&json_override).map_err(LoadConfigError::Json)?;

        let mut base_json =
            serde_json::to_value(&base_config).map_err(LoadConfigError::Json)?;

        if let (JsonValue::Object(base_map), JsonValue::Object(override_map)) =
            (&mut base_json, json_override)
        {
            for (key, value) in override_map {
                base_map.insert(key, value);
            }
        }

        serde_json::from_value(base_json).map_err(LoadConfigError::Json)
    } else {
        Ok(base_config)
    }
}
