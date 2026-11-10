# Third-party notices

Wincast is an independent MIT-licensed rewrite. The Go build in
`go/` bundles the following runtime modules. Versions are pinned by `go.mod`
and `go.sum`; the release script copies each module's full license text next to
the executable under `licenses/`.

| Module | Version | License |
| --- | --- | --- |
| github.com/egoist/mygo | v0.6.4 | MIT |
| github.com/fastschema/qjs | v0.0.6 | MIT |
| github.com/tetratelabs/wazero | v1.9.0 | Apache-2.0 |
| github.com/yuin/goldmark | v1.7.13 | MIT |
| github.com/dustin/go-humanize | v1.1.0 | MIT |
| github.com/ebitengine/purego | v0.11.1 | Apache-2.0 |
| github.com/go-text/typesetting | v0.3.5 | Unlicense OR BSD-3-Clause |
| github.com/mattn/go-isatty | v0.0.24 | MIT |
| github.com/ncruces/go-strftime | v1.1.0 | MIT |
| github.com/remyoudompheng/bigfft | v0.0.0-20230129092748-24d4a6f8daec | BSD-3-Clause |
| golang.org/x/crypto | v0.57.0 | BSD-3-Clause |
| golang.org/x/image | v0.46.0 | BSD-3-Clause |
| golang.org/x/net | v0.59.0 | BSD-3-Clause |
| golang.org/x/sys | v0.49.0 | BSD-3-Clause |
| golang.org/x/text | v0.43.0 | BSD-3-Clause |
| modernc.org/libc | v1.79.0 | BSD-3-Clause |
| modernc.org/mathutil | v1.7.1 | BSD-3-Clause |
| modernc.org/memory | v1.12.1 | BSD-3-Clause |
| modernc.org/sqlite | v1.61.1 | BSD-3-Clause (SQLite itself is public domain) |

The Go runtime and standard library are distributed under the Go BSD-3-Clause
license.

## Bundled extension runtime

`go/internal/features/extensions/runtime/RaycastRuntime.generated.js` is built
from `github.com/BlastLauncher/blast` (MIT, Copyright 2023 Yukai Huang and
contributors), React and react-reconciler (MIT, Meta Platforms, Inc. and
affiliates), and browser-safe Node polyfills. Its source and build script live
next to it under `runtime/js/`; the generated file is committed so release
builds never need Node. The Blast license text is in
`runtime/js/LICENSE-blast`.

## Data and behavior references

- ECDICT dictionary data (MIT, Copyright (c) 2025 Linwei) is not bundled. The
  user installs it with `tools/build-dict.ps1`, which writes
  `LICENSE-ECDICT.txt` next to the database.
- The UI and product behavior reference the original macOS Tinycast project
  (AGPL-3.0). This repository is an independent rewrite and does not contain or
  link its Swift source.
