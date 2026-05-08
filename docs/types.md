# Synapse Type Reference

This document lists the type forms accepted by the Synapse parser and summarizes the current cFS C/Rust code generation behavior.

See `docs/examples.md` for complete `.syn` files that use these types.

## Scalar Types

| Synapse | C | Rust |
| --- | --- | --- |
| `f32` | `float` | `f32` |
| `f64` | `double` | `f64` |
| `i8` | `int8_t` | `i8` |
| `i16` | `int16_t` | `i16` |
| `i32` | `int32_t` | `i32` |
| `i64` | `int64_t` | `i64` |
| `u8` | `uint8_t` | `u8` |
| `u16` | `uint16_t` | `u16` |
| `u32` | `uint32_t` | `u32` |
| `u64` | `uint64_t` | `u64` |
| `bool` | `bool` | `bool` |
| `bytes` | `uint8_t*` | `*const u8` |
| `string` | `const char*` | `*const u8` |
| `SomeType` | generated typedef name | `SomeType` |
| `pkg::SomeType` | generated namespaced typedef name | `pkg::SomeType` |
| represented enum | fixed-width typedef name | enum type alias |

Examples:

```syn
struct Scalars {
    temperature: f32
    position: f64
    count: u32
    enabled: bool
    payload: bytes
    frame_id: string
}
```

Prefer bounded strings such as `string[<=32]` for cFS packet and table payloads. Unbounded `string` and `bytes` generate pointer-like fields and need an external ownership/length convention.

## Enums

Represented enums use an explicit integer primitive before the enum name.

```syn
enum u8 CameraMode {
    Standby = 0
    Preview = 1
}
```

The cFS generator emits a fixed-width C typedef and Rust type alias, plus constants for each variant. Supported representations are `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, and `u64`. Every variant must have an explicit value that fits the selected representation.

Unrepresented enums parse, but cFS codegen rejects them as field types because they do not specify an ABI width.

## Fixed Arrays

Fixed arrays use `[N]` and generate inline storage.

| Synapse | C | Rust |
| --- | --- | --- |
| `f32[N]` | `float field[N];` | `[f32; N]` |
| `f64[N]` | `double field[N];` | `[f64; N]` |
| `i8[N]` | `int8_t field[N];` | `[i8; N]` |
| `i16[N]` | `int16_t field[N];` | `[i16; N]` |
| `i32[N]` | `int32_t field[N];` | `[i32; N]` |
| `i64[N]` | `int64_t field[N];` | `[i64; N]` |
| `u8[N]` | `uint8_t field[N];` | `[u8; N]` |
| `u16[N]` | `uint16_t field[N];` | `[u16; N]` |
| `u32[N]` | `uint32_t field[N];` | `[u32; N]` |
| `u64[N]` | `uint64_t field[N];` | `[u64; N]` |
| `bool[N]` | `bool field[N];` | `[bool; N]` |
| `SomeType[N]` | `some_namespace_SomeType_t field[N];` | `[SomeType; N]` |
| `string[N]` | `char field[N];` | `[u8; N]` |

Example:

```syn
struct CameraIntrinsics {
    k: f64[9]
    distortion: f64[5]
}
```

## Dynamic Arrays

Dynamic arrays use `[]`. They parse for all base types, but cFS codegen rejects them until the IDL defines an ownership and length model.

Example:

```syn
struct Polygon {
    points: Point32[]
}
```

Prefer fixed arrays or bounded strings for cFS packet and table ABI data.

## Bounded Arrays

Bounded arrays use `[<=N]`. `string[<=N]` is supported as inline storage. Non-string bounded arrays parse, but cFS codegen rejects them until the IDL defines whether they should generate inline storage plus an explicit length field.

Example:

```syn
struct Samples {
    values: f32[<=128]
}
```

For now, use fixed arrays such as `f32[128]` when the generated cFS packet/table layout needs inline numeric storage.

## Strings

Strings are special-cased because cFS packets and tables often need inline byte storage for identifiers, frame names, labels, and similar values.

| Synapse | C | Rust | Recommended for cFS ABI |
| --- | --- | --- | --- |
| `string` | `const char*` | `*const u8` | No |
| `string[]` | cFS codegen error | cFS codegen error | No |
| `string[N]` | `char field[N];` | `[u8; N]` | Yes, if fixed length is desired |
| `string[<=N]` | `char field[N];` | `[u8; N]` | Yes |

Example:

```syn
struct CameraId {
    name: string[<=32]
}
```

`string[N]` and `string[<=N]` both generate exactly `N` bytes of inline storage. Synapse does not guarantee null termination, text encoding, UTF-8 validity, or that the buffer contains a C string. Treat the generated field as an inline byte buffer whose interpretation belongs to the mission/application code.

Many cFS projects will still choose to use these inline buffers with a C-string convention. Under that convention, `N` is the total byte capacity including the null terminator, so `string[<=32]` can hold at most 31 non-null bytes plus `\0`. Readers find the actual length by scanning for the first null byte, bounded by `N`; if no null byte is present, the buffer should be treated as a full `N` bytes and not passed to APIs that require null termination.

Rust code can convert such a buffer by slicing to the first null byte and then validating UTF-8:

```rust
fn c_string_bytes(buf: &[u8]) -> &[u8] {
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    &buf[..end]
}

fn c_string_str(buf: &[u8]) -> Result<&str, core::str::Utf8Error> {
    core::str::from_utf8(c_string_bytes(buf))
}
```

C++ code can use a bounded scan such as `strnlen` and expose the result as `std::string_view`. Future Synapse support libraries may provide these Rust and C++ helpers, but the generated packet/table layout should remain plain inline storage.

If a future release needs strict C-string semantics, it should use explicit syntax such as `cstring[<=N]` rather than changing `string[<=N]` silently.

## Field Forms

The parser accepts these field forms:

```syn
field: Type
field?: Type
field: Type = value
field?: Type = value
```

In `0.2.x`, cFS codegen rejects optional markers, defaults, dynamic arrays, and non-string bounded arrays until concrete ABI and initializer semantics exist.
