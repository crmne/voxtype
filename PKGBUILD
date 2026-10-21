# Maintainer: Carmine Paolino <carmine@paolino.me>
# Local-only variant of voxtype-git (peteonrails/voxtype#757) that builds
# Carmine's fork at the `integration` branch: origin/dev plus his open PRs,
# rebuilt by ~/Code/voxtype/.integration/rebuild.sh. Not for the AUR.
#
# The package adapts to the machine that builds it, and is meant to be
# installed on that machine. Variants, laid out like voxtype-bin so
# `voxtype setup gpu|onnx` can switch between them:
#
#   always
#     /usr/lib/voxtype/voxtype-native            Whisper on the CPU
#     /usr/lib/voxtype/voxtype-vulkan            Whisper on Vulkan
#     /usr/lib/voxtype/onnx-cpu/voxtype-onnx-avx512 (or -avx2, after the
#         build host's CPU) ONNX engines on the CPU, next to Microsoft's
#         CPU ONNX Runtime (load-dynamic, like Dockerfile.onnx-cuda-13)
#   when CUDA 13 is installed at build time (VOXTYPE_CUDA=auto|1|0)
#     /usr/lib/voxtype/cuda-13/voxtype-onnx-cuda-13  next to Microsoft's
#         ONNX Runtime cu13 build, as in Dockerfile.onnx-cuda-13
#   when ROCm MIGraphX and onnxruntime-rocm are installed at build time
#   (VOXTYPE_MIGRAPHX=auto|1|0)
#     /usr/lib/voxtype/migraphx/voxtype-onnx-migraphx  load-dynamic against
#         Arch's onnxruntime-rocm (MIGraphX EP), symlinked next to it
#
# plus every OSD frontend (gtk4, native, quickshell), the audio bridge,
# QML, styles, recipes, man pages, completions, configure launcher and the
# systemd user unit.
#
# CPU tuning: Rust code is built for makepkg's x86-64 baseline. whisper.cpp
# (in every daemon variant) keeps its default GGML_NATIVE=ON, so it is tuned
# for the build host: install the package on the machine that built it.
# ONNX Runtime picks AVX2/AVX-512 kernels at run time.

pkgname=voxtype-crmne-git
pkgver=1.1.1.r1365.g982f19b
pkgrel=1
pkgdesc="Push-to-talk voice-to-text for Linux (crmne integration branch, host-adaptive build)"
arch=('x86_64')
url="https://github.com/crmne/voxtype"
license=('MIT')
depends=('alsa-lib' 'bash' 'curl' 'glibc' 'gcc-libs')
makedepends=(
    'cargo'
    'clang'
    'cmake'
    'git'
    'pkgconf'
    'shaderc'
    'vulkan-headers'
    'vulkan-icd-loader'
    'gtk4'
    'gtk4-layer-shell'
    'wayland'
    'libxkbcommon'
)
optdepends=(
    'cuda: ONNX engines on NVIDIA GPUs (CUDA 13; only if built with CUDA)'
    'cudnn: ONNX engines on NVIDIA GPUs (only if built with CUDA)'
    'onnxruntime-rocm: ONNX engines on AMD GPUs via MIGraphX (only if built with MIGraphX)'
    'migraphx: ONNX engines on AMD GPUs (only if built with MIGraphX)'
    'rocm-hip-runtime: ONNX engines on AMD GPUs (only if built with MIGraphX)'
    'vulkan-icd-loader: Whisper on the GPU and native OSD rendering'
    'wtype: keyboard simulation for Wayland (recommended, best CJK support)'
    'dotool: keyboard simulation with layout support (KDE/GNOME compatible)'
    'ydotool: keyboard simulation fallback (X11/TTY support)'
    'wl-clipboard: Wayland clipboard support'
    'libnotify: desktop notifications'
    'gtk4: GTK4 OSD frontend'
    'gtk4-layer-shell: GTK4 OSD frontend'
    'wayland: native OSD frontend'
    'libxkbcommon: native OSD frontend'
    'quickshell: Quickshell OSD frontend'
    'qt6-declarative: Quickshell OSD frontend'
)
provides=("voxtype=${pkgver%%.r*}")
conflicts=('voxtype' 'voxtype-bin' 'voxtype-git')
backup=('etc/voxtype/config.toml')
# whisper.cpp mixes C/C++ with Rust: no cross-language LTO.
options=('!lto' '!debug')
install=voxtype-crmne-git.install
_ort_ver=1.24.4

# --- Build host detection ----------------------------------------------
# Each GPU variant is built only when its runtime is installed here, so a
# laptop never downloads the 180 MB CUDA ONNX Runtime. Override with
# VOXTYPE_CUDA=0|1 and VOXTYPE_MIGRAPHX=0|1 (default: auto).

_cuda13_present() {
    local nvcc
    nvcc=$(command -v nvcc 2>/dev/null || echo /opt/cuda/bin/nvcc)
    if [ -x "$nvcc" ]; then
        "$nvcc" --version 2>/dev/null | grep -q 'release 13\.' && return 0
    fi
    [ -e /opt/cuda/lib64/libcudart.so.13 ] || [ -e /usr/lib/libcudart.so.13 ]
}

# The MIGraphX variant loads Arch's onnxruntime-rocm (which carries the
# MIGraphX EP) instead of bundling an ONNX Runtime: Microsoft ships no
# MIGraphX build, and Dockerfile.onnx-migraphx compiles ORT from source.
_migraphx_present() {
    [ -e /opt/rocm/lib/libmigraphx_c.so.3 ] &&
        [ -e /usr/lib/libonnxruntime_providers_migraphx.so ]
}

_want() { # _want <setting> <probe>
    case "$1" in
        1|yes|true|on) return 0 ;;
        0|no|false|off) return 1 ;;
        *) "$2" ;;
    esac
}

_with_cuda=0
_want "${VOXTYPE_CUDA:-auto}" _cuda13_present && _with_cuda=1
_with_migraphx=0
_want "${VOXTYPE_MIGRAPHX:-auto}" _migraphx_present && _with_migraphx=1

# Name the ONNX CPU variant the way voxtype-bin does, so
# `voxtype setup onnx --disable` finds it on this CPU.
if grep -qw avx512f /proc/cpuinfo 2>/dev/null; then
    _onnx_cpu=voxtype-onnx-avx512
else
    _onnx_cpu=voxtype-onnx-avx2
fi

source=(
    'voxtype::git+https://github.com/crmne/voxtype.git#branch=integration'
    "onnxruntime-cpu-${_ort_ver}.tgz::https://github.com/microsoft/onnxruntime/releases/download/v${_ort_ver}/onnxruntime-linux-x64-${_ort_ver}.tgz"
    'voxtype-crmne-git.install'
)
sha256sums=('SKIP'
            '3a211fbea252c1e66290658f1b735b772056149f28321e71c308942cdb54b747'
            '5d6538281506b9ef5e5beb2d5d985b9c3f2d1f3493dbab5ffb95a570c98628a8')
if [ "$_with_cuda" = 1 ]; then
    source+=("onnxruntime-cuda13-${_ort_ver}.tgz::https://github.com/microsoft/onnxruntime/releases/download/v${_ort_ver}/onnxruntime-linux-x64-gpu_cuda13-${_ort_ver}.tgz")
    sha256sums+=('fdc6eb18317b4eaeda8b3b86595e5da7e853f72bac67ccac9b04ffc20c9f7fe0')
fi

pkgver() {
    cd voxtype
    # The fork carries no recent release tags, so take the version from
    # Cargo.toml and count commits for monotonic -git versions.
    local _ver
    _ver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
    printf '%s.r%s.g%s' "${_ver//-/.}" "$(git rev-list --count HEAD)" "$(git rev-parse --short=7 HEAD)"
}

prepare() {
    cd voxtype
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd voxtype
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    export CARGO_PROFILE_RELEASE_LTO=false
    export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=8
    export CC=clang
    export CXX=clang++
    export CMAKE_BUILD_PARALLEL_LEVEL="${CMAKE_BUILD_PARALLEL_LEVEL:-8}"
    export RUSTFLAGS="${RUSTFLAGS} --remap-path-prefix=$srcdir/="
    export CFLAGS="${CFLAGS} -ffile-prefix-map=$srcdir/="
    export CXXFLAGS="${CXXFLAGS} -ffile-prefix-map=$srcdir/="

    echo "Variants: whisper-native whisper-vulkan ${_onnx_cpu}$([ "$_with_cuda" = 1 ] && echo ' onnx-cuda-13')$([ "$_with_migraphx" = 1 ] && echo ' onnx-migraphx')"

    # Each daemon variant overwrites target/release/voxtype; save it before
    # the next feature set. Dependencies are reused between builds.
    cargo build --frozen --release --bin voxtype
    cp target/release/voxtype "$srcdir/voxtype-native"

    cargo build --frozen --release --bin voxtype --features gpu-vulkan
    cp target/release/voxtype "$srcdir/voxtype-vulkan"

    # Engine set of Dockerfile.onnx, but load-dynamic: the binary dlopens
    # the libonnxruntime.so installed next to it instead of pyke's static
    # download.
    local _onnx_engines=moonshine,sensevoice,paraformer,dolphin,omnilingual,cohere
    cargo build --frozen --release --bin voxtype \
        --features parakeet,parakeet-load-dynamic,${_onnx_engines},openvino-whisper,onnx-load-dynamic,ml-diarization
    cp target/release/voxtype "$srcdir/${_onnx_cpu}"

    if [ "$_with_cuda" = 1 ]; then
        # Same feature set as Dockerfile.onnx-cuda-13: ort load-dynamic, so
        # the binary dlopens the bundled libonnxruntime.so next to it, and
        # ORT_CUDA_VERSION stamps CUDA 13 into the build.
        ORT_CUDA_VERSION=13 cargo build --frozen --release --bin voxtype \
            --features parakeet-cuda,parakeet-load-dynamic,moonshine-cuda,sensevoice-cuda,paraformer-cuda,dolphin-cuda,omnilingual-cuda,cohere-cuda,openvino-whisper,onnx-load-dynamic,ml-diarization
        cp target/release/voxtype "$srcdir/voxtype-onnx-cuda-13"
    fi

    if [ "$_with_migraphx" = 1 ]; then
        # Feature set of Dockerfile.onnx-migraphx (only Parakeet has a
        # MIGraphX EP; the other engines run on the CPU), load-dynamic.
        cargo build --frozen --release --bin voxtype \
            --features parakeet-migraphx,parakeet-load-dynamic,${_onnx_engines},openvino-whisper,onnx-load-dynamic,ml-diarization
        cp target/release/voxtype "$srcdir/voxtype-onnx-migraphx"
    fi

    cargo build --frozen --release \
        --bin voxtype-osd \
        --bin voxtype-osd-gtk4 \
        --bin voxtype-osd-native \
        --bin voxtype-osd-quickshell \
        --bin voxtype-audio-bridge \
        --features osd-gtk4,osd-native

    # Same renderer scripts/package.sh uses for the deb and rpm (#893).
    rm -rf "$srcdir/man"
    cargo run --frozen --release --package xtask -- man "$srcdir/man"
}

check() {
    cd voxtype
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    export CC=clang
    export CXX=clang++
    cargo test --frozen --lib --bins --tests --features osd-gtk4,osd-native
}

package() {
    cd voxtype
    local lib="$pkgdir/usr/lib/voxtype"

    install -Dm755 -t "$lib" \
        "$srcdir/voxtype-native" "$srcdir/voxtype-vulkan" \
        target/release/voxtype-osd \
        target/release/voxtype-osd-gtk4 \
        target/release/voxtype-osd-native \
        target/release/voxtype-osd-quickshell

    # ort's load-dynamic looks for libonnxruntime.so next to the real
    # executable (/proc/self/exe), so each ONNX variant gets its own
    # directory with its ONNX Runtime beside it.
    local cpu_ort="$srcdir/onnxruntime-linux-x64-${_ort_ver}/lib"
    install -Dm755 "$srcdir/${_onnx_cpu}" "$lib/onnx-cpu/${_onnx_cpu}"
    install -Dm644 "$cpu_ort/libonnxruntime.so.${_ort_ver}" "$lib/onnx-cpu/libonnxruntime.so.${_ort_ver}"
    ln -s "libonnxruntime.so.${_ort_ver}" "$lib/onnx-cpu/libonnxruntime.so"
    ln -s "onnx-cpu/${_onnx_cpu}" "$lib/${_onnx_cpu}"

    if [ "$_with_cuda" = 1 ]; then
        local ort="$srcdir/onnxruntime-linux-x64-gpu-${_ort_ver}/lib"
        # ONNX Runtime finds its provider .so files next to the real binary.
        install -Dm755 "$srcdir/voxtype-onnx-cuda-13" "$lib/cuda-13/voxtype-onnx-cuda-13"
        install -Dm644 "$ort/libonnxruntime.so.${_ort_ver}" "$lib/cuda-13/libonnxruntime.so.${_ort_ver}"
        ln -s "libonnxruntime.so.${_ort_ver}" "$lib/cuda-13/libonnxruntime.so"
        install -Dm644 -t "$lib/cuda-13" \
            "$ort/libonnxruntime_providers_cuda.so" \
            "$ort/libonnxruntime_providers_shared.so"
        ln -s cuda-13/voxtype-onnx-cuda-13 "$lib/voxtype-onnx-cuda-13"
    fi

    if [ "$_with_migraphx" = 1 ]; then
        # Point at onnxruntime-rocm's libraries from the variant directory,
        # so ORT resolves the MIGraphX provider beside the library it loaded.
        install -Dm755 "$srcdir/voxtype-onnx-migraphx" "$lib/migraphx/voxtype-onnx-migraphx"
        ln -s /usr/lib/libonnxruntime.so.1 "$lib/migraphx/libonnxruntime.so"
        ln -s /usr/lib/libonnxruntime_providers_shared.so "$lib/migraphx/libonnxruntime_providers_shared.so"
        ln -s /usr/lib/libonnxruntime_providers_migraphx.so "$lib/migraphx/libonnxruntime_providers_migraphx.so"
        ln -s migraphx/voxtype-onnx-migraphx "$lib/voxtype-onnx-migraphx"
    fi
    # /usr/bin/voxtype and /usr/lib/voxtype/voxtype-onnx-cuda are written
    # by the install script, as voxtype-bin does, so `voxtype setup gpu`
    # can repoint them and pacman can replace voxtype-bin cleanly.

    install -Dm755 target/release/voxtype-audio-bridge "$pkgdir/usr/bin/voxtype-audio-bridge"
    ln -s /usr/lib/voxtype/voxtype-osd "$pkgdir/usr/bin/voxtype-osd"

    install -d "$pkgdir/usr/share/voxtype"
    cp -r --no-preserve=ownership quickshell "$pkgdir/usr/share/voxtype/quickshell"
    cp -r --no-preserve=ownership examples/osd-packages "$pkgdir/usr/share/voxtype/osd"
    cp -r --no-preserve=ownership examples/osd-recipes "$pkgdir/usr/share/voxtype/osd-recipes"
    find "$pkgdir/usr/share/voxtype" -name '.*' -prune -exec rm -rf {} +
    find "$pkgdir/usr/share/voxtype" -type f -exec chmod 644 {} +
    find "$pkgdir/usr/share/voxtype" -type d -exec chmod 755 {} +

    install -Dm755 packaging/scripts/voxtype-configure-launcher \
        "$pkgdir/usr/bin/voxtype-configure-launcher"
    install -Dm644 packaging/voxtype-configure.desktop \
        "$pkgdir/usr/share/applications/voxtype-configure.desktop"
    install -Dm644 config/default.toml "$pkgdir/etc/voxtype/config.toml"
    install -Dm644 packaging/systemd/voxtype.service \
        "$pkgdir/usr/lib/systemd/user/voxtype.service"
    install -Dm644 -t "$pkgdir/usr/share/doc/$pkgname" README.md docs/INSTALL.md
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"

    install -Dm644 packaging/completions/voxtype.bash \
        "$pkgdir/usr/share/bash-completion/completions/voxtype"
    install -Dm644 packaging/completions/voxtype.zsh \
        "$pkgdir/usr/share/zsh/site-functions/_voxtype"
    install -Dm644 packaging/completions/voxtype.fish \
        "$pkgdir/usr/share/fish/vendor_completions.d/voxtype.fish"

    test -s "$srcdir/man/voxtype.1"
    install -Dm644 -t "$pkgdir/usr/share/man/man1" "$srcdir/man"/*.1
}
