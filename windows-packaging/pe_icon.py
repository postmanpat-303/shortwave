#!/usr/bin/env python3
"""Icon helpers for the Windows build (needs: pip install pefile, or
pacman -S mingw-w64-ucrt-x86_64-python-pefile).

  python pe_icon.py --check-ico shortwave.ico   # frames <256 px must be BMP32
  python pe_icon.py --fix EXE [EXE...]          # repair rcedit output, then check
  python pe_icon.py --check EXE [EXE...]        # check embedded icon only

Why --fix exists: rcedit (electron/rcedit 2.0.0) writes each frame's size
into RT_GROUP_ICON as a 16-bit value, so the 128 px BMP frame (67,624 bytes)
is recorded as 2,088 bytes, and it creates the icon group with ID 0. --fix
rewrites the sizes from the real RT_ICON data, renumbers the group to 1 and
updates the PE checksum. Code, imports, relocations and the manifest are
untouched.
"""
import struct, sys

PNG = b"\x89PNG\r\n\x1a\n"

def _fmt(data):
    if data[:8] == PNG:
        return "PNG"
    return f"BMP{struct.unpack('<H', data[14:16])[0]}"

def _report(name, frames):
    ok = bool(frames)
    for w, fmt, size in frames:
        bad = w < 256 and fmt != "BMP32"
        ok &= not bad
        print(f"  {w}x{w} {fmt} {size} bytes{'  <-- must be 32-bit BMP' if bad else ''}")
    print(f"{name}: {'OK' if ok else 'FAIL'}")
    return ok

def check_ico(path):
    d = open(path, "rb").read()
    _, typ, cnt = struct.unpack("<HHH", d[:6])
    frames = []
    for i in range(cnt):
        w, h, _, _, _, _, size, off = struct.unpack("<BBBBHHII", d[6 + i*16:22 + i*16])
        frames.append((w or 256, _fmt(d[off:off+size]), size))
    return _report(path, frames)

def _resources(pe):
    icons, group = {}, None
    for t in getattr(pe, "DIRECTORY_ENTRY_RESOURCE").entries:
        if t.id == 3:
            for n in t.directory.entries:
                icons[n.id] = n.directory.entries[0].data.struct
        elif t.id == 14 and group is None:
            group = t.directory.entries[0]
    return icons, group

def check_exe(path, pe=None):
    import pefile
    own = pe is None
    pe = pe or pefile.PE(path, fast_load=False)
    try:
        icons, group = _resources(pe)
        if group is None:
            print(f"{path}: no RT_GROUP_ICON")
            return False
        l = group.directory.entries[0].data.struct
        g = pe.get_data(l.OffsetToData, l.Size)
        frames = []
        for i in range(struct.unpack("<H", g[4:6])[0]):
            w, _, _, _, _, _, size, iid = struct.unpack("<BBBBHHIH", g[6 + i*14:20 + i*14])
            s = icons[iid]
            data = pe.get_data(s.OffsetToData, s.Size)
            if size != s.Size:
                print(f"  icon {iid}: group says {size} bytes, data is {s.Size}")
                return False
            frames.append((w or 256, _fmt(data), size))
        print(f"  group icon ID {group.id}")
        return _report(path, frames) and group.id != 0
    finally:
        if own:
            pe.close()

def fix_exe(path):
    import pefile
    pe = pefile.PE(path)
    icons, group = _resources(pe)
    if group is None:
        sys.exit(f"{path}: no icon group to fix")
    l = group.directory.entries[0].data.struct
    off = pe.get_offset_from_rva(l.OffsetToData)
    for i in range(struct.unpack("<H", pe.__data__[off+4:off+6])[0]):
        e = off + 6 + i*14
        iid = struct.unpack("<H", pe.__data__[e+12:e+14])[0]
        pe.set_dword_at_offset(e + 8, icons[iid].Size)
    if group.id == 0:
        group.struct.Name = 1
    if pe.OPTIONAL_HEADER.CheckSum:
        pe.OPTIONAL_HEADER.CheckSum = pe.generate_checksum()
    data = pe.write()
    pe.close()
    with open(path, "wb") as f:
        f.write(data)

if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    mode, files = sys.argv[1], sys.argv[2:]
    ok = True
    for f in files:
        if mode == "--check-ico":
            ok &= check_ico(f)
        elif mode in ("--fix", "--check"):
            if mode == "--fix":
                fix_exe(f)
            ok &= check_exe(f)
        else:
            sys.exit(__doc__)
    sys.exit(0 if ok else 1)
