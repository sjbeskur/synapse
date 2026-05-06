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

Strings are special-cased because cFS packets and tables often need inline character buffers.

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

`string[<=N]` currently generates the same inline representation as `string[N]`. The `<=` form documents that the logical string length is bounded by `N`.

## Field Forms

The parser accepts these field forms:

```syn
field: Type
field?: Type
field: Type = value
field?: Type = value
```

In `0.2.x`, cFS codegen rejects optional markers, defaults, dynamic arrays, and non-string bounded arrays until concrete ABI and initializer semantics exist.
