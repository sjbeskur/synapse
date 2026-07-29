cmake_minimum_required(VERSION 3.20)

project(synapse_cpp_canary LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(SYNAPSE_REPO_ROOT "${CMAKE_CURRENT_LIST_DIR}/../..")
set(SYNAPSE_IDL_DIR "${CMAKE_CURRENT_LIST_DIR}/syn")
set(SYNAPSE_GENERATED_DIR "${CMAKE_CURRENT_LIST_DIR}/generated")

find_program(SYNAPSE_BIN synapse)
if(SYNAPSE_BIN)
    set(SYNAPSE_COMMAND "${SYNAPSE_BIN}")
else()
    find_program(CARGO_BIN cargo REQUIRED)
    set(
        SYNAPSE_COMMAND
        "${CARGO_BIN}" "run"
        "--manifest-path" "${SYNAPSE_REPO_ROOT}/Cargo.toml"
        "-p" "cfs-synapse"
        "--locked"
        "--"
    )
endif()

add_custom_command(
    OUTPUT
        "${SYNAPSE_GENERATED_DIR}/demo_msgs.h"
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${SYNAPSE_GENERATED_DIR}"
    COMMAND
        ${SYNAPSE_COMMAND}
        --lang c
        -o "${SYNAPSE_GENERATED_DIR}"
        "${SYNAPSE_IDL_DIR}/demo_msgs.syn"
    DEPENDS
        "${SYNAPSE_IDL_DIR}/demo_msgs.syn"
    WORKING_DIRECTORY "${SYNAPSE_REPO_ROOT}"
    VERBATIM
)

add_custom_target(
    generate_synapse_cpp_canary_headers
    DEPENDS
        "${SYNAPSE_GENERATED_DIR}/demo_msgs.h"
)

add_executable(synapse_cpp_canary src/main.cpp)
add_dependencies(synapse_cpp_canary generate_synapse_cpp_canary_headers)
target_include_directories(
    synapse_cpp_canary
    PRIVATE
        "${SYNAPSE_GENERATED_DIR}"
        "${CMAKE_CURRENT_LIST_DIR}/include"
)
