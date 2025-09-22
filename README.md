# mlua-stdlib

[![crates.io](https://img.shields.io/crates/v/mlua-stdlib)](https://crates.io/crates/mlua-stdlib)
[![docs.rs](https://docs.rs/mlua/badge.svg)](https://docs.rs/mlua-stdlib)
[![codecov.io](https://codecov.io/gh/mlua-rs/mlua-stdlib/graph/badge.svg?token=sgJohTeiff)](https://codecov.io/gh/mlua-rs/mlua-stdlib)

A standard library for [mlua](https://github.com/mlua-rs/mlua), providing common functionality and utilities for Lua scripting in Rust applications.

## Features

mlua-stdlib provides a collection of modules that extend Lua with useful functionality:

- **assertions** - Useful assertion functions for testing and validation
- **testing** - A testing framework with hooks and reporting
- **env** - Environment functions

With the following optional modules:
- **json** (feature) - JSON encoding/decoding
- **regex** (feature) - Regular expressions support
- **yaml** (feature) - YAML encoding/decoding

The following feature flags are passed to `mlua`, when enabled:

- `lua51`, `lua52`, `lua53`, `lua54`, `luau` - Lua version selection
- `send` - Enable `Send+Sync` support
- `vendored` - Use vendored Lua

## Documentation

The project is still in early stages, the API documentation will be published on [docs.rs](https://docs.rs/mlua-stdlib) or in the repository once stabilized.

## Examples

Check the `tests/lua/` directory for comprehensive examples of how to use each module.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
