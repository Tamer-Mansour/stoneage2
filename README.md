# StoneAge 2

**A modern Rust re-implementation of the StoneAge frequency-sounding survey software for the
Velleman PCSGU250 USB oscilloscope.**

StoneAge 2 records frequency soundings point by point and row by row, then turns them into depth
profiles, horizontal slices, maps and a 3D block of the subsurface. It is written from scratch in
Rust with [egui](https://github.com/emilk/egui) and reads and writes the same `.saf` survey files
as the classic StoneAge 1.49.

> StoneAge 2 is an independent project. It is not affiliated with, endorsed by or supported by the
> makers of the original StoneAge software or by Velleman.

## Features

- **Six views**
  - **Map**: survey area on top of a background map image (move, scale, rotate the grid)
  - **3D**: rotatable volume of all rows with slices and opacity control
  - **Areal**: all rows side by side as a depth slice
  - **Row**: depth profile of the current row
  - **Point**: spectrum and signal of a single measurement point
  - **Soundtest**: live audio input check
- **Compatible files**: opens and saves StoneAge `.saf` surveys
- **CSV import and export**, image export and printing
- **Recording with delay**: countdown before each point, so you can step away from the probe
- **Languages**: German, English and Turkish
- **Dark and light theme**
- **Automatic updates** from GitHub Releases (verified with SHA-256 before installing)

## Download

Get the latest version from the
[**Releases page**](https://github.com/Tamer-Mansour/stoneage2/releases).

| Package | Use it for |
| --- | --- |
| `stoneage-windows-x86.zip` | **Working with the real oscilloscope.** Required, because the vendor driver `PCSGU250.dll` is 32-bit. |
| `stoneage-windows-x64.zip` | Demo mode and working with existing files (no oscilloscope). |

Unzip and run `stoneage.exe`. No installer is needed.

## Hardware setup

To measure with the PCSGU250, place these vendor files **next to `stoneage.exe`**:

- `PCSGU250.dll`
- `PcLab2000LT.exe`
- `pcgu250.bit`

They come with Velleman's PC-Lab2000LT software and are **not included** in this repository or in
the release packages. Without them StoneAge 2 starts in demo mode.

## Building from source

Requires a current stable [Rust](https://rustup.rs) toolchain and the MSVC build tools on Windows.

```sh
# 32-bit build (needed for the real oscilloscope)
rustup target add i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc

# run the tests
cargo test
```

The executable ends up in `target/i686-pc-windows-msvc/release/stoneage.exe`.

Public builds embed the own assets from `crates/stoneage-core/assets/public/` (splash screen,
colour palettes, license pages and base UI texts). Strings of the new interface live in
`crates/stoneage-core/assets/i18n/app_<lang>.properties`; add new `App.*` keys there.

## Releasing

Releases are built by GitHub Actions when a version tag is pushed. See
[docs/RELEASING.md](docs/RELEASING.md).

## Credits

- Icons: [tlp-icons](crates/stoneage-app/assets/icons/LICENSE-tlp-icons.txt), MIT License
- Based on the workflow and file format of StoneAge 1.49

## License

No license has been chosen for this code yet. All rights reserved unless a `LICENSE` file is
added to this repository.
