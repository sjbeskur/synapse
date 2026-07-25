use super::*;

fn p(input: &str) -> SynFile {
    parse(input).expect("parse failed")
}

// ── Namespace ────────────────────────────────────────────

#[test]
fn namespace_simple() {
    let f = p("namespace geometry");
    assert_eq!(
        f.items[0],
        Item::Namespace(NamespaceDecl {
            name: vec!["geometry".into()]
        })
    );
}

#[test]
fn namespace_qualified() {
    let f = p("namespace nav::msgs");
    assert_eq!(
        f.items[0],
        Item::Namespace(NamespaceDecl {
            name: vec!["nav".into(), "msgs".into()]
        })
    );
}

// ── Import ───────────────────────────────────────────────

#[test]
fn import_path() {
    let f = p(r#"import "geometry.syn""#);
    assert_eq!(
        f.items[0],
        Item::Import(ImportDecl {
            path: "geometry.syn".into()
        })
    );
}

// ── Const ────────────────────────────────────────────────

#[test]
fn const_float() {
    let f = p("const PI: f64 = 3.14");
    assert_eq!(
        f.items[0],
        Item::Const(ConstDecl {
            name: "PI".into(),
            ty: TypeExpr {
                base: BaseType::Primitive(PrimitiveType::F64),
                array: None
            },
            value: Literal::Float(3.14),
            doc: vec![],
            attrs: vec![],
        })
    );
}

#[test]
fn const_int() {
    let f = p("const MAX: u32 = 256");
    assert_eq!(
        f.items[0],
        Item::Const(ConstDecl {
            name: "MAX".into(),
            ty: TypeExpr {
                base: BaseType::Primitive(PrimitiveType::U32),
                array: None
            },
            value: Literal::Int(256),
            doc: vec![],
            attrs: vec![],
        })
    );
}

#[test]
fn const_string() {
    let f = p(r#"const FRAME: string = "world""#);
    assert_eq!(
        f.items[0],
        Item::Const(ConstDecl {
            name: "FRAME".into(),
            ty: TypeExpr {
                base: BaseType::String,
                array: None
            },
            value: Literal::Str("world".into()),
            doc: vec![],
            attrs: vec![],
        })
    );
}

// ── Enum ─────────────────────────────────────────────────

#[test]
fn enum_with_values() {
    let f = p("enum DriveMode { Idle = 0  Forward = 1  Error = 2 }");
    let Item::Enum(e) = &f.items[0] else { panic!() };
    assert_eq!(e.name, "DriveMode");
    assert_eq!(e.repr, None);
    assert_eq!(
        e.variants[0],
        EnumVariant {
            name: "Idle".into(),
            value: Some(0),
            doc: vec![]
        }
    );
    assert_eq!(
        e.variants[1],
        EnumVariant {
            name: "Forward".into(),
            value: Some(1),
            doc: vec![]
        }
    );
    assert_eq!(
        e.variants[2],
        EnumVariant {
            name: "Error".into(),
            value: Some(2),
            doc: vec![]
        }
    );
    assert!(e.attrs.is_empty());
}

#[test]
fn enum_without_values() {
    let f = p("enum Dir { North South East West }");
    let Item::Enum(e) = &f.items[0] else { panic!() };
    assert_eq!(e.repr, None);
    assert!(e.variants.iter().all(|v| v.value.is_none()));
    assert_eq!(e.variants.len(), 4);
}

#[test]
fn enum_with_repr() {
    let f = p("enum u8 CameraMode { Idle = 0 Streaming = 1 }");
    let Item::Enum(e) = &f.items[0] else { panic!() };
    assert_eq!(e.name, "CameraMode");
    assert_eq!(e.repr, Some(PrimitiveType::U8));
    assert_eq!(e.variants.len(), 2);
}

// ── Struct ───────────────────────────────────────────────

#[test]
fn struct_basic() {
    let f = p("struct Point { x: f64 = 0.0  y: f64 = 0.0  z: f64 = 0.0 }");
    let Item::Struct(s) = &f.items[0] else {
        panic!()
    };
    assert_eq!(s.name, "Point");
    assert_eq!(s.fields.len(), 3);
    assert_eq!(s.fields[0].name, "x");
    assert_eq!(s.fields[0].ty.base, BaseType::Primitive(PrimitiveType::F64));
    assert_eq!(s.fields[0].default, Some(Literal::Float(0.0)));
    assert!(!s.fields[0].optional);
}

#[test]
fn struct_qualified_type() {
    let f = p("struct Pose { position: geometry::Point  orientation: geometry::Quaternion }");
    let Item::Struct(s) = &f.items[0] else {
        panic!()
    };
    assert_eq!(
        s.fields[0].ty.base,
        BaseType::Ref(vec!["geometry".into(), "Point".into()])
    );
}

// ── Message ──────────────────────────────────────────────

#[test]
fn message_optional_field() {
    let f = p("message Foo { required: i32  optional?: string }");
    let Item::Message(m) = &f.items[0] else {
        panic!()
    };
    assert_eq!(m.kind, PacketKind::Message);
    assert!(!m.fields[0].optional);
    assert!(m.fields[1].optional);
    assert_eq!(m.fields[1].ty.base, BaseType::String);
}

#[test]
fn command_packet_kind() {
    let f = p("@mid(0x1880)\ncommand SetMode { mode: u8 }");
    let Item::Command(m) = &f.items[0] else {
        panic!()
    };
    assert_eq!(m.kind, PacketKind::Command);
    assert_eq!(m.name, "SetMode");
    assert_eq!(m.fields[0].name, "mode");
}

#[test]
fn command_group_assigns_logical_topic_to_nested_commands() {
    let f = p("commands CameraCommands {
            @cc(1)
            command SetMode { mode: u8 }

            @cc(2)
            command SetExposure { exposure_us: u32 }
        }");
    assert_eq!(f.items.len(), 2);

    let Item::Command(set_mode) = &f.items[0] else {
        panic!()
    };
    assert_eq!(set_mode.command_group.as_deref(), Some("CameraCommands"));
    assert_eq!(set_mode.name, "SetMode");

    let Item::Command(set_exposure) = &f.items[1] else {
        panic!()
    };
    assert_eq!(
        set_exposure.command_group.as_deref(),
        Some("CameraCommands")
    );
    assert_eq!(set_exposure.name, "SetExposure");
}

#[test]
fn telemetry_packet_kind() {
    let f = p("@mid(0x0801)\ntelemetry NavState { x: f64 }");
    let Item::Telemetry(m) = &f.items[0] else {
        panic!()
    };
    assert_eq!(m.kind, PacketKind::Telemetry);
    assert_eq!(m.name, "NavState");
    assert_eq!(m.fields[0].name, "x");
}

#[test]
fn table_is_plain_data_item() {
    let f = p("table NavConfig { max_speed: f64  enabled: bool }");
    let Item::Table(t) = &f.items[0] else {
        panic!()
    };
    assert_eq!(t.name, "NavConfig");
    assert_eq!(t.fields.len(), 2);
}

#[test]
fn message_array_fields() {
    let f = p("message D { dynamic: u8[]  fixed: f64[3]  bounded: u8[<=256] }");
    let Item::Message(m) = &f.items[0] else {
        panic!()
    };
    assert_eq!(m.fields[0].ty.array, Some(ArraySuffix::Dynamic));
    assert_eq!(m.fields[1].ty.array, Some(ArraySuffix::Fixed(3)));
    assert_eq!(m.fields[2].ty.array, Some(ArraySuffix::Bounded(256)));
}

#[test]
fn message_enum_default() {
    let f = p("message S { mode: DriveMode = DriveMode::Idle }");
    let Item::Message(m) = &f.items[0] else {
        panic!()
    };
    assert_eq!(
        m.fields[0].default,
        Some(Literal::Ident(vec!["DriveMode".into(), "Idle".into()]))
    );
}

#[test]
fn message_string_bounded() {
    let f = p(r#"message S { label: string[<=64] = "robot" }"#);
    let Item::Message(m) = &f.items[0] else {
        panic!()
    };
    assert_eq!(m.fields[0].ty.base, BaseType::String);
    assert_eq!(m.fields[0].ty.array, Some(ArraySuffix::Bounded(64)));
    assert_eq!(m.fields[0].default, Some(Literal::Str("robot".into())));
}

// ── Hex literal ──────────────────────────────────────────

#[test]
fn hex_literal_const() {
    let f = p("const MID: u16 = 0x0801");
    let Item::Const(c) = &f.items[0] else {
        panic!()
    };
    assert_eq!(c.value, Literal::Hex(0x0801));
}

#[test]
fn hex_literal_uppercase() {
    let f = p("const MID: u16 = 0X1F80");
    let Item::Const(c) = &f.items[0] else {
        panic!()
    };
    assert_eq!(c.value, Literal::Hex(0x1F80));
}

// ── Attributes ───────────────────────────────────────────

#[test]
fn attribute_hex_on_message() {
    let f = p("@mid(0x0801)\nmessage NavTlm { x: f64 }");
    let Item::Message(m) = &f.items[0] else {
        panic!()
    };
    assert_eq!(m.attrs.len(), 1);
    assert_eq!(m.attrs[0].name, "mid");
    assert_eq!(m.attrs[0].value, Literal::Hex(0x0801));
}

#[test]
fn attribute_ident_ref() {
    let f = p("@mid(nav_app::NAV_TLM_MID)\nmessage NavTlm { x: f64 }");
    let Item::Message(m) = &f.items[0] else {
        panic!()
    };
    assert_eq!(
        m.attrs[0].value,
        Literal::Ident(vec!["nav_app".into(), "NAV_TLM_MID".into()])
    );
}

#[test]
fn no_attrs_is_empty() {
    let f = p("message Foo { x: i32 }");
    let Item::Message(m) = &f.items[0] else {
        panic!()
    };
    assert!(m.attrs.is_empty());
}

// ── String escape sequences ───────────────────────────────

#[test]
fn string_escape_sequences() {
    let f = p(r#"const S: string = "hello\nworld""#);
    let Item::Const(c) = &f.items[0] else {
        panic!()
    };
    assert_eq!(c.value, Literal::Str("hello\nworld".into()));
}

#[test]
fn string_escape_quote() {
    let f = p(r#"const S: string = "say \"hi\"""#);
    let Item::Const(c) = &f.items[0] else {
        panic!()
    };
    assert_eq!(c.value, Literal::Str("say \"hi\"".into()));
}

// ── Full realistic file ───────────────────────────────────

#[test]
fn full_robot_state() {
    let src = r#"
        namespace robot
        import "geometry.syn"

        enum DriveMode {
            Idle    = 0
            Forward = 1
            Error   = 2
        }

        const MAX_SPEED: f64 = 2.5

        message RobotState {
            mode:        DriveMode      = DriveMode::Idle
            position:    geometry::Point
            battery:     f32            = 100.0
            label:       string[<=64]   = "robot"
            sensor_data: u8[]
            error_code?: i32
        }
    "#;

    let f = parse(src).unwrap();
    assert_eq!(f.items.len(), 5);

    let Item::Namespace(ns) = &f.items[0] else {
        panic!()
    };
    assert_eq!(ns.name, vec!["robot"]);

    let Item::Enum(e) = &f.items[2] else { panic!() };
    assert_eq!(e.variants.len(), 3);

    let Item::Message(m) = &f.items[4] else {
        panic!()
    };
    assert_eq!(m.name, "RobotState");
    assert_eq!(m.fields.len(), 6);

    // last field is optional
    assert!(m.fields[5].optional);
    assert_eq!(m.fields[5].name, "error_code");
}
