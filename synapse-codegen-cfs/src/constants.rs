use std::collections::HashMap;

use synapse_parser::ast::{ConstDecl, Item, Literal, SynFile};

use crate::{types::ResolvedConstants, util::file_namespace};

pub(crate) struct ConstContext<'a> {
    pub(crate) local_defs: HashMap<Vec<String>, &'a ConstDecl>,
    imported_values: &'a ResolvedConstants,
}

pub(crate) fn const_context<'a>(
    file: &'a SynFile,
    imported_values: &'a ResolvedConstants,
) -> ConstContext<'a> {
    let namespace = file_namespace(file);
    let mut local_defs = HashMap::new();
    for item in &file.items {
        if let Item::Const(c) = item {
            local_defs.insert(vec![c.name.clone()], c);
            if !namespace.is_empty() {
                let mut qualified = namespace.clone();
                qualified.push(c.name.clone());
                local_defs.insert(qualified, c);
            }
        }
    }
    ConstContext {
        local_defs,
        imported_values,
    }
}

impl ConstContext<'_> {
    pub(crate) fn resolved_local_constants(&self) -> ResolvedConstants {
        self.local_defs
            .keys()
            .filter_map(|segments| {
                resolve_ident_to_u64(segments, self).map(|value| (segments.clone(), value))
            })
            .collect()
    }

    pub(crate) fn is_local_bare_ident(&self, segments: &[String]) -> bool {
        segments.len() == 1 && self.local_defs.contains_key(segments)
    }
}

pub(crate) fn resolve_literal_to_u64(lit: &Literal, constants: &ConstContext<'_>) -> Option<u64> {
    resolve_literal_to_u64_inner(lit, constants, &mut Vec::new())
}

fn resolve_literal_to_u64_inner(
    lit: &Literal,
    constants: &ConstContext<'_>,
    seen: &mut Vec<Vec<String>>,
) -> Option<u64> {
    match lit {
        Literal::Ident(segments) => resolve_ident_to_u64_inner(segments, constants, seen),
        other => literal_to_u64(other),
    }
}

pub(crate) fn resolve_ident_to_u64(
    segments: &[String],
    constants: &ConstContext<'_>,
) -> Option<u64> {
    resolve_ident_to_u64_inner(segments, constants, &mut Vec::new())
}

fn resolve_ident_to_u64_inner(
    segments: &[String],
    constants: &ConstContext<'_>,
    seen: &mut Vec<Vec<String>>,
) -> Option<u64> {
    if seen.iter().any(|s| s == segments) {
        return None;
    }
    if let Some(c) = constants.local_defs.get(segments) {
        seen.push(segments.to_vec());
        let resolved = resolve_literal_to_u64_inner(&c.value, constants, seen);
        seen.pop();
        return resolved;
    }
    constants.imported_values.get(segments).copied()
}

fn literal_to_u64(lit: &Literal) -> Option<u64> {
    match lit {
        Literal::Hex(n) => Some(*n),
        Literal::Int(n) if *n >= 0 => Some(*n as u64),
        _ => None,
    }
}
