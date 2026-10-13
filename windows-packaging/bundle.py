#!/usr/bin/env python3
"""Collect Shortwave.exe + all runtime dependencies into a self-contained
prefix-layout bundle (bin/ lib/ share/) suitable for a Windows installer."""
import os, re, shutil, subprocess, sys

def find_root(start):
    """Walk upward from this file until a dir containing a built installroot
    and tools/msys64/ is found, so the script works from the workspace root,
    from a windows-packaging/ subfolder, or inside a repo where the source
    itself lives at the root. Returns (root, src_sub)."""
    layouts = ("shortwave", "")  # workspace: root/shortwave/... ; in-repo: root/...
    d = os.path.abspath(start)
    while True:
        for sub in layouts:
            if os.path.isdir(os.path.join(d, sub, "build", "installroot")) and \
               os.path.isdir(os.path.join(d, "tools", "msys64", "ucrt64")):
                return d, sub
        parent = os.path.dirname(d)
        if parent == d:
            return os.path.abspath(start), "shortwave"  # give up; fail loudly below
        d = parent

ROOT, SRC_SUB = find_root(os.path.dirname(os.path.abspath(__file__)))
MSYS = os.path.join(ROOT, "tools", "msys64", "ucrt64")
INSTALL = os.path.join(ROOT, SRC_SUB, "build", "installroot")
BUNDLE = os.path.join(ROOT, SRC_SUB, "bundle")

if os.path.exists(BUNDLE):
    shutil.rmtree(BUNDLE)

def b(path, *parts):
    return os.path.join(BUNDLE, path, *parts)

def mkdir(path):
    os.makedirs(path, exist_ok=True)

def copy(src, dst):
    mkdir(os.path.dirname(dst))
    shutil.copy2(src, dst)

# ---------------------------------------------------------------- PE scan --
DLL_RE = re.compile(r"^\s*DLL Name:\s*(\S+)", re.M)

def depends(pe_file):
    out = subprocess.run(
        [os.path.join(MSYS, "bin", "objdump.exe"), "-p", pe_file],
        capture_output=True, text=True, check=True).stdout
    return DLL_RE.findall(out)

def collect_dlls(seed_files, out_dir):
    """BFS: copy every dependency resolvable in ucrt64/bin into out_dir."""
    done = set()
    queue = list(seed_files)
    copied = []
    while queue:
        pe = queue.pop()
        try:
            deps = depends(pe)
        except Exception as e:
            print(f"  ! cannot scan {pe}: {e}")
            continue
        for name in deps:
            lname = name.lower()
            if lname in done:
                continue
            done.add(lname)
            src = os.path.join(MSYS, "bin", name)
            if not os.path.exists(src):
                continue  # Windows system DLL
            dst = os.path.join(out_dir, name)
            copy(src, dst)
            copied.append(name)
            queue.append(dst)
    return copied

# ------------------------------------------------------------ app binary --
print("== app binary")
exe_src = os.path.join(INSTALL, "bin", "shortwave.exe")
exe_dst = b("bin", "shortwave-bin.exe")
copy(exe_src, exe_dst)

# The real binary is renamed to shortwave-bin.exe; a small launcher becomes
# shortwave.exe and sets XDG_DATA_DIRS so GSettings finds the relocated
# schemas.
gcc_env = dict(os.environ)
gcc_env["PATH"] = os.path.join(MSYS, "bin") + os.pathsep + gcc_env.get("PATH", "")
subprocess.run(
    [os.path.join(MSYS, "bin", "gcc.exe"), "-O2", "-mwindows", "-s",
     os.path.join(ROOT, "launcher.c"),
     "-o", b("bin", "shortwave.exe")],
    check=True, env=gcc_env)

# ------------------------------------------------------------- gstreamer --
# Only the plugins a radio player needs: Shortwave builds the pipeline
#   uridecodebin ! audioconvert ! audioresample ! tee ! queue ! wasapi2sink
# plus a recorder branch (vorbisenc ! oggmux ! filesink) and codec coverage
# for common stream formats (MP3, AAC, Opus, Vorbis, FLAC, WAV, AC3, HLS, MPEG-TS).
print("== gstreamer plugins (curated set)")
GST_WHITELIST = [
    # infrastructure
    "libgstcoreelements",      # queue, tee, filesink, typefind, multiqueue
    "libgsttypefindfunctions", # stream sniffers
    "libgstplayback",          # uridecodebin, decodebin, playbin
    "libgstsoup",              # souphttpsrc (http/https streams)
    "libgstwasapi2",           # wasapi2sink (Windows audio output)
    # audio conversion / volume
    "libgstaudioconvert",
    "libgstaudioresample",
    "libgstvolume",
    # demuxers / parsers
    "libgstogg",               # oggdemux, oggmux
    "libgsthls",               # legacy hlsdemux (works inside decodebin v2;
                               # the newer hlsdemux2 requires a streams-aware
                               # parent that uridecodebin v1 cannot provide)
    "libgstmpegtsdemux",       # tsdemux (MPEG-TS / DAB)
    "libgstisomp4",            # qtdemux (MP4/M4A)
    "libgstaudioparsers",      # aacparse, mpegaudioparse
    "libgstid3demux",          # ID3 tags
    "libgsticydemux",          # ICY metadata (song titles!)
    # decoders
    "libgstmpg123",            # MP3
    "libgstfaad",              # AAC
    "libgstfdkaac",            # AAC (second decoder)
    "libgstopus",              # Opus
    "libgstvorbis",            # Vorbis (+ vorbisenc for recording)
    "libgstflac",              # FLAC
    "libgstwavparse",          # WAV
    "libgsta52dec",            # AC3
]
gst_plugin_dir = os.path.join(MSYS, "lib", "gstreamer-1.0")
gst_seeds = []
for name in GST_WHITELIST:
    f = name + ".dll"
    src = os.path.join(gst_plugin_dir, f)
    if not os.path.exists(src):
        print(f"  ! WARNING: {f} not found")
        continue
    dst = b("lib", "gstreamer-1.0", f)
    copy(src, dst)
    gst_seeds.append(dst)
print(f"  {len(gst_seeds)}/{len(GST_WHITELIST)} plugins")

# -------------------------------------------------------------- gio modules
print("== gio modules")
gio_dir = os.path.join(MSYS, "lib", "gio", "modules")
gio_seeds = []
for f in sorted(os.listdir(gio_dir)):
    if f.endswith(".dll"):
        src = os.path.join(gio_dir, f)
        dst = b("lib", "gio", "modules", f)
        copy(src, dst)
        gio_seeds.append(dst)
# no giomodule.cache: absolute build paths inside would break module loading;
# without a cache GIO scans the module dir instead.

# --------------------------------------------------------- pixbuf loaders --
print("== gdk-pixbuf loaders")
SKIP_PIXBUF = {"libpixbufloader-heif.dll", "io-wmf.dll"}
pix_dir = os.path.join(MSYS, "lib", "gdk-pixbuf-2.0", "2.10.0", "loaders")
pix_dst_dir = b("lib", "gdk-pixbuf-2.0", "2.10.0", "loaders")
pix_seeds, pix_names = [], []
for f in sorted(os.listdir(pix_dir)):
    if f.endswith(".dll") and f not in SKIP_PIXBUF:
        copy(os.path.join(pix_dir, f), os.path.join(pix_dst_dir, f))
        pix_seeds.append(os.path.join(pix_dst_dir, f))
        pix_names.append(f)

# regenerate loaders.cache with relative paths (resolved against the loaders dir)
cache = subprocess.run(
    [os.path.join(MSYS, "bin", "gdk-pixbuf-query-loaders.exe")] + pix_names,
    capture_output=True, text=True, check=True,
    cwd=pix_dst_dir).stdout
cache = cache.replace(pix_dst_dir.replace("/", "\\") + "\\", "")
cache = cache.replace(pix_dst_dir.replace("\\", "/") + "/", "")
with open(os.path.join(pix_dst_dir, "loaders.cache"), "w") as fh:
    fh.write(cache)

# ------------------------------------------------------------ DLL closure --
# Adaptivedemuxers (hlsdemux2/dashdemux2) load these support libraries at
# RUNTIME (not via the PE import table), so the BFS alone misses them.
# Without libgstadaptivedemux, HLS fails with the cryptic
# "Element requires a streams-aware context".
RUNTIME_LIBS = ["libgstadaptivedemux-1.0-0.dll", "libgsturidownloader-1.0-0.dll"]

print("== transitive DLL closure")
all_seeds = [exe_dst] + gst_seeds + gio_seeds + pix_seeds
for name in RUNTIME_LIBS:
    src = os.path.join(MSYS, "bin", name)
    if os.path.exists(src):
        dst = b("bin", name)
        copy(src, dst)
        all_seeds.append(dst)
    else:
        print(f"  ! WARNING: runtime lib {name} not found")
copied = collect_dlls(all_seeds, b("bin"))
print(f"  {len(copied)} DLLs collected")

# ---------------------------------------------------------------- schemas --
# NOTE: compiling ALL of MSYS2's schemas fails silently: one version-mismatched
# file (org.gnome.desktop.a11y.interface referencing an enum that isn't defined
# in the bundled enums.xml) makes glib-compile-schemas abort the whole run
# (non-strict: rc 0, warning swallowed), leaving a DB without
# org.gnome.system.proxy -> libsoup aborts the app at playback with
# "Settings schema 'org.gnome.system.proxy' is not installed".
# So we ship only the schemas the app stack actually reads:
#   - the app's own schema
#   - org.gnome.system.proxy (libsoup, at every stream playback)
# GTK4/libadwaita/GStreamer do not use GSettings on Windows.
print("== gsettings schemas")
SCHEMA_NEEDED = [
    "org.gnome.desktop.enums.xml",  # defines GDesktopProxyMode, required by proxy schema
    "org.gnome.system.proxy.gschema.xml",
    "de.haeckerfelix.Shortwave.gschema.xml",
]
schema_src = os.path.join(MSYS, "share", "glib-2.0", "schemas")
schema_dst = b("share", "glib-2.0", "schemas")
for f in SCHEMA_NEEDED:
    src = os.path.join(schema_src, f)
    if not os.path.exists(src):
        src = os.path.join(INSTALL, "share", "glib-2.0", "schemas", f)
    copy(src, os.path.join(schema_dst, f))
subprocess.run([os.path.join(MSYS, "bin", "glib-compile-schemas.exe"), schema_dst],
               check=True, capture_output=True)
compiled = open(os.path.join(schema_dst, "gschemas.compiled"), "rb").read()
for needle in (b"org.gnome.system.proxy", b"de.haeckerfelix.Shortwave"):
    assert needle in compiled, f"{needle} missing from gschemas.compiled"

# ----------------------------------------------------------------- icons --
print("== icons")
for theme in ("Adwaita", "hicolor"):
    src = os.path.join(MSYS, "share", "icons", theme)
    if os.path.isdir(src):
        shutil.copytree(src, b("share", "icons", theme),
                        ignore=shutil.ignore_patterns("icon-theme.cache"))
# app icons on top of hicolor
hi = os.path.join(INSTALL, "share", "icons", "hicolor")
for dirpath, _, files in os.walk(hi):
    for f in files:
        copy(os.path.join(dirpath, f),
             os.path.join(b("share", "icons", "hicolor"),
                          os.path.relpath(os.path.join(dirpath, f), hi)))

# ------------------------------------------------------- app data, locales --
print("== gresource + locales")
shutil.copytree(os.path.join(INSTALL, "share", "shortwave"),
                b("share", "shortwave"))
shutil.copytree(os.path.join(INSTALL, "share", "locale"),
                b("share", "locale"))

total = sum(os.path.getsize(os.path.join(dp, f))
            for dp, _, fs in os.walk(BUNDLE) for f in fs)
print(f"\nBundle ready: {BUNDLE} ({total / 1024 / 1024:.1f} MB)")
