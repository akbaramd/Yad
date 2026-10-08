#[derive(Debug, Clone, Copy)]
pub struct BuiltinSchema {
    pub id: &'static str,
    pub yaml: &'static str,
}

pub const ADR_SCHEMA_YAML: &str = include_str!("../schemas/builtin/adr.schema.yaml");

pub const BUILTIN_SCHEMAS: &[BuiltinSchema] = &[
    BuiltinSchema { id: "adr", yaml: ADR_SCHEMA_YAML },
    BuiltinSchema { id: "architecture-description", yaml: include_str!("../schemas/builtin/architecture-description.schema.yaml") },
    BuiltinSchema { id: "requirements-specification", yaml: include_str!("../schemas/builtin/requirements-specification.schema.yaml") },
    BuiltinSchema { id: "context-map", yaml: include_str!("../schemas/builtin/context-map.schema.yaml") },
    BuiltinSchema { id: "bounded-context-canvas", yaml: include_str!("../schemas/builtin/bounded-context-canvas.schema.yaml") },
    BuiltinSchema { id: "ubiquitous-language", yaml: include_str!("../schemas/builtin/ubiquitous-language.schema.yaml") },
    BuiltinSchema { id: "domain-story", yaml: include_str!("../schemas/builtin/domain-story.schema.yaml") },
    BuiltinSchema { id: "event-storming", yaml: include_str!("../schemas/builtin/event-storming.schema.yaml") },
    BuiltinSchema { id: "aggregate-design-canvas", yaml: include_str!("../schemas/builtin/aggregate-design-canvas.schema.yaml") },
    BuiltinSchema { id: "domain-message-flow", yaml: include_str!("../schemas/builtin/domain-message-flow.schema.yaml") },
    BuiltinSchema { id: "api-contract", yaml: include_str!("../schemas/builtin/api-contract.schema.yaml") },
    BuiltinSchema { id: "event-api-contract", yaml: include_str!("../schemas/builtin/event-api-contract.schema.yaml") },
    BuiltinSchema { id: "data-schema", yaml: include_str!("../schemas/builtin/data-schema.schema.yaml") },
    BuiltinSchema { id: "process-model", yaml: include_str!("../schemas/builtin/process-model.schema.yaml") },
    BuiltinSchema { id: "incident", yaml: include_str!("../schemas/builtin/incident.schema.yaml") },
    BuiltinSchema { id: "runbook", yaml: include_str!("../schemas/builtin/runbook.schema.yaml") },
    BuiltinSchema { id: "core-domain-chart", yaml: include_str!("../schemas/builtin/core-domain-chart.schema.yaml") },
    BuiltinSchema { id: "threat-model", yaml: include_str!("../schemas/builtin/threat-model.schema.yaml") },
    BuiltinSchema { id: "data-repair", yaml: include_str!("../schemas/builtin/data-repair.schema.yaml") },
    BuiltinSchema { id: "migration", yaml: include_str!("../schemas/builtin/migration.schema.yaml") },
    BuiltinSchema { id: "engineering-rfc", yaml: include_str!("../schemas/builtin/engineering-rfc.schema.yaml") },
    BuiltinSchema { id: "quality-requirements", yaml: include_str!("../schemas/builtin/quality-requirements.schema.yaml") },
    BuiltinSchema { id: "postmortem", yaml: include_str!("../schemas/builtin/postmortem.schema.yaml") },
];

pub fn get(id: &str) -> Option<&'static BuiltinSchema> {
    BUILTIN_SCHEMAS
        .iter()
        .find(|schema| schema.id.eq_ignore_ascii_case(id))
}

pub fn count() -> usize {
    BUILTIN_SCHEMAS.len()
}
