# Changelog for netmap-min-sys

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - YYYY-MM-DD

### Added
- Created this `CHANGELOG.md` file.
- Created `README.md` explaining prerequisites, build configuration using `NETMAP_LOCATION` and `DISABLE_NETMAP_KERNEL` environment variables.

### Changed
- Modified `build.rs` to explicitly pass the include path (`$NETMAP_LOCATION/include` or default `/usr/local/include`) to `bindgen` using `.clang_arg()`. This makes the discovery of Netmap headers more robust, especially for non-standard installation locations of the Netmap C library.

### Fixed
- N/A (No prior bugs addressed in this iteration for this sub-crate)

### Removed
- N/A

---
*Note: This changelog reflects the state of the sub-crate as of the current work. If this were an actual release, the version and date would be set accordingly.*
