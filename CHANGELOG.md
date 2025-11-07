# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2025-10-24

### Added
- Build script (`build.rs`) to automatically detect netmap installation and provide helpful error messages
- Better error handling and diagnostic messages for missing netmap C library
- Environment variable `NETMAP_LOCATION` support for custom netmap installations
- Comprehensive troubleshooting section in README
- Added `new()` method to `Frame` struct as an alias for `new_borrowed()` for consistency

### Changed
- **BREAKING**: Updated Rust edition from 2024 to 2021 for better compatibility with current Rust toolchains
- Updated all dependencies to latest stable versions:
  - `bitflags`: 2.0 → 2.6
  - `tokio`: 1 → 1.40
  - `thiserror`: 1.0 → 2.0
  - `criterion`: 0.4 → 0.5
  - `tempfile`: 3.3 → 3.13
  - `ctrlc`: 3.2 → 3.4
  - `polling`: 3.2 → 3.7
- Updated `netmap-min-sys` dependency to 0.2.2 to match what Cargo resolves
- Improved error messages in `Error::from` implementation
- Enhanced README with detailed installation instructions and troubleshooting

### Fixed
- Fixed compilation issues with `Frame::new()` method not being available
- Fixed `io::Error::other()` usage which is not available in current Rust versions
- Fixed edition compatibility issues that prevented compilation
- Fixed documentation examples to use correct feature flags
- Fixed ring buffer synchronization logic to prevent potential race conditions

### Security
- Updated all dependencies to latest versions to address potential security vulnerabilities
- Improved error handling to prevent panic conditions

## [0.2.1] - 2025-10-17

### Added
- Initial project structure for `netmap-rs`.
- Core Netmap abstractions: `NetmapBuilder`, `Netmap`, `TxRing`, `RxRing` (available under the `sys` feature).
- `Frame` structure for packet representation.
- Error handling types.
- Fallback mechanisms for non-Netmap platforms (basic structure).
- Tokio async support via the `tokio-async` feature.
- Example usage files in the `examples/` directory.
- `netmap-min-sys` crate for low-level FFI bindings to Netmap.

### Changed
- **Technical Consideration: Feature Flags**: The core functionality of interacting with Netmap relies on the `sys` feature flag. This flag enables the compilation of C bindings and makes types like `NetmapBuilder`, `Netmap`, `TxRing`, and `RxRing` available. Without `sys`, these types are not exported by the crate. Users consuming this crate must enable this feature in their `Cargo.toml` (e.g., `netmap-rs = { version = "...", features = ["sys"] }`) to use Netmap capabilities.
- The `tokio-async` feature enables integration with the Tokio runtime, providing `AsyncNetmapRxRing`, `AsyncNetmapTxRing`, and `TokioNetmap` types. This also requires the `sys` feature.
- Clarified in documentation (README) that `NetmapBuilder` and related types require the `sys` feature to be enabled. This addresses potential compilation errors where these types might appear undeclared if the feature is missing.
- Updated `README.md` to include troubleshooting tips for Netmap C library detection, highlighting the use of the `NETMAP_LOCATION` environment variable.
- The underlying `netmap-min-sys` dependency's build script (`build.rs`) was enhanced to more robustly use `NETMAP_LOCATION` for discovering Netmap C headers.
- `netmap-min-sys` now also has its own `README.md` and `CHANGELOG.md` for better clarity on its specific build options and changes.

### Fixed
- Resolved confusion regarding `NetmapBuilder` not being found: This was identified as an issue in how consuming crates specify dependencies. The `sys` feature flag must be enabled in the dependent crate's `Cargo.toml` to make `NetmapBuilder` and other `sys`-gated items available.