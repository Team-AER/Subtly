#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

WHISPER_CPP_REPO="${WHISPER_CPP_REPO:-https://github.com/ggerganov/whisper.cpp.git}"
WHISPER_CPP_DIR="${WHISPER_CPP_DIR:-$ROOT_DIR/deps/whisper.cpp}"
WHISPER_CPP_REF="${WHISPER_CPP_REF:-}"
WHISPER_CPP_BUILD_DIR="${WHISPER_CPP_BUILD_DIR:-$WHISPER_CPP_DIR/build}"

cpu_count() {
  if command -v nproc >/dev/null 2>&1; then
    nproc
    return
  fi
  if command -v sysctl >/dev/null 2>&1; then
    sysctl -n hw.ncpu
    return
  fi
  if command -v getconf >/dev/null 2>&1; then
    getconf _NPROCESSORS_ONLN
    return
  fi
  echo 4
}

is_windows() {
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) return 0 ;;
  esac
  return 1
}

is_macos() {
  [[ "$(uname -s)" == "Darwin" ]]
}

is_linux() {
  [[ "$(uname -s)" == "Linux" ]]
}

whisper_bin_name="whisper-cli"
if is_windows; then
  whisper_bin_name="whisper-cli.exe"
fi

whisper_bin_candidates=()
if is_windows; then
  whisper_bin_candidates+=(
    "$WHISPER_CPP_BUILD_DIR/bin/Release/$whisper_bin_name"
    "$WHISPER_CPP_BUILD_DIR/bin/$whisper_bin_name"
  )
else
  whisper_bin_candidates+=(
    "$WHISPER_CPP_BUILD_DIR/bin/$whisper_bin_name"
    "$WHISPER_CPP_BUILD_DIR/bin/Release/$whisper_bin_name"
  )
fi

binary_needs_packaged_ggml_libs() {
  local binary_path="$1"

  if command -v readelf >/dev/null 2>&1; then
    if readelf -d "$binary_path" 2>/dev/null | grep -Eq 'Shared library: \[(libwhisper|libggml).*\.so'; then
      return 0
    fi
  fi

  if command -v ldd >/dev/null 2>&1; then
    if ldd "$binary_path" 2>/dev/null | grep -Eq 'libwhisper|libggml'; then
      return 0
    fi
  fi

  return 1
}

find_existing_binary() {
  local candidate
  for candidate in "${whisper_bin_candidates[@]}"; do
    if [[ -f "$candidate" ]]; then
      echo "$candidate"
      return 0
    fi
  done
  return 1
}

built_binary=""

if [[ -n "${WHISPER_CPP_FORCE_REBUILD:-}" ]]; then
  rm -rf "$WHISPER_CPP_BUILD_DIR"
fi

if built_binary="$(find_existing_binary)"; then
  if is_linux && binary_needs_packaged_ggml_libs "$built_binary"; then
    echo "Existing Linux whisper-cli links to shared ggml/whisper libs; rebuilding static binary."
    rm -rf "$WHISPER_CPP_BUILD_DIR"
  else
    echo "whisper.cpp already built at $built_binary"
    exit 0
  fi
fi

mkdir -p "$(dirname "$WHISPER_CPP_DIR")"

if [[ ! -d "$WHISPER_CPP_DIR" ]]; then
  echo "Cloning whisper.cpp..."
  git clone --depth 1 "$WHISPER_CPP_REPO" "$WHISPER_CPP_DIR"
elif [[ ! -d "$WHISPER_CPP_DIR/.git" ]]; then
  echo "Warning: $WHISPER_CPP_DIR exists but is not a git repo; using it as-is."
fi

if [[ -n "$WHISPER_CPP_REF" ]]; then
  echo "Checking out whisper.cpp ref $WHISPER_CPP_REF..."
  git -C "$WHISPER_CPP_DIR" fetch --depth 1 origin "$WHISPER_CPP_REF"
  git -C "$WHISPER_CPP_DIR" checkout "$WHISPER_CPP_REF"
fi

mkdir -p "$WHISPER_CPP_BUILD_DIR"

cmake_args=(
  -B "$WHISPER_CPP_BUILD_DIR"
  -S "$WHISPER_CPP_DIR"
  -DCMAKE_BUILD_TYPE=Release
)

if is_macos; then
  cmake_args+=("-DGGML_METAL=ON")
elif is_windows || is_linux; then
  cmake_args+=("-DGGML_VULKAN=ON")
fi

if is_windows; then
  # Prefer static linking on Windows to avoid missing DLLs at runtime.
  cmake_args+=(
    "-DBUILD_SHARED_LIBS=OFF"
    "-DGGML_STATIC=ON"
    "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded"
    # Build a portable binary to avoid illegal instruction crashes on older CPUs.
    "-DGGML_NATIVE=OFF"
    "-DGGML_SSE42=OFF"
    "-DGGML_AVX=OFF"
    "-DGGML_AVX2=OFF"
    "-DGGML_BMI2=OFF"
  )
fi

if is_linux; then
  # Keep AppImage self-contained: avoid runtime dependency on local libwhisper/libggml sonames.
  cmake_args+=(
    "-DBUILD_SHARED_LIBS=OFF"
    "-DGGML_STATIC=ON"
    "-DGGML_BACKEND_DL=OFF"
  )
fi

if is_macos; then
  # Static-link so the bundled whisper-cli has no @rpath references to dylibs
  # outside the .app — those break notarization and runtime loading.
  cmake_args+=(
    "-DBUILD_SHARED_LIBS=OFF"
    "-DGGML_STATIC=ON"
    "-DGGML_BACKEND_DL=OFF"
  )
fi

echo "Configuring whisper.cpp..."
cmake "${cmake_args[@]}"

echo "Building whisper.cpp..."
cmake --build "$WHISPER_CPP_BUILD_DIR" --config Release -j "$(cpu_count)"

if ! built_binary="$(find_existing_binary)"; then
  echo "Error: whisper-cli not found. Checked:" >&2
  printf '  - %s\n' "${whisper_bin_candidates[@]}" >&2
  exit 1
fi

echo "whisper.cpp build output: $built_binary"
