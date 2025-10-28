# bevy_config_file

A simple Bevy plugin for loading configuration from YAML files with environment variable overrides.

## Features

- Load configuration from YAML files at application startup
- Override configuration values using environment variables with JSON
- Automatic resource registration with Bevy's reflection system
- Type-safe configuration with serde deserialization
- Support for any type that implements `Resource`, `Deserialize`, `Serialize`, and `Reflect`

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
bevy_config_file = { git = "https://github.com/peterellisjones/bevy_config_file" }
```

## Quick Start

1. Define your configuration struct:

```rust
use bevy::prelude::*;
use bevy_config_file::{ConfigFile, config_file_plugin};
use serde::{Deserialize, Serialize};

#[derive(Resource, Reflect, Debug, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct CameraSettings {
    pub pan_speed: f32,
    pub zoom_speed: f32,
    pub initial_height: f32,
}

impl ConfigFile for CameraSettings {
    const PATH: &'static str = "assets/config/camera_settings.yaml";
}
```

2. Create your YAML configuration file at `assets/config/camera_settings.yaml`:

```yaml
pan_speed: 1000.0
zoom_speed: 1.0
initial_height: 1000.0
```

3. Add the plugin to your Bevy app:

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(config_file_plugin::<CameraSettings>)
        .run();
}
```

4. Access the configuration in your systems:

```rust
fn camera_movement(
    settings: Res<CameraSettings>,
    // ... other system parameters
) {
    let speed = settings.pan_speed;
    // Use the configuration...
}
```

## Environment Variable Overrides

You can override configuration values at runtime using environment variables. This is particularly useful for testing or debugging.

The environment variable name follows the pattern `CONFIG_{TypeName}` where `TypeName` is the last component of your type's fully qualified name.

### Example

For a type `my_game::config::CameraSettings`, you would use:

```bash
CONFIG_CameraSettings='{"pan_speed": 2000.0}' ./my_game
```

The JSON object should contain the fields you want to override. Only top-level fields are overridden; nested objects are replaced entirely, not merged.

### Testing Use Case

This feature is especially useful in tests:

```rust
#[test]
fn test_with_custom_config() {
    std::env::set_var("CONFIG_GameSettings", r#"{"auto_save": false}"#);

    // Run your test...

    std::env::remove_var("CONFIG_GameSettings");
}
```

## Advanced Usage

### Manual Loading

If you need more control over when and how the configuration is loaded, you can use the lower-level functions:

```rust
use bevy_config_file::{load_resource_from_config_file, load_config_file};

// In a Bevy system:
fn custom_setup(mut commands: Commands) {
    load_resource_from_config_file::<MySettings>(&mut commands);
}

// Or load without inserting into ECS:
let config = load_config_file::<MySettings>().expect("Failed to load config");
```

### Multiple Configuration Files

You can load multiple configuration files by creating multiple types and adding multiple plugins:

```rust
App::new()
    .add_plugins(config_file_plugin::<CameraSettings>)
    .add_plugins(config_file_plugin::<AudioSettings>)
    .add_plugins(config_file_plugin::<InputSettings>)
    .run();
```