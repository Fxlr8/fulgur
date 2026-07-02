# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.22.0](https://github.com/fulgur-rs/fulgur/compare/pyfulgur-v0.21.0...pyfulgur-v0.22.0) - 2026-07-02

### Added

- *(pyfulgur)* add type stubs (__init__.pyi) for public API
- *(pyfulgur)* enable abi3-py39 for single wheel across Python 3.9+
- *(pyfulgur)* add __version__ and integration tests
- *(pyfulgur)* add Engine(**kwargs) Pythonic constructor
- *(pyfulgur)* add Engine.render_html_to_file
- *(pyfulgur)* add Engine.render_html with GIL release
- *(pyfulgur)* add EngineBuilder with chainable config methods
- *(pyfulgur)* add RenderError and map_fulgur_error helper
- *(pyfulgur)* add AssetBundle with css/font/image registration
- *(pyfulgur)* add Margin class with uniform/symmetric/uniform_mm
- *(pyfulgur)* add PageSize class with A4/LETTER/A3 + custom/landscape
- *(pyfulgur)* wire PyO3 extension crate into workspace

### Fixed

- revert pyfulgur doc example to render_html_to_file; clarify render_template migration note
- *(deps)* bump pyo3 to 0.29 and smallbitvec to 2.6.1 for security advisories
- *(bindings)* handle fulgur::Error::Other in pyfulgur and fulgur-ruby
- address coderabbit review feedback
- address AI review feedback on placeholder packages
- use license table syntax for PyPI compatibility

### Other

- sync auxiliary version files to 0.21.0
- release v0.21.0
- v0.20.0
- resolve conflict with main; update multicol_column_rule_renders to render()
- update all call sites to new render API names
- v0.18.0
- v0.17.0
- v0.16.0
- v0.15.0
- v0.14.0
- v0.13.0
- *(pyfulgur)* rephrase EngineBuilder single-use note
- *(pyfulgur)* mirror docstrings into __init__.pyi for mkdocstrings
- v0.12.0
- *(pyfulgur)* note PEP 561 type stubs in README
- *(pyfulgur)* add Google-style docstrings via Rust /// comments
- *(pyfulgur)* switch to mixed layout for PEP 561 stubs
- v0.11.0
- v0.10.0
- v0.9.0
- v0.8.0
- v0.7.0
- v0.6.0
- v0.5.14
- v0.5.13
- v0.5.12
- v0.5.11
- v0.5.10
- v0.5.9
- v0.5.8
- v0.5.7
- v0.5.6
- v0.5.5
- v0.5.4
- v0.5.3
- Merge pull request #109 from fulgur-rs/docs/add-cla
- v0.5.2
- *(pyfulgur)* bump minimum maturin to 1.9.4 for pyo3 0.28
- *(pyfulgur)* upgrade pyo3 0.22 → 0.28
- v0.5.1
- *(pyfulgur)* clarify blitz noise fires on recoverable parse errors
- clarify fd 1 policy per crate, document stdout noise in pyfulgur
- *(pyfulgur)* sync __version__ assertion to pyproject.toml dynamically
- v0.5.0
- *(pyfulgur)* fix fmt + silence pyo3 0.22 macro lints
- *(pyfulgur)* update README for MVP release and CHANGELOG
- *(pyfulgur)* switch to maturin, add smoke test
- add not-available note above planned API examples
- add placeholder packages for PyPI (pyfulgur) and RubyGems (fulgur)

## [0.21.0](https://github.com/fulgur-rs/fulgur/compare/pyfulgur-v0.20.0...pyfulgur-v0.21.0) - 2026-07-02

### Added

- *(pyfulgur)* add type stubs (__init__.pyi) for public API
- *(pyfulgur)* enable abi3-py39 for single wheel across Python 3.9+
- *(pyfulgur)* add __version__ and integration tests
- *(pyfulgur)* add Engine(**kwargs) Pythonic constructor
- *(pyfulgur)* add Engine.render_html_to_file
- *(pyfulgur)* add Engine.render_html with GIL release
- *(pyfulgur)* add EngineBuilder with chainable config methods
- *(pyfulgur)* add RenderError and map_fulgur_error helper
- *(pyfulgur)* add AssetBundle with css/font/image registration
- *(pyfulgur)* add Margin class with uniform/symmetric/uniform_mm
- *(pyfulgur)* add PageSize class with A4/LETTER/A3 + custom/landscape
- *(pyfulgur)* wire PyO3 extension crate into workspace

### Fixed

- revert pyfulgur doc example to render_html_to_file; clarify render_template migration note
- *(deps)* bump pyo3 to 0.29 and smallbitvec to 2.6.1 for security advisories
- *(bindings)* handle fulgur::Error::Other in pyfulgur and fulgur-ruby
- address coderabbit review feedback
- address AI review feedback on placeholder packages
- use license table syntax for PyPI compatibility

### Other

- v0.20.0
- resolve conflict with main; update multicol_column_rule_renders to render()
- update all call sites to new render API names
- v0.18.0
- v0.17.0
- v0.16.0
- v0.15.0
- v0.14.0
- v0.13.0
- *(pyfulgur)* rephrase EngineBuilder single-use note
- *(pyfulgur)* mirror docstrings into __init__.pyi for mkdocstrings
- v0.12.0
- *(pyfulgur)* note PEP 561 type stubs in README
- *(pyfulgur)* add Google-style docstrings via Rust /// comments
- *(pyfulgur)* switch to mixed layout for PEP 561 stubs
- v0.11.0
- v0.10.0
- v0.9.0
- v0.8.0
- v0.7.0
- v0.6.0
- v0.5.14
- v0.5.13
- v0.5.12
- v0.5.11
- v0.5.10
- v0.5.9
- v0.5.8
- v0.5.7
- v0.5.6
- v0.5.5
- v0.5.4
- v0.5.3
- Merge pull request #109 from fulgur-rs/docs/add-cla
- v0.5.2
- *(pyfulgur)* bump minimum maturin to 1.9.4 for pyo3 0.28
- *(pyfulgur)* upgrade pyo3 0.22 → 0.28
- v0.5.1
- *(pyfulgur)* clarify blitz noise fires on recoverable parse errors
- clarify fd 1 policy per crate, document stdout noise in pyfulgur
- *(pyfulgur)* sync __version__ assertion to pyproject.toml dynamically
- v0.5.0
- *(pyfulgur)* fix fmt + silence pyo3 0.22 macro lints
- *(pyfulgur)* update README for MVP release and CHANGELOG
- *(pyfulgur)* switch to maturin, add smoke test
- add not-available note above planned API examples
- add placeholder packages for PyPI (pyfulgur) and RubyGems (fulgur)
