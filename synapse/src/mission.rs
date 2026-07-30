use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

use synapse_codegen_cfs::CfsPacketKind;

use crate::{
    Error, ImportGraph, ParsedUnit, collect_input_paths, imported_constants_for_unit,
    load_import_graphs, units_by_path, validate_cfs_graph,
};

const MANIFEST_VERSION: u64 = 1;
const MAX_TOPIC_ID: u64 = u16::MAX as u64;

/// Mission-owned assignments from logical Synapse topics to cFE topic IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionManifest {
    version: u64,
    command_topics: BTreeMap<String, u16>,
    telemetry_topics: BTreeMap<String, u16>,
}

impl MissionManifest {
    /// Parse a mission manifest from TOML text.
    pub fn parse(source: &str) -> Result<Self, Error> {
        let value = toml::from_str::<toml::Value>(source)
            .map_err(|error| Error::Manifest(format!("invalid mission manifest TOML: {error}")))?;
        parse_manifest_value(&value)
    }

    /// Read and parse a mission manifest from disk.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let source = fs::read_to_string(path).map_err(|error| {
            Error::Manifest(format!(
                "error reading mission manifest `{}`: {error}",
                path.display()
            ))
        })?;
        Self::parse(&source).map_err(|error| {
            Error::Manifest(format!(
                "error parsing mission manifest `{}`: {error}",
                path.display()
            ))
        })
    }

    /// Manifest format version.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Command-topic assignments, keyed by fully qualified logical topic.
    pub fn command_topics(&self) -> &BTreeMap<String, u16> {
        &self.command_topics
    }

    /// Telemetry-topic assignments, keyed by fully qualified logical topic.
    pub fn telemetry_topics(&self) -> &BTreeMap<String, u16> {
        &self.telemetry_topics
    }
}

/// Validate one or more schema roots against a mission manifest.
pub fn check_paths_with_manifest<I, P>(inputs: I, manifest: &MissionManifest) -> Result<(), Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    prepare_topics(inputs, manifest)?;
    Ok(())
}

/// Generate a cFE routing header from schema roots and a mission manifest.
pub fn generate_routing_header<I, P>(inputs: I, manifest: &MissionManifest) -> Result<String, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let topics = prepare_topics(inputs, manifest)?;
    Ok(render_routing_header(&topics, manifest))
}

/// Generate and write a cFE routing header.
pub fn write_routing_header<I, P>(
    inputs: I,
    manifest: &MissionManifest,
    output: impl AsRef<Path>,
) -> Result<PathBuf, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let header = generate_routing_header(inputs, manifest)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, header)?;
    Ok(output.to_path_buf())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TopicKind {
    Command,
    Telemetry,
}

impl TopicKind {
    fn label(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Telemetry => "telemetry",
        }
    }

    fn mapping_macro(self) -> &'static str {
        match self {
            Self::Command => "CFE_PLATFORM_CMD_TOPICID_TO_MIDV",
            Self::Telemetry => "CFE_PLATFORM_TLM_TOPICID_TO_MIDV",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LogicalTopic {
    kind: TopicKind,
    name: String,
}

fn parse_manifest_value(value: &toml::Value) -> Result<MissionManifest, Error> {
    let root = value
        .as_table()
        .ok_or_else(|| manifest_error("manifest root must be a TOML table"))?;
    reject_unknown_keys(root, &["version", "topics"], "manifest root")?;

    let version = required_integer(root, "version", "manifest root")?;
    if version != MANIFEST_VERSION {
        return Err(manifest_error(format!(
            "unsupported manifest version `{version}`; expected `{MANIFEST_VERSION}`"
        )));
    }

    let topics = required_table(root, "topics", "manifest root")?;
    reject_unknown_keys(topics, &["command", "telemetry"], "`topics`")?;
    let command_topics = parse_topic_assignments(topics.get("command"), TopicKind::Command)?;
    let telemetry_topics = parse_topic_assignments(topics.get("telemetry"), TopicKind::Telemetry)?;

    Ok(MissionManifest {
        version,
        command_topics,
        telemetry_topics,
    })
}

fn required_integer(table: &toml::Table, key: &str, context: &str) -> Result<u64, Error> {
    let value = table
        .get(key)
        .ok_or_else(|| manifest_error(format!("missing `{key}` in {context}")))?;
    let value = value
        .as_integer()
        .ok_or_else(|| manifest_error(format!("`{key}` in {context} must be an integer")))?;
    u64::try_from(value)
        .map_err(|_| manifest_error(format!("`{key}` in {context} must not be negative")))
}

fn required_table<'a>(
    table: &'a toml::Table,
    key: &str,
    context: &str,
) -> Result<&'a toml::Table, Error> {
    table
        .get(key)
        .ok_or_else(|| manifest_error(format!("missing `{key}` table in {context}")))?
        .as_table()
        .ok_or_else(|| manifest_error(format!("`{key}` in {context} must be a table")))
}

fn parse_topic_assignments(
    value: Option<&toml::Value>,
    kind: TopicKind,
) -> Result<BTreeMap<String, u16>, Error> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let table = value
        .as_table()
        .ok_or_else(|| manifest_error(format!("`topics.{}` must be a table", kind.label())))?;
    let mut assignments = BTreeMap::new();
    let mut used_ids = HashMap::<u16, String>::new();

    for (topic, value) in table {
        if topic.trim().is_empty() {
            return Err(manifest_error(format!(
                "`topics.{}` contains an empty topic name",
                kind.label()
            )));
        }
        let integer = value.as_integer().ok_or_else(|| {
            manifest_error(format!(
                "topic ID for {} topic `{topic}` must be an integer",
                kind.label()
            ))
        })?;
        let topic_id = u64::try_from(integer).map_err(|_| {
            manifest_error(format!(
                "topic ID for {} topic `{topic}` must not be negative",
                kind.label()
            ))
        })?;
        if topic_id > MAX_TOPIC_ID {
            return Err(manifest_error(format!(
                "topic ID for {} topic `{topic}` exceeds 0xFFFF",
                kind.label()
            )));
        }
        let topic_id = topic_id as u16;
        if let Some(first_topic) = used_ids.insert(topic_id, topic.clone()) {
            return Err(manifest_error(format!(
                "duplicate {} topic ID `0x{topic_id:04X}` assigned to `{first_topic}` and `{topic}`",
                kind.label()
            )));
        }
        assignments.insert(topic.clone(), topic_id);
    }

    Ok(assignments)
}

fn reject_unknown_keys(table: &toml::Table, allowed: &[&str], context: &str) -> Result<(), Error> {
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(manifest_error(format!("unknown key `{key}` in {context}")));
        }
    }
    Ok(())
}

fn manifest_error(message: impl Into<String>) -> Error {
    Error::Manifest(message.into())
}

fn prepare_topics<I, P>(inputs: I, manifest: &MissionManifest) -> Result<Vec<LogicalTopic>, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let inputs = collect_input_paths(inputs, "mission routing")?;
    let graph = load_import_graphs(&inputs)?;
    validate_cfs_graph(&graph)?;
    let topics = collect_logical_topics(&graph)?;
    validate_topic_symbols(&topics)?;
    validate_assignments(&topics, manifest)?;
    Ok(topics)
}

fn collect_logical_topics(graph: &ImportGraph) -> Result<Vec<LogicalTopic>, Error> {
    let units = units_by_path(graph);
    let mut topics = BTreeSet::new();

    for unit in &graph.units {
        collect_unit_topics(unit, &units, &mut topics)?;
    }

    if topics.is_empty() {
        return Err(Error::Mission(
            "mission routing requires at least one logical command or telemetry topic".to_string(),
        ));
    }

    Ok(topics.into_iter().collect())
}

fn collect_unit_topics(
    unit: &ParsedUnit,
    units: &HashMap<PathBuf, &ParsedUnit>,
    topics: &mut BTreeSet<LogicalTopic>,
) -> Result<(), Error> {
    let imported_constants = imported_constants_for_unit(unit, units)?;
    let packets =
        synapse_codegen_cfs::collect_cfs_packets_with_constants(&unit.file, &imported_constants)?;

    for packet in packets {
        let mut qualified = packet.namespace;
        qualified.push(packet.topic);
        topics.insert(LogicalTopic {
            kind: match packet.kind {
                CfsPacketKind::Command => TopicKind::Command,
                CfsPacketKind::Telemetry => TopicKind::Telemetry,
            },
            name: qualified.join("::"),
        });
    }
    Ok(())
}

fn validate_assignments(topics: &[LogicalTopic], manifest: &MissionManifest) -> Result<(), Error> {
    let required_command = topics_for_kind(topics, TopicKind::Command);
    let required_telemetry = topics_for_kind(topics, TopicKind::Telemetry);
    let assigned_command = manifest.command_topics.keys().cloned().collect();
    let assigned_telemetry = manifest.telemetry_topics.keys().cloned().collect();

    let mut problems = Vec::new();
    collect_wrong_kind(
        &required_command,
        &assigned_telemetry,
        TopicKind::Command,
        &mut problems,
    );
    collect_wrong_kind(
        &required_telemetry,
        &assigned_command,
        TopicKind::Telemetry,
        &mut problems,
    );
    collect_missing(
        &required_command,
        &assigned_command,
        &assigned_telemetry,
        TopicKind::Command,
        &mut problems,
    );
    collect_missing(
        &required_telemetry,
        &assigned_telemetry,
        &assigned_command,
        TopicKind::Telemetry,
        &mut problems,
    );
    collect_extra(
        &assigned_command,
        &required_command,
        &required_telemetry,
        TopicKind::Command,
        &mut problems,
    );
    collect_extra(
        &assigned_telemetry,
        &required_telemetry,
        &required_command,
        TopicKind::Telemetry,
        &mut problems,
    );

    if problems.is_empty() {
        Ok(())
    } else {
        Err(Error::Mission(format!(
            "mission topic assignments do not match the schemas:\n  {}",
            problems.join("\n  ")
        )))
    }
}

fn topics_for_kind(topics: &[LogicalTopic], kind: TopicKind) -> BTreeSet<String> {
    topics
        .iter()
        .filter(|topic| topic.kind == kind)
        .map(|topic| topic.name.clone())
        .collect()
}

fn collect_wrong_kind(
    required: &BTreeSet<String>,
    assigned_other: &BTreeSet<String>,
    actual_kind: TopicKind,
    problems: &mut Vec<String>,
) {
    for topic in required.intersection(assigned_other) {
        problems.push(format!(
            "{} topic `{topic}` is assigned under `topics.{}`",
            actual_kind.label(),
            opposite_kind(actual_kind).label()
        ));
    }
}

fn collect_missing(
    required: &BTreeSet<String>,
    assigned: &BTreeSet<String>,
    assigned_other: &BTreeSet<String>,
    kind: TopicKind,
    problems: &mut Vec<String>,
) {
    for topic in required.difference(assigned) {
        if !assigned_other.contains(topic) {
            problems.push(format!(
                "missing {} topic assignment for `{topic}`",
                kind.label()
            ));
        }
    }
}

fn collect_extra(
    assigned: &BTreeSet<String>,
    required: &BTreeSet<String>,
    required_other: &BTreeSet<String>,
    kind: TopicKind,
    problems: &mut Vec<String>,
) {
    for topic in assigned.difference(required) {
        if !required_other.contains(topic) {
            problems.push(format!(
                "unknown {} topic assignment `{topic}`",
                kind.label()
            ));
        }
    }
}

fn opposite_kind(kind: TopicKind) -> TopicKind {
    match kind {
        TopicKind::Command => TopicKind::Telemetry,
        TopicKind::Telemetry => TopicKind::Command,
    }
}

fn validate_topic_symbols(topics: &[LogicalTopic]) -> Result<(), Error> {
    let mut symbols = HashMap::<String, &LogicalTopic>::new();
    for topic in topics {
        let symbol = topic_symbol(&topic.name);
        if let Some(first) = symbols.insert(symbol.clone(), topic)
            && first != topic
        {
            return Err(Error::Mission(format!(
                "{} topic `{}` and {} topic `{}` both generate the C symbol `{symbol}`; rename one logical topic",
                first.kind.label(),
                first.name,
                topic.kind.label(),
                topic.name
            )));
        }
    }
    Ok(())
}

fn render_routing_header(topics: &[LogicalTopic], manifest: &MissionManifest) -> String {
    let mut out = format!(
        "/* Generated by Synapse {}. Do not edit directly. */\n\
         #pragma once\n\
         #include \"cfe_core_api_msgid_mapping.h\"\n\n",
        env!("CARGO_PKG_VERSION")
    );

    for kind in [TopicKind::Command, TopicKind::Telemetry] {
        let selected = topics
            .iter()
            .filter(|topic| topic.kind == kind)
            .collect::<Vec<_>>();
        if selected.is_empty() {
            continue;
        }
        out.push_str(&format!("/* {} Topics */\n", title_case(kind.label())));
        for topic in selected {
            let topic_id = assignment_for(manifest, topic);
            let symbol = topic_symbol(&topic.name);
            out.push_str(&format!(
                "/* {} */\n\
                 #define {symbol}_TOPICID  0x{topic_id:04X}U\n\
                 #define {symbol}_MID      {}({symbol}_TOPICID)\n\n",
                topic.name,
                kind.mapping_macro()
            ));
        }
    }

    out
}

fn assignment_for(manifest: &MissionManifest, topic: &LogicalTopic) -> u16 {
    let assignments = match topic.kind {
        TopicKind::Command => &manifest.command_topics,
        TopicKind::Telemetry => &manifest.telemetry_topics,
    };
    *assignments
        .get(&topic.name)
        .expect("mission assignments were validated before rendering")
}

fn topic_symbol(topic: &str) -> String {
    topic
        .split("::")
        .map(to_screaming_snake)
        .collect::<Vec<_>>()
        .join("_")
}

fn to_screaming_snake(name: &str) -> String {
    let mut out = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            out.push('_');
        }
        out.push(character.to_ascii_uppercase());
    }
    out
}

fn title_case(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + characters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const SCHEMA: &str = r#"namespace camera_app

commands CameraCommands {
    @cc(1)
    command SetMode {
        mode: u8
    }

    @cc(2)
    command SetExposure {
        exposure_us: u32
    }
}

telemetry CameraStatus {
    mode: u8
}
"#;

    const MANIFEST: &str = r#"version = 1

[topics.command]
"camera_app::CameraCommands" = 0x82

[topics.telemetry]
"camera_app::CameraStatus" = 0x82
"#;

    #[test]
    fn parses_separate_command_and_telemetry_topic_spaces() {
        let manifest = MissionManifest::parse(MANIFEST).unwrap();

        assert_eq!(manifest.version(), 1);
        assert_eq!(
            manifest.command_topics().get("camera_app::CameraCommands"),
            Some(&0x82)
        );
        assert_eq!(
            manifest.telemetry_topics().get("camera_app::CameraStatus"),
            Some(&0x82)
        );
    }

    #[test]
    fn rejects_duplicate_topic_ids_within_one_space() {
        let error = MissionManifest::parse(
            r#"version = 1
[topics.command]
"camera_app::CameraCommands" = 0x82
"nav_app::NavCommands" = 0x82
"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains(
            "duplicate command topic ID `0x0082` assigned to `camera_app::CameraCommands` and `nav_app::NavCommands`"
        ));
    }

    #[test]
    fn rejects_unknown_manifest_keys() {
        let error = MissionManifest::parse(
            r#"version = 1
topic = {}
"#,
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "unknown key `topic` in manifest root");
    }

    #[test]
    fn rejects_unsupported_manifest_versions() {
        let error = MissionManifest::parse(
            r#"version = 2
[topics]
"#,
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "unsupported manifest version `2`; expected `1`"
        );
    }

    #[test]
    fn rejects_topic_ids_wider_than_cfe_topic_storage() {
        let error = MissionManifest::parse(
            r#"version = 1
[topics.telemetry]
"camera_app::CameraStatus" = 0x10000
"#,
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "topic ID for telemetry topic `camera_app::CameraStatus` exceeds 0xFFFF"
        );
    }

    #[test]
    fn generates_namespaced_cfe_routing_macros() {
        let (dir, schema) = write_schema("generates-routing-header", SCHEMA);
        let manifest = MissionManifest::parse(MANIFEST).unwrap();

        let header = generate_routing_header([&schema], &manifest).unwrap();

        assert!(header.contains("#include \"cfe_core_api_msgid_mapping.h\""));
        assert!(header.contains("/* camera_app::CameraCommands */"));
        assert!(header.contains("#define CAMERA_APP_CAMERA_COMMANDS_TOPICID  0x0082U"));
        assert!(header.contains(
            "#define CAMERA_APP_CAMERA_COMMANDS_MID      CFE_PLATFORM_CMD_TOPICID_TO_MIDV(CAMERA_APP_CAMERA_COMMANDS_TOPICID)"
        ));
        assert!(header.contains("#define CAMERA_APP_CAMERA_STATUS_TOPICID  0x0082U"));
        assert!(header.contains(
            "#define CAMERA_APP_CAMERA_STATUS_MID      CFE_PLATFORM_TLM_TOPICID_TO_MIDV(CAMERA_APP_CAMERA_STATUS_TOPICID)"
        ));
        assert!(!dir.join("mission.toml").exists());
    }

    #[test]
    fn reports_missing_and_unknown_assignments() {
        let (_dir, schema) = write_schema("assignment-mismatch", SCHEMA);
        let manifest = MissionManifest::parse(
            r#"version = 1
[topics.command]
"camera_app::OldCommands" = 0x70
"#,
        )
        .unwrap();

        let error = check_paths_with_manifest([&schema], &manifest).unwrap_err();
        let message = error.to_string();

        assert!(
            message.contains("missing command topic assignment for `camera_app::CameraCommands`")
        );
        assert!(
            message.contains("missing telemetry topic assignment for `camera_app::CameraStatus`")
        );
        assert!(message.contains("unknown command topic assignment `camera_app::OldCommands`"));
    }

    #[test]
    fn reports_assignment_under_the_wrong_topic_kind() {
        let (_dir, schema) = write_schema("wrong-topic-kind", SCHEMA);
        let manifest = MissionManifest::parse(
            r#"version = 1
[topics.command]
"camera_app::CameraStatus" = 0x83

[topics.telemetry]
"camera_app::CameraCommands" = 0x82
"#,
        )
        .unwrap();

        let error = check_paths_with_manifest([&schema], &manifest).unwrap_err();
        let message = error.to_string();

        assert!(message.contains(
            "command topic `camera_app::CameraCommands` is assigned under `topics.telemetry`"
        ));
        assert!(message.contains(
            "telemetry topic `camera_app::CameraStatus` is assigned under `topics.command`"
        ));
    }

    #[test]
    fn rejects_schema_defined_mids() {
        let schema = r#"namespace camera_app

commands CameraCommands {
    @cc(1)
    command SetMode { mode: u8 }
}

@mid(0x0801)
telemetry LegacyStatus { mode: u8 }
"#;
        let (_dir, schema) = write_schema("legacy-packet", schema);
        let manifest = MissionManifest::parse(
            r#"version = 1
[topics.command]
"camera_app::CameraCommands" = 0x82
[topics.telemetry]
"camera_app::LegacyStatus" = 0x83
"#,
        )
        .unwrap();

        let error = check_paths_with_manifest([&schema], &manifest).unwrap_err();
        assert!(error.to_string().contains(
            "`@mid(...)` is not supported on `LegacyStatus`; assign its logical topic in the mission manifest"
        ));
    }

    #[test]
    fn rejects_colliding_generated_c_symbols() {
        let (dir, first) = write_schema(
            "symbol-collision",
            "namespace foo_bar\ntelemetry Baz { value: u8 }",
        );
        let second = dir.join("second.syn");
        fs::write(&second, "namespace foo\ntelemetry BarBaz { value: u8 }").unwrap();
        let manifest = MissionManifest::parse(
            r#"version = 1
[topics.telemetry]
"foo_bar::Baz" = 0x81
"foo::BarBaz" = 0x82
"#,
        )
        .unwrap();

        let error = generate_routing_header([&first, &second], &manifest).unwrap_err();

        assert!(error.to_string().contains(
            "telemetry topic `foo::BarBaz` and telemetry topic `foo_bar::Baz` both generate the C symbol `FOO_BAR_BAZ`"
        ));
    }

    fn write_schema(name: &str, source: &str) -> (PathBuf, PathBuf) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "synapse-mission-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let schema = dir.join("camera.syn");
        fs::write(&schema, source).unwrap();
        (dir, schema)
    }
}
