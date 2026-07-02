# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.21.0](https://github.com/fulgur-rs/fulgur/compare/fulgur-wasm-v0.20.0...fulgur-wasm-v0.21.0) - 2026-07-02

### Added

- *(fulgur-wasm)* add Engine.configure with POJO options (B-3c)
- *(fulgur-wasm)* add Engine.add_css and Engine.add_image (B-3a)
- *(fulgur-wasm)* add Engine builder mirror with add_font
- *(fulgur-wasm)* add wasm-bindgen wrapper crate + browser demo (B-1)

### Fixed

- *(release)* bump fulgur-wasm to 0.6.0 + add to release-prepare sed list
- *(fulgur-wasm)* switch configure to JSON.stringify path + add wasm tests

### Other

- v0.20.0
- resolve conflict with main; update multicol_column_rule_renders to render()
- fix stale render_html references in error messages, expect strings, and doc comments
- update all call sites to new render API names
- *(wasm)* use double-quoted strings in wasm-opt array for Cargo.toml consistency
- *(wasm)* cut fulgur-wasm bundle 37MB->7MB via size-optimised build
- v0.18.0
- v0.17.0
- v0.16.0
- v0.15.0
- v0.14.0
- v0.13.0
- v0.12.0
- v0.11.0
- v0.10.0
- v0.9.0
- v0.8.0
- v0.7.0
- *(fulgur-wasm)* refresh wasm_tests comment after JSON.stringify switch
- *(fulgur-wasm)* cargo fmt
- *(fulgur-wasm)* add failing tests for Engine.configure (B-3c)
