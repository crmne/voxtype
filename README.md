# voxtype-crmne-git

A local Arch package for Carmine's voxtype fork, built from the `integration`
branch of [crmne/voxtype](https://github.com/crmne/voxtype) (origin/dev plus
his open PRs). It isn't published to the AUR. Build it on the machine that will
run it, because whisper.cpp is tuned for the CPU it's built on.

The package adapts to the machine that builds it:

| Variant | Path in `/usr/lib/voxtype/` | Built when |
|---|---|---|
| Whisper, CPU | `voxtype-native` | always |
| Whisper, Vulkan | `voxtype-vulkan` | always |
| ONNX engines, CPU | `onnx-cpu/voxtype-onnx-avx512` (`-avx2` on CPUs without AVX-512), next to Microsoft's CPU ONNX Runtime 1.24.4 | always |
| ONNX engines, CUDA 13 | `cuda-13/voxtype-onnx-cuda-13`, next to Microsoft's CUDA 13 ONNX Runtime 1.24.4 | `nvcc` reports CUDA 13 (or `libcudart.so.13` is present) |
| Parakeet on AMD GPU, MIGraphX | `migraphx/voxtype-onnx-migraphx`, linked to Arch's `onnxruntime-rocm` | `migraphx` and `onnxruntime-rocm` are installed |

You can override the detection with `VOXTYPE_CUDA=0|1` and
`VOXTYPE_MIGRAPHX=0|1`, for example `VOXTYPE_CUDA=0 makepkg -sif`. When a
GPU variant is off, its sources are not downloaded.
`.SRCINFO` is for reference only. It was generated on the desktop, so it
lists the CUDA source. makepkg reads the `PKGBUILD`.

## How `/usr/bin/voxtype` is chosen

`/usr/bin/voxtype` is written by the install script, as in `voxtype-bin`, and
`sudo voxtype setup gpu|onnx --enable|--disable` can change it later.

- **Upgrade:** the current selection is kept.
- **First install, ONNX engine** (Parakeet and the others, or no user config
  yet): the CUDA 13 variant is used if it was built, an NVIDIA GPU is present
  and `libcudart.so.13` is installed. Otherwise the ONNX CPU variant is used.
  MIGraphX is never selected automatically.
- **First install, Whisper:** the Vulkan variant is used if a Vulkan driver
  and a GPU are present. Otherwise the native CPU variant is used.

## Laptop (Zenbook S16: Ryzen AI 9 HX 370, Radeon 890M, no NVIDIA)

```sh
sudo pacman -S --needed base-devel git rust
git clone -b packaging/crmne-git https://github.com/crmne/voxtype.git ~/Code/aur/voxtype-crmne-git
cd ~/Code/aur/voxtype-crmne-git
makepkg -si          # replaces voxtype-bin if it is installed
voxtype setup --download --model parakeet-tdt-0.6b-v3-int8
systemctl --user daemon-reload
systemctl --user enable voxtype
systemctl --user restart voxtype
voxtype status
```

This installs Whisper (CPU and Vulkan) and ONNX on the CPU. With
`engine = "parakeet"`, `/usr/bin/voxtype` points to
`/usr/lib/voxtype/onnx-cpu/voxtype-onnx-avx512`.

## Desktop (RTX 3090, CUDA 13 in /opt/cuda)

Use the same commands. The build also produces the CUDA 13 variant, and
`/usr/bin/voxtype` runs `cuda-13/voxtype-onnx-cuda-13`. The ONNX CPU
variant is installed as well, and `sudo voxtype setup onnx --disable` switches
to it.

## Update

```sh
cd ~/Code/aur/voxtype-crmne-git
git pull                 # PKGBUILD changes, if any
makepkg -sif             # fetches the latest integration branch and rebuilds
systemctl --user restart voxtype   # when the daemon is idle
```

The selected variant is kept across upgrades.

## Revert

Each build leaves its `voxtype-crmne-git-<version>-x86_64.pkg.tar.zst` in
this directory. To go back to an earlier build:

```sh
sudo pacman -U ~/Code/aur/voxtype-crmne-git/voxtype-crmne-git-<older-version>-x86_64.pkg.tar.zst
systemctl --user restart voxtype
```

To go back to the stock release, run `yay -S voxtype-bin`. It conflicts with
this package and replaces it. Your config is left unchanged.

## Trying Parakeet on the Radeon 890M (MIGraphX, optional)

1. Install ROCm and the ONNX Runtime that includes the MIGraphX EP, then
   rebuild so the variant is included. This is several GB of ROCm.

   ```sh
   sudo pacman -S --needed rocm-hip-runtime migraphx onnxruntime-rocm
   rocminfo | grep -m1 gfx1150      # the iGPU must show up; if access is denied: sudo usermod -aG render,video $USER and log in again
   cd ~/Code/aur/voxtype-crmne-git && makepkg -sif
   ```

2. Compare it with the CPU on the same clip. Run each command twice: the
   first MIGraphX run compiles the graph and caches it in
   `$ORT_MIGRAPHX_MODEL_CACHE_PATH`, which defaults to
   `~/.cache/voxtype/migraphx`.

   ```sh
   ffmpeg -i some-dictation.m4a -ar 16000 -ac 1 /tmp/clip.wav
   time /usr/lib/voxtype/onnx-cpu/voxtype-onnx-avx512 transcribe /tmp/clip.wav
   export ORT_MIGRAPHX_MODEL_CACHE_PATH=~/.cache/voxtype/migraphx; mkdir -p $ORT_MIGRAPHX_MODEL_CACHE_PATH
   time /usr/lib/voxtype/migraphx/voxtype-onnx-migraphx -v transcribe /tmp/clip.wav
   time /usr/lib/voxtype/migraphx/voxtype-onnx-migraphx -v transcribe /tmp/clip.wav
   ```

   Also try a clip of a different length. If every new length is slow again,
   MIGraphX is recompiling for each input shape, and dictation will feel slow.

3. Switch only if it's faster:

   ```sh
   sudo voxtype setup onnx --enable && systemctl --user restart voxtype    # MIGraphX
   sudo voxtype setup onnx --disable && systemctl --user restart voxtype   # back to ONNX CPU
   ```

The int8 model uses `DynamicQuantizeLinear` and `MatMulInteger`. MIGraphX
lists both as supported, but "dynamic shape is not supported". If the int8
model fails or falls back to the CPU, download the fp32 model
(`voxtype setup --download --model parakeet-tdt-0.6b-v3`, about 2.5 GB) and
point `[parakeet] model` at it for the test.
