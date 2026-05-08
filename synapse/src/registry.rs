use std::{collections::HashMap, path::PathBuf};

use crate::{
    Error, ImportGraph, ParsedUnit, RegistryFormat, format_mid, imported_constants_for_unit,
};

#[derive(Debug, Clone)]
struct RegistryPacket {
    source: PathBuf,
    namespace: Vec<String>,
    name: String,
    kind: synapse_codegen_cfs::CfsPacketKind,
    mid: u64,
    cc: Option<u64>,
}

pub(crate) fn render_registry(
    graph: &ImportGraph,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
    format: RegistryFormat,
) -> Result<String, Error> {
    let packets = collect_registry_packets(graph, units_by_path)?;
    Ok(match format {
        RegistryFormat::Json => render_registry_json(&packets),
        RegistryFormat::Csv => render_registry_csv(&packets),
    })
}

fn collect_registry_packets(
    graph: &ImportGraph,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
) -> Result<Vec<RegistryPacket>, Error> {
    let mut packets = Vec::new();
    for unit in &graph.units {
        let imported_constants = imported_constants_for_unit(unit, units_by_path)?;
        let unit_packets = synapse_codegen_cfs::collect_cfs_packets_with_constants(
            &unit.file,
            &imported_constants,
        )?;

        packets.extend(unit_packets.into_iter().map(|packet| RegistryPacket {
            source: unit.path.clone(),
            namespace: packet.namespace,
            name: packet.name,
            kind: packet.kind,
            mid: packet.mid,
            cc: packet.cc,
        }));
    }
    Ok(packets)
}

fn render_registry_json(packets: &[RegistryPacket]) -> String {
    let mut out = String::from("{\n  \"packets\": [\n");
    for (idx, packet) in packets.iter().enumerate() {
        if idx > 0 {
            out.push_str(",\n");
        }
        out.push_str("    {\n");
        out.push_str(&format!(
            "      \"namespace\": {},\n",
            json_string(&packet.namespace.join("::"))
        ));
        out.push_str(&format!("      \"name\": {},\n", json_string(&packet.name)));
        out.push_str(&format!(
            "      \"qualified_name\": {},\n",
            json_string(&registry_packet_name(packet))
        ));
        out.push_str(&format!(
            "      \"kind\": {},\n",
            json_string(registry_kind(packet.kind))
        ));
        out.push_str(&format!(
            "      \"source\": {},\n",
            json_string(&packet.source.display().to_string())
        ));
        out.push_str(&format!("      \"mid\": {},\n", packet.mid));
        out.push_str(&format!(
            "      \"mid_hex\": {},\n",
            json_string(&format_mid(packet.mid))
        ));
        match packet.cc {
            Some(cc) => out.push_str(&format!("      \"cc\": {cc}\n")),
            None => out.push_str("      \"cc\": null\n"),
        }
        out.push_str("    }");
    }
    out.push_str("\n  ]\n}\n");
    out
}

fn render_registry_csv(packets: &[RegistryPacket]) -> String {
    let mut out = String::from("namespace,name,qualified_name,kind,source,mid,mid_hex,cc\n");
    for packet in packets {
        let fields = [
            packet.namespace.join("::"),
            packet.name.clone(),
            registry_packet_name(packet),
            registry_kind(packet.kind).to_string(),
            packet.source.display().to_string(),
            packet.mid.to_string(),
            format_mid(packet.mid),
            packet.cc.map(|cc| cc.to_string()).unwrap_or_default(),
        ];
        out.push_str(
            &fields
                .iter()
                .map(|field| csv_field(field))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

fn registry_packet_name(packet: &RegistryPacket) -> String {
    if packet.namespace.is_empty() {
        packet.name.clone()
    } else {
        let mut segments = packet.namespace.clone();
        segments.push(packet.name.clone());
        segments.join("::")
    }
}

fn registry_kind(kind: synapse_codegen_cfs::CfsPacketKind) -> &'static str {
    match kind {
        synapse_codegen_cfs::CfsPacketKind::Command => "command",
        synapse_codegen_cfs::CfsPacketKind::Telemetry => "telemetry",
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04X}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn csv_field(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    format!("\"{escaped}\"")
}
