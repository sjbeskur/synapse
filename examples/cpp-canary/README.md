# Synapse C++ Canary

This small CMake project demonstrates using Synapse-generated C headers from C++.

It generates cFS-shaped C headers from `.syn` files into `generated/`, provides a tiny local `cfe.h` stub, and compiles a C++ executable that constructs a command and telemetry packet using the generated ABI types.

Build and run from the repository root:

```bash
cmake -S examples/cpp-canary -B /tmp/synapse-cpp-canary-build
cmake --build /tmp/synapse-cpp-canary-build
/tmp/synapse-cpp-canary-build/synapse_cpp_canary
```

The interesting pieces are:

- `syn/demo_msgs.syn`: declares a logical command group and telemetry topic.
- Command codes remain schema-owned through `@cc(...)`; the mission manifest
  owns the runtime cFS message IDs.
- `CMakeLists.txt`: regenerates `generated/*.h` with the `synapse` CLI.
- `include/cfe.h`: minimal cFS packet-header stub for standalone compilation.
- `src/main.cpp`: includes and uses the generated C headers from C++.
