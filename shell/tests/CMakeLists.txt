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
