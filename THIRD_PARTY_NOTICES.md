# Third-party notices

NxMonitor is currently a local technical-validation project. `Cargo.lock` pins the resolved software graph. This document is an initial dependency/source notice, not a complete distribution license audit. The NxMonitor source license has not yet been selected by its owner; `publish = false` prevents accidental Cargo publication.

## GPUI Kit

- Package: `gpui-kit = 0.8.0`; facade resolves the matched `gpui-pre = 0.3.9` family.
- Upstream: https://github.com/longbridge/gpui-kit
- Published metadata: https://crates.io/crates/gpui-kit/0.8.0
- Software license declared by upstream: **Apache-2.0**. See [upstream license copy](docs/licenses/GPUI-Kit-APACHE-2.0.txt).
- Source notices, transitive crate licenses, icon/font/image resources and upstream NOTICE requirements must be checked before binary distribution. No Graphy application source or textures are copied; see the separately licensed Roboto font notice below.

### Archived documentation

The archived Design Guides and Coding Guides are credited to **GPUI Kit**:

- https://gpui-kit.com/zh-CN/docs/design-guides/
- https://gpui-kit.com/zh-CN/docs/coding-guides/

The upstream [documentation notice](https://github.com/longbridge/gpui-kit/blob/main/LICENSE-DOCS.md) offers qualifying documentation prose/original illustrations under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/); code snippets remain Apache-2.0 and third-party material retains its original terms. Markdown copies remove navigation, scripts, styles and syntax-highlighting markup, preserve body/code/tables and link to remote illustrations. Original HTML is retained unmodified. This is a local reference archive, not an assertion of rights to redistribute third-party illustrations. See [local upstream documentation notice](docs/licenses/GPUI-Kit-DOCS.md).

## Intel PresentMon

- Version verified for this prototype: **2.6.0 x64**.
- Official source/release: https://github.com/GameTechDev/PresentMon/releases/tag/v2.6.0
- License: MIT, Copyright (C) 2017-2024 Intel Corporation. See [license copy](docs/licenses/PresentMon-MIT.txt).
- The helper EXE is not committed or bundled. `scripts/Get-PresentMon.ps1` downloads the official asset only on explicit invocation, validates the pinned SHA-256, and requires a valid Intel Authenticode signature.
- The capture backend uses a separate ETW console process. It does not inject a DLL, install a driver/service, change user groups or request automatic elevation.

## Graphy

Reference-only application source at `D:\Code\graphy`, MIT. No Graphy application source/shaders/textures are copied into the Rust implementation. Static visual study uses the Roboto Regular/Bold font files from its `Font/Roboto` directory, with their accompanying Apache-2.0 license retained at [assets/fonts/LICENSE-Roboto.txt](assets/fonts/LICENSE-Roboto.txt). Font binaries are unchanged. Any future substantial Graphy source reuse must retain its MIT copyright and permission notice.

## Configuration dependencies

`serde`, `serde_json`, and `tempfile` are used for settings serialization and same-directory atomic replacement. Exact resolved versions and declared license metadata are in `Cargo.lock` / `cargo metadata`; this is not a complete transitive redistribution audit.
