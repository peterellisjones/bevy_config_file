# Multi-Format Config File Support — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add YAML, JSON, and RON config file support with feature flags, compile-time validation, and extension-based format detection.

**Architecture:** Format is inferred from `ConfigFile::PATH` file extension. Each format is gated behind a Cargo feature (`yaml` default, `json`, `ron`). A const fn validates the extension against enabled features at compile time. Env overrides remain JSON-based regardless of config format.

**Tech Stack:** serde_yml (replaces deprecated serde_yaml), serde_json (already present), ron (new optional dep)

---

### Task 1: Update Cargo.toml

**Files:**
- Modify: `Cargo.toml`

**Step 1: Update features and dependencies**

Replace the entire `[features]` and `[dependencies]` sections:

```toml
[features]
default = ["yaml", "logging"]
logging = []
yaml = ["dep:serde_yml"]
json = []
ron = ["dep:ron"]

[dependencies]
bevy = { version = "^0.18.0", default-features = false, features = ["bevy_log"] }
serde = { version = "1.0", features = ["derive"] }
serde_yml = { version = "0.0.12", optional = true }
serde_json = "1.0"
ron = { version = "0.8", optional = true }
```

Also update description and keywords:
```toml
description = "A Bevy plugin for loading configuration from YAML, JSON, or RON files with environment variable overrides"
keywords = ["bevy", "config", "yaml", "json", "gamedev"]
```

Note: `keywords` max is 5, so we drop "plugin" and add "json". RON is less common so omit it from keywords.

**Step 2: Verify compilation**

Run: `cargo check --features yaml,json,ron`
Expected: Compiles (unused dep warnings for ron are fine at this stage)

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: add feature flags and dependencies for multi-format support"
```

---

### Task 2: Add const format validation and update ConfigFile trait

**Files:**
- Modify: `src/lib.rs:114-120` (ConfigFile trait)

**Step 1: Add the const validation function**

Add this above the `ConfigFile` trait (before line 114):

```rust
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
```

**Step 2: Update the ConfigFile trait**

Replace the trait definition (lines 114-120) with:

```rust
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
```

**Step 3: Force evaluation in config_file_plugin**

In `config_file_plugin` (line 168, just before `app.register_type`), add:

```rust
    // Force compile-time evaluation of format validation
    let _ = T::_FORMAT_CHECK;
```

**Step 4: Verify compilation**

Run: `cargo check --features yaml`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add src/lib.rs
git commit -m "feat: add compile-time config format validation"
```

---

### Task 3: Update LoadConfigError

**Files:**
- Modify: `src/lib.rs:65-94` (LoadConfigError enum + impls)

**Step 1: Replace the LoadConfigError enum**

Replace lines 65-74 with:

```rust
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
```

**Step 2: Update the Display impl**

Replace lines 76-84 with:

```rust
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
```

**Step 3: Update the Error impl**

Replace lines 86-94 with:

```rust
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
```

**Step 4: Verify compilation**

Run: `cargo check --features yaml`
Expected: Compiles (with possible warning about unused `serde_yaml` import — that's fixed in Task 4)

**Step 5: Commit**

```bash
git add src/lib.rs
git commit -m "feat: add feature-gated error variants for multi-format support"
```

---

### Task 4: Refactor load_config_file for format dispatch

**Files:**
- Modify: `src/lib.rs:60-63` (imports)
- Modify: `src/lib.rs:288-339` (load_config_file function)

**Step 1: Update imports**

Replace lines 60-63:

```rust
use bevy::{prelude::*, reflect::GetTypeRegistration};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{env, fs};
```

No change to the imports themselves — `serde_yaml` was used inline (as `serde_yaml::from_str`), not imported. Remove any `use serde_yaml` if present. The new crates (`serde_yml`, `ron`) will be used inline too.

**Step 2: Replace load_config_file function**

Replace lines 288-339 with:

```rust
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
```

**Step 3: Update function doc comment**

Replace the doc comment for `load_config_file` (lines 232-287). Change references from "YAML file" to "config file". Key changes:
- Line 232: `/// Loads configuration from a file with optional environment variable overrides.`
- Line 234-235: `/// 1. Loads the base configuration from the file specified in T::PATH`
- Line 236: Remove "YAML" from step 1 description
- Add: `/// The file format is determined by the file extension (.yaml/.yml, .json, .ron).`
- Error list: Add RON and UnsupportedFormat entries

**Step 4: Run tests**

Run: `cargo test --features yaml`
Expected: All 5 existing tests pass

**Step 5: Commit**

```bash
git add src/lib.rs
git commit -m "feat: implement format dispatch in load_config_file"
```

---

### Task 5: Gate existing YAML tests and add JSON tests

**Files:**
- Modify: `tests/integration_tests.rs`

**Step 1: Wrap existing YAML test config and tests in a feature-gated module**

Wrap `TestConfig`, its `ConfigFile` impl, and all 5 existing test functions (lines 16-152) in:

```rust
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
            None,
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
                assert_eq!(config.value, 100);
                assert_eq!(config.name, "test");
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
```

**Step 2: Add JSON tests**

After the yaml_tests module, add:

```rust
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
}
```

**Step 3: Run YAML and JSON tests**

Run: `cargo test --features yaml,json`
Expected: All tests pass (5 YAML + 4 JSON)

**Step 4: Commit**

```bash
git add tests/integration_tests.rs
git commit -m "test: add JSON format tests and gate YAML tests behind feature"
```

---

### Task 6: Add RON tests and unsupported format test

**Files:**
- Modify: `tests/integration_tests.rs`

**Step 1: Add RON tests**

After the json_tests module, add:

```rust
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
}
```

**Step 2: Add unsupported format test**

After the ron_tests module, add. This uses `load_config_file` directly (not via the Bevy system) so the const validation isn't triggered:

```rust
mod unsupported_format_tests {
    use super::*;
    use bevy_config_file::load_config_file;

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
        assert!(result.is_err());

        std::env::set_current_dir(original_dir).unwrap();
    }
}
```

**Step 3: Run all tests**

Run: `cargo test --features yaml,json,ron`
Expected: All tests pass (5 YAML + 4 JSON + 4 RON + 1 unsupported)

**Step 4: Commit**

```bash
git add tests/integration_tests.rs
git commit -m "test: add RON format tests and unsupported format test"
```

---

### Task 7: Update module docs and README

**Files:**
- Modify: `src/lib.rs:1-58` (module doc comment)
- Modify: `README.md`
- Modify: `Cargo.toml` (already done in Task 1, verify)

**Step 1: Update module doc comment in lib.rs**

Replace lines 1-58 with updated docs. Key changes:
- Title: "A Bevy plugin for loading configuration from YAML, JSON, or RON files..."
- Features list: mention all three formats
- Cargo Features: document `yaml`, `json`, `ron` features
- Quick Start: keep YAML example (it's the default)
- Add a section showing JSON and RON examples
- Update the ConfigFile trait doc example to mention format detection from extension

**Step 2: Update README.md**

Key changes:
- Title/description: mention all three formats
- Add "Supported Formats" section with examples for each
- Update "Cargo Features" section to document yaml/json/ron
- Update installation to show feature selection
- Keep env override docs (note they're always JSON)

**Step 3: Verify docs compile**

Run: `cargo doc --features yaml,json,ron --no-deps`
Expected: Docs build without warnings

**Step 4: Commit**

```bash
git add src/lib.rs README.md
git commit -m "docs: update documentation for multi-format support"
```

---

### Task 8: Final verification

**Step 1: Test with all features**

Run: `cargo test --features yaml,json,ron`
Expected: All tests pass

**Step 2: Test with only yaml (default user experience)**

Run: `cargo test --features yaml`
Expected: YAML tests pass, JSON/RON tests skipped

**Step 3: Test with only json**

Run: `cargo test --no-default-features --features json`
Expected: JSON tests pass, YAML/RON tests skipped

**Step 4: Test with only ron**

Run: `cargo test --no-default-features --features ron`
Expected: RON tests pass, YAML/JSON tests skipped

**Step 5: Test default features (yaml + logging)**

Run: `cargo test`
Expected: YAML tests pass (default features)

**Step 6: Verify clippy**

Run: `cargo clippy --features yaml,json,ron -- -D warnings`
Expected: No warnings

**Step 7: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix: address clippy warnings"
```
