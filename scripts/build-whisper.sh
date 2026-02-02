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

whisper_bin_name="whisper-cli"
if is_windows; then
  whisper_bin_name="whisper-cli.exe"
fi

built_binary="$WHISPER_CPP_BUILD_DIR/bin/$whisper_bin_name"

if [[ -n "${WHISPER_CPP_FORCE_REBUILD:-}" ]]; then
  rm -rf "$WHISPER_CPP_BUILD_DIR"
fi

if [[ -f "$built_binary" ]]; then
  echo "whisper.cpp already built at $built_binary"
  exit 0
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

if [[ "$(uname -s)" == "Darwin" ]]; then
  cmake_args+=("-DGGML_METAL=ON")
fi

echo "Configuring whisper.cpp..."
cmake "${cmake_args[@]}"

echo "Building whisper.cpp..."
cmake --build "$WHISPER_CPP_BUILD_DIR" --config Release -j "$(cpu_count)"

if [[ ! -f "$built_binary" ]]; then
  echo "Error: whisper-cli not found at $built_binary" >&2
  exit 1
fi

echo "whisper.cpp build output: $built_binary"
