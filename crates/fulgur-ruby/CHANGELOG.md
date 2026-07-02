# Changelog

All notable changes to the `fulgur` gem will be documented here.

## [Unreleased]

## [0.22.0](https://github.com/fulgur-rs/fulgur/compare/fulgur-ruby-v0.21.0...fulgur-ruby-v0.22.0) - 2026-07-02

### Added

- *(fulgur-ruby)* add render_html_to_file + integration specs
- *(fulgur-ruby)* release GVL during render_html
- *(fulgur-ruby)* add Pdf#write_to_path + #write_to_io (64KB chunked, binmode)
- *(fulgur-ruby)* add Pdf result object (to_s/bytesize/to_base64/to_data_uri) + render_html
- *(fulgur-ruby)* add Engine + EngineBuilder (kwargs + chain)
- *(fulgur-ruby)* add AssetBundle wrapper + long/short aliases
- *(fulgur-ruby)* add Margin wrapper (positional + kwargs + factory)
- *(fulgur-ruby)* add PageSize wrapper (A4/LETTER/A3 + custom + landscape)
- *(fulgur-ruby)* add error mapping (Fulgur::{Error,RenderError,AssetError} + Errno::ENOENT)
- *(fulgur-ruby)* scaffold gem + crate skeleton

### Fixed

- *(fulgur-ruby)* align Gemfile.lock fulgur path-gem to 0.10.0
- *(bindings)* handle fulgur::Error::Other in pyfulgur and fulgur-ruby
- *(fulgur-ruby)* strip rb_sys dep from native gem spec
- address CodeRabbit/Devin review feedback on PR #103
- *(fulgur-ruby)* use fulgur::asset::AssetBundle full path
- *(fulgur-ruby)* single-platform cross_platform + gemspec injection
- *(fulgur-ruby)* make cross-compile work in cross-gem-action mount
- *(fulgur-ruby)* use RbSys::ExtensionTask for cross-compile
- *(fulgur-ruby)* address CodeRabbit re-review
- *(fulgur-ruby)* address AI review feedback (coderabbit + devin + gemini)
- *(fulgur-ruby)* address Task 1 review feedback
- address AI review feedback on placeholder packages

### Other

- sync auxiliary version files to 0.21.0
- release v0.21.0
- v0.20.0
- resolve conflict with main; update multicol_column_rule_renders to render()
- update all call sites to new render API names
- v0.18.0
- v0.17.0
- v0.16.0
- *(deps)* bump rb_sys
- v0.15.0
- v0.14.0
- v0.13.0
- v0.12.0
- v0.11.0
- v0.10.0
- v0.9.0
- Merge pull request #231 from fulgur-rs/ci/bindings-check
- migrate remaining magnus deprecations in error.rs and lib.rs
- migrate to magnus 0.8 Ruby:: handle pattern
- v0.8.0
- v0.7.0
- v0.6.0
- v0.5.14
- v0.5.13
- *(deps)* bump magnus from 0.7.1 to 0.8.2
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
- v0.5.1
- v0.5.0
- *(fulgur-ruby)* loosen required_ruby_version to 3.1.0
- *(fulgur-ruby)* clarify write_to_path description (no binmode concept)
- *(fulgur-ruby)* add README + CHANGELOG
- add not-available note above planned API examples
- add placeholder packages for PyPI (pyfulgur) and RubyGems (fulgur)

## [0.21.0](https://github.com/fulgur-rs/fulgur/compare/fulgur-ruby-v0.20.0...fulgur-ruby-v0.21.0) - 2026-07-02

### Added

- *(fulgur-ruby)* add render_html_to_file + integration specs
- *(fulgur-ruby)* release GVL during render_html
- *(fulgur-ruby)* add Pdf#write_to_path + #write_to_io (64KB chunked, binmode)
- *(fulgur-ruby)* add Pdf result object (to_s/bytesize/to_base64/to_data_uri) + render_html
- *(fulgur-ruby)* add Engine + EngineBuilder (kwargs + chain)
- *(fulgur-ruby)* add AssetBundle wrapper + long/short aliases
- *(fulgur-ruby)* add Margin wrapper (positional + kwargs + factory)
- *(fulgur-ruby)* add PageSize wrapper (A4/LETTER/A3 + custom + landscape)
- *(fulgur-ruby)* add error mapping (Fulgur::{Error,RenderError,AssetError} + Errno::ENOENT)
- *(fulgur-ruby)* scaffold gem + crate skeleton

### Fixed

- *(fulgur-ruby)* align Gemfile.lock fulgur path-gem to 0.10.0
- *(bindings)* handle fulgur::Error::Other in pyfulgur and fulgur-ruby
- *(fulgur-ruby)* strip rb_sys dep from native gem spec
- address CodeRabbit/Devin review feedback on PR #103
- *(fulgur-ruby)* use fulgur::asset::AssetBundle full path
- *(fulgur-ruby)* single-platform cross_platform + gemspec injection
- *(fulgur-ruby)* make cross-compile work in cross-gem-action mount
- *(fulgur-ruby)* use RbSys::ExtensionTask for cross-compile
- *(fulgur-ruby)* address CodeRabbit re-review
- *(fulgur-ruby)* address AI review feedback (coderabbit + devin + gemini)
- *(fulgur-ruby)* address Task 1 review feedback
- address AI review feedback on placeholder packages

### Other

- v0.20.0
- resolve conflict with main; update multicol_column_rule_renders to render()
- update all call sites to new render API names
- v0.18.0
- v0.17.0
- v0.16.0
- *(deps)* bump rb_sys
- v0.15.0
- v0.14.0
- v0.13.0
- v0.12.0
- v0.11.0
- v0.10.0
- v0.9.0
- Merge pull request #231 from fulgur-rs/ci/bindings-check
- migrate remaining magnus deprecations in error.rs and lib.rs
- migrate to magnus 0.8 Ruby:: handle pattern
- v0.8.0
- v0.7.0
- v0.6.0
- v0.5.14
- v0.5.13
- *(deps)* bump magnus from 0.7.1 to 0.8.2
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
- v0.5.1
- v0.5.0
- *(fulgur-ruby)* loosen required_ruby_version to 3.1.0
- *(fulgur-ruby)* clarify write_to_path description (no binmode concept)
- *(fulgur-ruby)* add README + CHANGELOG
- add not-available note above planned API examples
- add placeholder packages for PyPI (pyfulgur) and RubyGems (fulgur)

## [0.0.1] - 2026-04-17

Initial Ruby binding for fulgur.

### Added

- `Fulgur::Engine` (kwargs constructor + builder chain)
- `Fulgur::EngineBuilder` for reusable engine construction
- `Fulgur::AssetBundle` with long (`add_*`) and short (`css`, `font_file`, etc.) aliases
- `Fulgur::PageSize` with `A4` / `LETTER` / `A3` constants and `.custom(w_mm, h_mm)`; accepts `Symbol`, `String`, or class constants as input
- `Fulgur::Margin` with CSS-style positional args, keyword args, and `.uniform` / `.symmetric` factories
- `Fulgur::Pdf` result object: `#to_s` (ASCII-8BIT), `#to_base64`, `#to_data_uri`, `#write_to_path`, `#write_to_io` (64 KiB chunked, binmode-guaranteed), `#bytesize`
- `Engine#render_html` and `Engine#render_html_to_file` release the GVL during the Rust render call
- Error hierarchy: `Fulgur::Error` / `Fulgur::RenderError` / `Fulgur::AssetError`, plus standard `ArgumentError` / `Errno::ENOENT`
- Ruby 3.3+ support

### Known Limitations

- Precompiled gems / RubyGems publish automation are tracked separately (fulgur-qyf) and not yet in place; gems must be built from source for now
- Streaming renderer: Krilla emits bytes at the end of rendering, so `#write_to_io` chunks a completed buffer rather than streaming during layout
- No Ractor safety analysis yet
