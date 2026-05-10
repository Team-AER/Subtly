# Force MSVC static CRT for cmake-rs subprojects (whisper-rs-sys, etc.) so
# their C/C++ objects match Rust's `+crt-static` target-feature set in
# .cargo/config.toml. Without this, the C side links /MD (dynamic CRT,
# emitting __imp_* import references) while the Rust side links /MT
# (static libucrt.lib), and the final link fails with LNK2019 on dozens
# of stdio symbols.
#
# cmake-rs already passes `-MT` via CMAKE_C_FLAGS / CMAKE_CXX_FLAGS, but
# CMake policy CMP0091 (NEW since 3.15) silently drops /MT /MD from those
# flags and routes runtime selection through CMAKE_MSVC_RUNTIME_LIBRARY,
# which defaults to MultiThreadedDLL — hence the mismatch.
#
# Wired in via the CMAKE_TOOLCHAIN_FILE env var in .github/workflows/build.yml
# so it loads before whisper.cpp's project() call and the CACHE FORCE wins
# over any later assignment.
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    CACHE STRING "" FORCE)
