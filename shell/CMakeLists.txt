cmake_minimum_required(VERSION 3.21)
project(sch-pdf-compare VERSION 0.1.0 LANGUAGES C CXX)

# C is enabled for one reason: the ABI harness must be compiled as C, not only
# as C++. Without it CMake falls back to the C++ compiler for a .c file and the
# "does this header work from C" half of that test silently stops being run.

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Debug CACHE STRING "" FORCE)
endif()

# PrintSupport is a separate qtbase module. A reviewer who has found the change
# wants it on paper next to the old sheet, so printing is not optional here.
find_package(Qt6 6.4 REQUIRED COMPONENTS Widgets PrintSupport)

# `-Wall -Wextra` is GCC and Clang's spelling. MSVC takes `-` as a switch lead
# too, so `-Wall` reaches it and means "every warning including the ones the
# standard library trips over"; `-Wextra` is not a switch it has at all.
if(MSVC)
    set(SC_WARNINGS /W4)
else()
    set(SC_WARNINGS -Wall -Wextra)
endif()

# --- the Rust core -----------------------------------------------------------
#
# Cargo is driven from here so one `cmake --build` produces a runnable shell,
# and its target directory is *inside the CMake build tree* rather than the
# workspace's own `crates/target`. A build tree per toolchain costs one extra
# compile of the core and removes the question of which glibc a shared target
# directory's library was linked against.

set(SC_CORE_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../crates")
set(SC_CARGO_DIR "${CMAKE_BINARY_DIR}/cargo")

find_program(CARGO_EXECUTABLE cargo
    HINTS "$ENV{HOME}/.cargo/bin" "$ENV{CARGO_HOME}/bin")
if(NOT CARGO_EXECUTABLE)
    message(FATAL_ERROR
        "cargo not found. Export $HOME/.cargo/bin, or pass -DCARGO_EXECUTABLE=.")
endif()

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(SC_CARGO_FLAGS "")
    set(SC_CARGO_SUBDIR debug)
else()
    set(SC_CARGO_FLAGS --release)
    set(SC_CARGO_SUBDIR release)
endif()

# Cross-compiling needs `--target`, and passing it moves cargo's output down a
# directory named after the triple. A *native* build must not be given one:
# `--target` even for the host disables the `[build]` config the workspace may
# rely on and rebuilds proc macros, so the two cases genuinely differ.
if(NOT DEFINED SC_CARGO_TARGET)
    if(CMAKE_CROSSCOMPILING AND WIN32)
        set(SC_CARGO_TARGET "x86_64-pc-windows-gnu")
    else()
        set(SC_CARGO_TARGET "")
    endif()
endif()
set(SC_CARGO_TARGET "${SC_CARGO_TARGET}" CACHE STRING
    "Rust target triple for the core, empty for the host's own")

if(SC_CARGO_TARGET)
    set(SC_CARGO_TARGET_FLAGS --target "${SC_CARGO_TARGET}")
    set(SC_CARGO_OUT "${SC_CARGO_DIR}/${SC_CARGO_TARGET}/${SC_CARGO_SUBDIR}")
else()
    set(SC_CARGO_TARGET_FLAGS "")
    set(SC_CARGO_OUT "${SC_CARGO_DIR}/${SC_CARGO_SUBDIR}")
endif()

# Cargo's own names, not CMake's. `CMAKE_SHARED_LIBRARY_PREFIX` is `lib` for a
# MinGW target, and cargo emits `schcompare.dll` there — taking CMake's answer
# looks right and points at a file that is never written.
if(WIN32)
    set(SC_CORE_LIB "${SC_CARGO_OUT}/schcompare.dll")
    if(MSVC)
        set(SC_CORE_IMPLIB "${SC_CARGO_OUT}/schcompare.dll.lib")
    else()
        set(SC_CORE_IMPLIB "${SC_CARGO_OUT}/libschcompare.dll.a")
    endif()
else()
    set(SC_CORE_LIB "${SC_CARGO_OUT}/libschcompare.so")
    set(SC_CORE_IMPLIB "")
endif()

# Always run: cargo does its own dependency tracking and is a no-op when
# nothing changed, which is cheaper and more correct than globbing the
# workspace's sources into a DEPENDS list that silently misses a new file.
add_custom_target(sc_core ALL
    BYPRODUCTS "${SC_CORE_LIB}" ${SC_CORE_IMPLIB}
    COMMAND ${CMAKE_COMMAND} -E env "CARGO_TARGET_DIR=${SC_CARGO_DIR}"
            "${CARGO_EXECUTABLE}" build -p sc-ffi ${SC_CARGO_FLAGS}
            ${SC_CARGO_TARGET_FLAGS}
    WORKING_DIRECTORY "${SC_CORE_ROOT}"
    COMMENT "cargo build -p sc-ffi"
    USES_TERMINAL VERBATIM)

# Windows has no rpath, so the loader looks beside the executable and nowhere
# else useful. It belongs on `sc_core` rather than on the application, because
# the tests are run without ever building the application.
if(WIN32)
    add_custom_command(TARGET sc_core POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
                "${SC_CORE_LIB}" "${CMAKE_BINARY_DIR}"
        COMMENT "schcompare.dll beside the executables")
endif()

add_library(schcompare_core SHARED IMPORTED GLOBAL)
set_target_properties(schcompare_core PROPERTIES
    IMPORTED_LOCATION "${SC_CORE_LIB}"
    INTERFACE_INCLUDE_DIRECTORIES "${SC_CORE_ROOT}/sc-ffi/include")
if(WIN32)
    set_target_properties(schcompare_core PROPERTIES
        IMPORTED_IMPLIB "${SC_CORE_IMPLIB}")
endif()

# --- the shell ---------------------------------------------------------------
#
# A static library rather than sources listed on the executable, so the test
# binaries link the same objects the application does.

add_library(sc_shell STATIC
    src/ColourDialog.cpp
    src/CompareView.cpp
    src/MainWindow.cpp
    src/Session.cpp
)
target_include_directories(sc_shell PUBLIC src)
target_compile_options(sc_shell PRIVATE ${SC_WARNINGS})
target_link_libraries(sc_shell PUBLIC
    Qt6::Widgets Qt6::PrintSupport schcompare_core)
add_dependencies(sc_shell sc_core)

add_executable(sch-pdf-compare src/main.cpp)
target_compile_options(sch-pdf-compare PRIVATE ${SC_WARNINGS})
target_link_libraries(sch-pdf-compare PRIVATE sc_shell)
set_target_properties(sch-pdf-compare PROPERTIES
    WIN32_EXECUTABLE ON
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")

# The loader has to find the core next to cargo's output during development,
# and beside the installed binary afterwards.
if(UNIX)
    set_target_properties(sch-pdf-compare PROPERTIES
        BUILD_RPATH "${SC_CARGO_OUT}"
        INSTALL_RPATH "$ORIGIN/../lib")
endif()

enable_testing()
add_subdirectory(tests)
