#!/bin/sh
# Puts a relocatable libmpv for one target in src-tauri/vendor/mpv/, which is bundled with the app.
set -eu
# One of: osx-arm64, osx-x64, linux-x64, windows-x64
TARGET=${1:-osx-arm64}
DEST="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/vendor/mpv"

rm -rf "$DEST"
mkdir -p "$DEST"

case "$TARGET" in
  osx-arm64|osx-x64)
    if [ "$(uname)" != Darwin ]; then echo "libmpv for $TARGET has to be fetched on a Mac" >&2; exit 1; fi
    # The build runs on the target's own architecture, so Homebrew's mpv is the right one.
    brew list mpv >/dev/null 2>&1 || brew install mpv
    SRC="$(brew --prefix mpv)/lib/libmpv.2.dylib"
    python3 - "$SRC" "$DEST" <<'PY'
"""Copies a dylib and the libraries it needs into one folder, pointing each at its neighbours
(install_name_tool), so the folder works wherever the app is installed. System libraries stay put."""
import os
import subprocess
import sys

src, dest = sys.argv[1], sys.argv[2]
SYSTEM = ("/usr/lib/", "/System/")


def deps(path):
    out = subprocess.run(["otool", "-L", path], check=True, capture_output=True, text=True).stdout.splitlines()[1:]
    return [line.split(" (")[0].strip() for line in out]


def rpaths(path):
    out = subprocess.run(["otool", "-l", path], check=True, capture_output=True, text=True).stdout.splitlines()
    return [out[i + 2].split()[1] for i, line in enumerate(out) if line.strip() == "cmd LC_RPATH"]


def resolve(dep, owner, owner_rpaths):
    if dep.startswith("@rpath/"):
        for r in owner_rpaths:
            base = os.path.dirname(owner) if r.startswith("@loader_path") else ""
            candidate = os.path.join(r.replace("@loader_path", base).replace("@executable_path", base), dep[7:])
            if os.path.exists(candidate):
                return os.path.realpath(candidate)
        raise SystemExit(f"can't find {dep} for {owner}")
    if dep.startswith("@loader_path/"):
        return os.path.realpath(os.path.join(os.path.dirname(owner), dep[13:]))
    return os.path.realpath(dep)


todo, done = [os.path.realpath(src)], {}
while todo:
    path = todo.pop()
    name = os.path.basename(path)
    if name in done:
        continue
    done[name] = path
    target = os.path.join(dest, name)
    subprocess.run(["cp", path, target], check=True)
    os.chmod(target, 0o755)
    for dep in deps(path)[1:]:  # the first line is the library's own id
        if dep.startswith(SYSTEM):
            continue
        real = resolve(dep, path, rpaths(path))
        todo.append(real)
        subprocess.run(["install_name_tool", "-change", dep, f"@loader_path/{os.path.basename(real)}", target], check=True,
                       stderr=subprocess.DEVNULL)
    subprocess.run(["install_name_tool", "-id", f"@loader_path/{name}", target], check=True, stderr=subprocess.DEVNULL)

# libmpv is looked up as libmpv.2.dylib.
if "libmpv.2.dylib" not in done:
    (name,) = [n for n in done if n.startswith("libmpv")]
    os.rename(os.path.join(dest, name), os.path.join(dest, "libmpv.2.dylib"))

# Apple Silicon kills unsigned code: ad-hoc sign every library, once all edits are done.
for name in os.listdir(dest):
    subprocess.run(["codesign", "--force", "--sign", "-", os.path.join(dest, name)], check=True, stderr=subprocess.DEVNULL)

# Nothing may still point into Homebrew.
for name in os.listdir(dest):
    bad = [d for d in deps(os.path.join(dest, name))[1:] if not d.startswith(SYSTEM) and not d.startswith("@loader_path/")]
    if bad:
        raise SystemExit(f"{name} still points at {bad}")
print(f"{len(done)} libraries")
PY
    ;;
  windows-x64)
    # shinchiro's mpv-winbuild: one self-contained DLL. The baseline x86-64 build, so CPUs without AVX2 work too.
    BUILD=20261021
    NAME=mpv-dev-x86_64-20261021-git-e471496c56.7z
    SHA256=efc719eff3dc5954b4a5efc800632808bcec01cc9034940dd14e5eb7fe9ea633
    TMP=$(mktemp -d)
    curl -fL "https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/$BUILD/$NAME" -o "$TMP/$NAME"
    GOT=$( (sha256sum "$TMP/$NAME" 2>/dev/null || shasum -a 256 "$TMP/$NAME") | cut -d' ' -f1)
    if [ "$GOT" != "$SHA256" ]; then echo "$NAME: expected $SHA256, got $GOT" >&2; exit 1; fi
    if command -v 7z >/dev/null 2>&1; then
      7z e -y -o"$TMP" "$TMP/$NAME" libmpv-2.dll >/dev/null
    else
      tar -xf "$TMP/$NAME" -C "$TMP" libmpv-2.dll
    fi
    cp "$TMP/libmpv-2.dll" "$DEST/"
    rm -rf "$TMP"
    ;;
  linux-x64)
    if [ "$(uname)" != Linux ]; then echo "libmpv for $TARGET has to be fetched on Linux" >&2; exit 1; fi
    # Built on the oldest Ubuntu the app supports, so the glibc floor matches the app's.
    SUDO=$(command -v sudo || true)
    $SUDO apt-get update -qq
    DEBIAN_FRONTEND=noninteractive $SUDO apt-get install -y -qq --no-install-recommends libmpv-dev libwebkit2gtk-4.1-0 patchelf python3 >/dev/null
    python3 - "$DEST" <<'PY'
"""Copies libmpv and the libraries it needs into one folder and points each at its neighbours
(patchelf, $ORIGIN), so the folder works in every Linux package. A library stays the system's only
when it's certain to be there: cardamoi needs WebKitGTK, so everything WebKitGTK links is; and the
C runtime, graphics drivers and audio servers have to be the system's own. Everything else ships."""
import os
import re
import shutil
import subprocess
import sys

dest = sys.argv[1]
SYSTEM = re.compile(
    r"^(ld-linux|linux-vdso|libc|libm|libdl|libpthread|librt|libresolv|libutil|libgcc_s|libstdc\+\+"
    r"|libGL|libGLX|libGLdispatch|libEGL|libOpenGL|libgbm|libdrm|libvulkan|libasound|libpulse)[.-]"
)

found = subprocess.run(["ldconfig", "-p"], check=True, capture_output=True, text=True).stdout


def locate(pattern):
    return next(line.split(" => ")[1].strip() for line in found.splitlines() if re.search(pattern, line))


def ldd(path):
    out = subprocess.run(["ldd", path], check=True, capture_output=True, text=True).stdout
    missing = [l.strip() for l in out.splitlines() if "not found" in l]
    if missing:
        raise SystemExit(f"{path}: unresolved {missing}")
    return {m.group(1): m.group(2) for m in (re.match(r"\s*(\S+) => (/\S+)", l) for l in out.splitlines()) if m}


libmpv = locate(r"libmpv\.so\.\d ")
soname = os.path.basename(libmpv)
webkit = locate(r"libwebkit2gtk-4\.1\.so\.0 ")
present = set(ldd(webkit)) | {os.path.basename(webkit)}
deps = ldd(libmpv)

copied = [soname]
shutil.copy(os.path.realpath(libmpv), os.path.join(dest, soname))
for name, path in sorted(deps.items()):
    if SYSTEM.match(name) or name in present:
        continue
    shutil.copy(os.path.realpath(path), os.path.join(dest, name))
    copied.append(name)
for name in copied:
    target = os.path.join(dest, name)
    os.chmod(target, 0o755)
    subprocess.run(["patchelf", "--set-rpath", "$ORIGIN", target], check=True)
size = sum(os.path.getsize(os.path.join(dest, n)) for n in copied)
print(f"{soname} and {len(copied) - 1} libraries, {size // (1024 * 1024)} MB")
PY
    ;;
  *) echo "unknown target: $TARGET" >&2; exit 1 ;;
esac

echo "libmpv ($TARGET) is in $DEST"
