# Every test is a plain executable, not a framework. Each one that draws also
# takes `--write <dir>` and dumps what it drew as a PNG, because a failing
# pixel assertion is far easier to understand next to the picture.

find_package(Qt6 6.4 REQUIRED COMPONENTS Test)

# The C ABI gets its own harness, compiled as C *and* as C++: a Rust test
# proves nothing about the seam, and the header has to be usable from both.
add_executable(abi_test ../../crates/sc-ffi/tests/abi.c)
target_link_libraries(abi_test PRIVATE schcompare_core)
target_compile_options(abi_test PRIVATE -Wall -Wextra -Werror -pedantic)
add_dependencies(abi_test sc_core)
add_test(NAME abi COMMAND abi_test)

add_executable(abi_cxx_test ../../crates/sc-ffi/tests/abi_cxx.cpp)
target_link_libraries(abi_cxx_test PRIVATE schcompare_core)
target_compile_options(abi_cxx_test PRIVATE -Wall -Wextra -Werror -pedantic)
add_dependencies(abi_cxx_test sc_core)
add_test(NAME abi_cxx COMMAND abi_cxx_test)

# Neither harness has a Q_OBJECT in it; moc would only add a compile step.
set_target_properties(abi_test abi_cxx_test PROPERTIES AUTOMOC OFF)

if(UNIX)
    set_target_properties(abi_test abi_cxx_test PROPERTIES
        BUILD_RPATH "${SC_CARGO_OUT}")
endif()

# The documents the window test compares. Written by a Rust helper into the
# build tree, so the window's own behaviour is exercised on a clone that has no
# customer drawings in it. The real sets, when present, are checked as well.
set(SC_FIXTURE_DIR "${CMAKE_BINARY_DIR}/fixtures")
add_custom_target(sc_fixtures ALL
    BYPRODUCTS "${SC_FIXTURE_DIR}/a.pdf" "${SC_FIXTURE_DIR}/b.pdf"
    COMMAND ${CMAKE_COMMAND} -E env "CARGO_TARGET_DIR=${SC_CARGO_DIR}"
            "${CARGO_EXECUTABLE}" run -q -p sc-fixture --bin gen-fixtures
            ${SC_CARGO_FLAGS} ${SC_CARGO_TARGET_FLAGS} -- "${SC_FIXTURE_DIR}"
    WORKING_DIRECTORY "${SC_CORE_ROOT}"
    COMMENT "writing the window test's fixture documents"
    USES_TERMINAL VERBATIM)

# The window, driven the way a reader drives it. Qt's offscreen platform, so no
# X server and no window manager are involved.
add_executable(view_test view_test.cpp)
target_link_libraries(view_test PRIVATE sc_shell Qt6::Test)
target_compile_definitions(view_test PRIVATE
    SC_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}"
    SC_FIXTURE_DIR="${SC_FIXTURE_DIR}")
target_compile_options(view_test PRIVATE ${SC_WARNINGS})
add_dependencies(view_test sc_fixtures)
add_test(NAME view COMMAND view_test)
set_tests_properties(view PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
if(UNIX)
    set_target_properties(view_test PROPERTIES BUILD_RPATH "${SC_CARGO_OUT}")
endif()
