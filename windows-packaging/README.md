# Shortwave for Windows — Build & Package Guide

> **Using this folder:** place it at `windows-packaging/` in your forked
> Shortwave repo. `bundle.py` finds the repo root automatically and accepts
> either layout: the source in a `shortwave/` subfolder next to `tools/msys64`
> (workspace style), or the source at the repo root itself with `tools/msys64`
> beside it (in-repo style). It looks for `build/installroot` under either.
> Paths can be overridden with the constants at the top of `bundle.py`.

Unofficial Windows build of [Shortwave](https://apps.gnome.org/Shortwave/)
(GNOME internet radio player, GPL-3.0-or-later) from
`gitlab.gnome.org/World/Shortwave` @ `e468e17` (v5.1.0, 2026-10-11).

Upstream already contains the Windows port (SMTC media controls via the
`windows` crate, `gdk4-win32`, platform `cfg` gates). Everything below is
build + packaging only — **no source modifications**.

## Layout

| Path | What it is |
|---|---|
| `shortwave/` | Cloned source tree + Meson build (`build/`) + installed prefix (`build/installroot/`) + collected bundle (`bundle/`) |
| `tools/msys64/` | Self-contained MSYS2/UCRT64 toolchain (Rust 1.99, GCC 16, GTK 4.24, libadwaita 1.10, GStreamer 1.28, glycin 2.2, NSIS, Meson) |
| `bundle.py` | Collects `shortwave.exe` + transitive DLL closure + curated GStreamer plugins + schemas/icons/locales into `shortwave/bundle/`; compiles the launcher |
| `launcher.c` | Tiny wrapper that becomes `shortwave.exe`; sets `XDG_DATA_DIRS` so GSettings finds the relocated schemas, then runs `shortwave-bin.exe`. GLib bakes its data dir at compile time; without this a moved install can't find its schemas |
| `shortwave.nsi` | NSIS installer script (MUI2, LZMA solid) |
| `shortwave.ico` | Installer/app icon, generated from the app SVG. Frames under 256 px are 32-bit BMP, 256 px is PNG (an all-PNG ICO shows as a generic icon in Explorer/Edge) |
| `shortwave.rc` | Icon + version info compiled into the launcher (`shortwave.exe`) by `bundle.py` via `windres` |
| `pe_icon.py` | Checks ICO/EXE icon frames and repairs `rcedit` output (`--check-ico`, `--check`, `--fix`); needs `pefile` |
| `COPYING.txt` | GPL-3.0 text shown on the installer's license page |
| `shortwave-5.1.0.1-win64-setup.exe` | **The deliverable** (34 MB) |

## Quick rebuild after changing code or graphics

All from an UCRT64 shell (`export MSYSTEM=UCRT64; tools/msys64/usr/bin/bash -lc '...'`):

```bash
cd shortwave && ninja -C build install   # recompile (assets only = seconds; code = minutes)
cd .. && python bundle.py                # recollect bundle (~1 min)
makensis.exe shortwave.nsi               # rebuild installer (~2 min)
```

Extra tools for the icon step: `rcedit-x64.exe` (from
https://github.com/electron/rcedit/releases) in `tools/` or on `PATH`, and
`pefile` (`pacman -S mingw-w64-ucrt-x86_64-python-pefile`). `bundle.py`
embeds `shortwave.ico` + version info in both `bin/shortwave.exe` and
`bin/shortwave-bin.exe` and stops if the icon check fails.

If the app SVG icon changed, regenerate `shortwave.ico` first. **Don't save it
with Pillow's default ICO writer**: that stores every size as PNG, and Windows
Explorer/Edge then show a generic icon. Frames under 256 px must be 32-bit
BMP (PNG is fine only at 256 px). Render each size with `rsvg-convert`, build
the ICO with BMP frames 16-128 and a PNG 256 frame (e.g. `icotool -c
--raw=icon-256.png icon-128.png ... icon-16.png -o shortwave.ico`), then check it:

```bash
python pe_icon.py --check-ico shortwave.ico
```

## Where the graphics live (for future customization)

| Asset | Location |
|---|---|
| In-app textures (`metal.png`, `wood.png`) | `shortwave/data/images/` (registered in `data/de.haeckerfelix.Shortwave.gresource.xml.in`) |
| App icon (window/taskbar/installer) | `shortwave/data/icons/hicolor/scalable/apps/de.haeckerfelix.Shortwave.svg` |
| Small UI icons (symbolic, 16×16 viewbox, single color) | `shortwave/data/icons/hicolor/scalable/{actions,status}/` |
| Window/widget layouts | `shortwave/data/gtk/*.ui` (GTK 4 XML; visual editor: Cambalache) |
| Colors / styling | `shortwave/data/gtk/style.css` |

Rules: new files must be added to the `.gresource.xml.in`; symbolic icons must
use `currentColor`; keep CSS class names in sync with the `.ui` files.

## GStreamer plugin whitelist

`bundle.py` (`GST_WHITELIST`) ships 23 plugins instead of all 249, selected from
the pipelines in `src/audio/gstreamer_backend.rs`:
`uridecodebin ! audioconvert ! audioresample ! tee ! queue ! wasapi2sink`
plus the recorder branch `vorbisenc ! oggmux ! filesink`.

Covered formats (live-tested through the app's real pipeline
`uridecodebin ! audioconvert ! audioresample ! tee ! queue ! wasapi2sink`):
MP3, AAC, FLAC, Vorbis, Ogg, WAV, AC3, MP4/M4A, HLS (two different
streams), MPEG-TS. Opus is included but untested (no live station available).
Recording (`vorbisenc ! oggmux ! filesink`) verified.

**If a station won't play:** run with `RUST_LOG=debug`, find the missing element
name in the log, map it with `gst-inspect-1.0 <element>` → add that plugin DLL
to `GST_WHITELIST` → rerun `bundle.py` + `makensis.exe`.

## Gotchas discovered (don't rediscover these)

1. **GSettings schemas:** without `XDG_DATA_DIRS` pointing at `<install>\share`,
   the app dies with `Settings schema 'de.haeckerfelix.Shortwave' is not
   installed`. The launcher exists solely for this.
2. **Test with a fresh `HOME`** for each GStreamer run: the registry cache in
   `~/.cache/gstreamer-1.0` can mask missing/extra plugins between test
   iterations, giving false PASS/FAIL flips on the same bundle.
3. **NSIS `File /r`:** directory specs behave oddly; `File /r "shortwave\bundle\*.*"`
   works and preserves structure.
4. **MSYS2 shell quirks:** `cmd //c "<exe> /S /D=..."` works for silent
   install/uninstall from Git Bash; bare `./setup.exe` may hit permission
   errors from inside MSYS bash. Windows' `timeout.exe` shadows Unix `timeout`
   when PATH is reduced to System32 — use background + `taskkill` instead.
5. **Build env:** Meson must run from the UCRT64 login shell (mingw Python +
   ucrt64 msgfmt/gettext for ITS rules, ucrt64 git for VCS tag).
6. **Ogg files need EOS** to finalize; killing a recording test process yields
   a 0-byte file that looks like a failure.
7. **GSettings schema compilation is all-or-nothing-silent:** MSYS2's schema set
   contains a version-mismatched file (`org.gnome.desktop.a11y.interface`
   references an enum not in the bundled `enums.xml`), and
   `glib-compile-schemas` (non-strict) then produces a tiny DB that is missing
   `org.gnome.system.proxy` — while still exiting 0. libsoup reads that schema
   when a playback session starts, so **the app crashes exactly when you press
   play** with `Settings schema 'org.gnome.system.proxy' is not installed`.
   Fix: ship only the needed schemas (`org.gnome.desktop.enums.xml`,
   `org.gnome.system.proxy`, `de.haeckerfelix.Shortwave`) and assert on their
   presence in `gschemas.compiled` (see `SCHEMA_NEEDED` in `bundle.py`).
8. **HLS: use legacy `hlsdemux`, not `hlsdemux2`.** The new adaptivedemux2
   elements require a parent bin with `GST_BIN_FLAG_STREAMS_AWARE`; decodebin v2
   (inside Shortwave's `uridecodebin`) never sets it, so `hlsdemux2` always
   fails there with "Element requires a streams-aware context" — even though
   the plugin and its `libgstadaptivedemux-1.0-0.dll` support library are
   present. `hlsdemux2` also outranks legacy `hlsdemux` (257 vs 256), so the
   legacy plugin must REPLACE it in the whitelist, not join it.
9. **Runtime-loaded DLLs are invisible to PE-import scanning:** adaptivedemux
   support libraries (`libgstadaptivedemux-1.0-0.dll`,
   `libgsturidownloader-1.0-0.dll`) are loaded dynamically, so `bundle.py`
   lists them explicitly in `RUNTIME_LIBS`.

## Version 5.1.0-windows.1 (icon fix)

Same Shortwave 5.1.0 build as `v5.1.0-windows`; only the icons changed. The
old installer's icon had every frame as PNG (generic icon in Explorer/Edge)
and the two exes had no icon at all. The repackaged installer was made from
the released payload with the two exes patched (`rcedit` + `pe_icon.py --fix`),
so nothing was recompiled.

## Installer behavior

- Installs to `%ProgramFiles%\Shortwave` (customizable), bin/lib/share prefix
  layout that GTK/GStreamer/gdk-pixbuf/GIO resolve relative to their DLLs
- Start Menu shortcut + optional desktop shortcut; "Launch Shortwave" on the
  finish page; proper Add/Remove Programs entry with estimated size
- Silent install: `shortwave-5.1.0.1-win64-setup.exe /S /D=C:\full\path`
- Silent uninstall: `<install>\uninstall.exe /S`

## Validation checklist used for the shipped build

- [x] Silent install / uninstall clean (0 files left)
- [x] App launches with PATH isolated from MSYS2 (window, DB, radio-browser API)
- [x] Live playback: MP3, AAC, FLAC, HLS through the app's pipeline shape
- [x] Recording chain: decode → vorbisenc → oggmux → filesink produces valid Ogg
- [x] GStreamer plugins load from the installed tree (`gst-inspect playbin`)
