# Multi-Format Config File Support

## Summary

Add support for YAML, JSON, and RON config file formats, detected from file extension, with each format gated behind a Cargo feature flag. Compile-time validation ensures users enable the correct feature for their chosen format.

## Decisions

- **Format detection:** Inferred from `ConfigFile::PATH` file extension (.yaml/.yml, .json, .ron)
- **Feature flags:** Each format is optional — `yaml` (default), `json`, `ron`
- **Env overrides:** Always JSON regardless of config format (serde_json stays required)
- **Missing feature errors:** Compile-time via const fn validation
- **YAML crate:** Switch from deprecated `serde_yaml` to `serde_yml`

## Feature Flags

```toml
[features]
default = ["yaml", "logging"]
logging = ["bevy/bevy_log"]
yaml = ["dep:serde_yml"]
json = []
ron = ["dep:ron"]
```

- `yaml` is default, preserving current behavior
- `json` enables JSON config parsing (no extra dep — serde_json already required for env overrides)
- `ron` adds the `ron` crate as an optional dependency

## Dependencies

```toml
[dependencies]
serde_yml = { version = "0.0.12", optional = true }   # replaces serde_yaml
serde_json = "1.0"                                      # always required (env overrides)
ron = { version = "0.8", optional = true }              # new
```

## Error Types

```rust
pub enum LoadConfigError {
    #[cfg(feature = "yaml")]
    Yaml(serde_yml::Error),
    Json(serde_json::Error),           // always available
    #[cfg(feature = "ron")]
    Ron(ron::error::SpannedError),
    Io(std::io::Error),
    UnsupportedFormat(String),         // unrecognized extensions
}
```

## Const Format Validation

A `const fn` extracts the extension from `ConfigFile::PATH` and panics at compile time if the feature isn't enabled:

```rust
const fn validate_config_format(path: &str) {
    // byte-level extension extraction
    // check against cfg!(feature = "...") for each known extension
    // panic with clear message if feature missing
    // panic if extension is unrecognized
}

pub trait ConfigFile {
    const PATH: &'static str;
    const _FORMAT_CHECK: () = validate_config_format(Self::PATH);
}
```

Evaluation is forced in `config_file_plugin` via `let _ = T::_FORMAT_CHECK`.

## Parsing Dispatch

```rust
let config: T = match ext {
    #[cfg(feature = "yaml")]
    "yaml" | "yml" => serde_yml::from_str(&content)?,
    #[cfg(feature = "json")]
    "json" => serde_json::from_str(&content)?,
    #[cfg(feature = "ron")]
    "ron" => ron::from_str(&content)?,
    other => return Err(LoadConfigError::UnsupportedFormat(other.into())),
};
```

Env override merging remains unchanged (JSON-based, applied after initial parse).

## Tests

- Existing YAML tests gated behind `#[cfg(feature = "yaml")]`
- Mirror test patterns (valid, missing, invalid, env override) for JSON and RON
- Add unsupported format test (e.g., `.toml` extension)
- All format-specific tests gated behind their respective features

## Breaking Changes

None for default feature users. Users with `default-features = false` must now explicitly add `features = ["yaml"]` to use YAML.

## Documentation

- Update README with format examples and feature flag usage
- Update Cargo.toml keywords to include json and ron
- Document that env overrides are always JSON regardless of config format
