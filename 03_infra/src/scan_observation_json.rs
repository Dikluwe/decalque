//! Fronteira JSON estrita para `ScanObservation v1`.
//!
//! Os tipos Serde deste módulo são deliberadamente privados: serialização é
//! responsabilidade de L3 e não atravessa para o domínio puro.

use decalque_core::entities::{
    validate_scan_observation, Claim, Confidence, EvidenceBasis, FramedBbox, FramedPolygon,
    FramedPolyline, GeometryClaims, ObservationKind, ObservationUnit, PageFrame, PageMapping,
    PageMappingKind, ProducerIdentity, ProvenanceRecord, ProvenanceStage, RasterArtifact,
    RasterFrame, ScanObservation, ScanObservationDiagnostic, ScanObservationDiagnosticLevel,
    ScanSource, UnicodeRange, UnknownReason,
};
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_path_to_error::{Path as SerdePath, Segment};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const SCHEMA: &str = "decalque.scan-observation";
const SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanObservationLimits {
    pub max_input_bytes: usize,
    pub max_units: usize,
    pub max_provenance_records: usize,
    pub max_geometry_points: usize,
    pub max_total_text_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanObservationJsonError {
    Io {
        message: String,
    },
    InputTooLarge {
        limit: usize,
        actual: usize,
    },
    JsonSyntax {
        path: Option<String>,
        message: String,
    },
    UnsupportedSchema {
        schema: String,
        version: u64,
    },
    UnknownField {
        path: String,
        field: String,
    },
    ContractViolation {
        path: String,
        message: String,
    },
    BudgetExceeded {
        resource: String,
        limit: usize,
        actual: usize,
    },
}

impl fmt::Display for ScanObservationJsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { message } => write!(formatter, "I/O error: {message}"),
            Self::InputTooLarge { limit, actual } => {
                write!(formatter, "input has {actual} bytes, limit is {limit}")
            }
            Self::JsonSyntax { path, message } => match path {
                Some(path) => write!(formatter, "JSON syntax error at {path}: {message}"),
                None => write!(formatter, "JSON syntax error: {message}"),
            },
            Self::UnsupportedSchema { schema, version } => {
                write!(formatter, "unsupported schema `{schema}` version {version}")
            }
            Self::UnknownField { path, field } => {
                write!(formatter, "unknown field `{field}` at {path}")
            }
            Self::ContractViolation { path, message } => {
                write!(formatter, "contract violation at {path}: {message}")
            }
            Self::BudgetExceeded {
                resource,
                limit,
                actual,
            } => write!(
                formatter,
                "budget `{resource}` exceeded: actual {actual}, limit {limit}"
            ),
        }
    }
}

impl std::error::Error for ScanObservationJsonError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScanObservationDto {
    schema: String,
    schema_version: u64,
    source: ScanSourceDto,
    producer: ProducerIdentityDto,
    raster_frame: RasterFrameDto,
    page_mapping: ClaimDto<PageMappingDto>,
    provenance: Vec<ProvenanceRecordDto>,
    units: Vec<ObservationUnitDto>,
    diagnostics: Vec<ScanObservationDiagnosticDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScanSourceDto {
    page_index: u64,
    raster: RasterArtifactDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RasterArtifactDto {
    artifact_id: String,
    sha256: String,
    media_type: String,
    width_px: u64,
    height_px: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProducerIdentityDto {
    name: String,
    version: String,
    run_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RasterFrameDto {
    id: String,
    unit: String,
    origin: String,
    x_direction: String,
    y_direction: String,
    coordinate_basis: String,
    extent: [u64; 2],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PageFrameDto {
    id: String,
    unit: String,
    origin: String,
    x_direction: String,
    y_direction: String,
    extent: [f64; 2],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PageMappingDto {
    source_frame_id: String,
    target_frame: PageFrameDto,
    kind: PageMappingKindDto,
    matrix: [f64; 9],
    max_error_pt: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum PageMappingKindDto {
    Homography3x3,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvenanceRecordDto {
    id: String,
    stage: ProvenanceStageDto,
    tool_name: String,
    tool_version: String,
    model_identifier: String,
    method: String,
    parameters_sha256: String,
    input_artifact_ids: Vec<String>,
    parent_provenance_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ProvenanceStageDto {
    LayoutDetection,
    TextRecognition,
    Segmentation,
    CoordinateTransform,
    ManualAnnotation,
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationUnitDto {
    id: String,
    kind: ObservationKindDto,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    parent_id: Option<String>,
    reading_order: ClaimDto<u32>,
    text: ClaimDto<String>,
    span_in_parent: ClaimDto<UnicodeRangeDto>,
    geometry: GeometryClaimsDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ObservationKindDto {
    Region,
    Line,
    Word,
    Glyph,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnicodeRangeDto {
    start: u64,
    end: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeometryClaimsDto {
    bbox: ClaimDto<FramedBboxDto>,
    polygon: ClaimDto<FramedPointsDto>,
    baseline: ClaimDto<FramedPointsDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FramedBboxDto {
    frame_id: String,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FramedPointsDto {
    frame_id: String,
    points: Vec<[f64; 2]>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
enum ClaimDto<T> {
    Known {
        value: T,
        basis: EvidenceBasisDto,
        evidence: Vec<String>,
        confidence: ConfidenceDto,
    },
    Unknown {
        reason: UnknownReasonDto,
        evidence: Vec<String>,
        #[serde(default, deserialize_with = "deserialize_optional_non_null")]
        detail: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
enum ConfidenceDto {
    Known { value: f64, semantics: String },
    Unknown { reason: UnknownReasonDto },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum EvidenceBasisDto {
    Observed,
    Inferred,
    Derived,
    Asserted,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum UnknownReasonDto {
    NotObserved,
    Ambiguous,
    Unsupported,
    Invalid,
    BudgetExhausted,
    Redacted,
    BelowPolicyThreshold,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScanObservationDiagnosticDto {
    level: ScanObservationDiagnosticLevelDto,
    code: String,
    detail: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ScanObservationDiagnosticLevelDto {
    Information,
    Warning,
}

fn deserialize_optional_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

pub fn parse_scan_observation_json(
    bytes: &[u8],
    limits: &ScanObservationLimits,
) -> Result<ScanObservation, ScanObservationJsonError> {
    validate_limits(limits)?;
    if bytes.len() > limits.max_input_bytes {
        return Err(ScanObservationJsonError::InputTooLarge {
            limit: limits.max_input_bytes,
            actual: bytes.len(),
        });
    }

    let dto = deserialize_strict(bytes)?;
    if dto.schema != SCHEMA || dto.schema_version != SCHEMA_VERSION {
        return Err(ScanObservationJsonError::UnsupportedSchema {
            schema: dto.schema,
            version: dto.schema_version,
        });
    }

    enforce_budgets(&dto, limits)?;
    validate_dto_contract(&dto)?;
    into_domain(dto)
}

pub fn load_scan_observation_json(
    path: &Path,
    limits: &ScanObservationLimits,
) -> Result<ScanObservation, ScanObservationJsonError> {
    validate_limits(limits)?;
    let file = File::open(path).map_err(io_error)?;
    let metadata_len = file.metadata().map_err(io_error)?.len();
    if metadata_len > limits.max_input_bytes as u64 {
        return Err(ScanObservationJsonError::InputTooLarge {
            limit: limits.max_input_bytes,
            actual: usize::try_from(metadata_len).unwrap_or(usize::MAX),
        });
    }

    let read_limit = limits.max_input_bytes.saturating_add(1) as u64;
    let mut bytes = Vec::with_capacity(metadata_len as usize);
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    parse_scan_observation_json(&bytes, limits)
}

fn io_error(error: std::io::Error) -> ScanObservationJsonError {
    ScanObservationJsonError::Io {
        message: error.to_string(),
    }
}

fn validate_limits(limits: &ScanObservationLimits) -> Result<(), ScanObservationJsonError> {
    for (field, value) in [
        ("max_input_bytes", limits.max_input_bytes),
        ("max_units", limits.max_units),
        ("max_provenance_records", limits.max_provenance_records),
        ("max_geometry_points", limits.max_geometry_points),
        ("max_total_text_bytes", limits.max_total_text_bytes),
    ] {
        if value == 0 {
            return contract_error(format!("$limits.{field}"), "limit must be positive");
        }
    }
    Ok(())
}

fn deserialize_strict(bytes: &[u8]) -> Result<ScanObservationDto, ScanObservationJsonError> {
    reject_duplicate_keys(bytes)?;
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let dto = serde_path_to_error::deserialize(&mut deserializer).map_err(map_json_error)?;
    deserializer
        .end()
        .map_err(|error| ScanObservationJsonError::JsonSyntax {
            path: None,
            message: error.to_string(),
        })?;
    Ok(dto)
}

struct DuplicateCheckedValue;

impl<'de> Deserialize<'de> for DuplicateCheckedValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateCheckedVisitor)
    }
}

struct DuplicateCheckedVisitor;

impl<'de> Visitor<'de> for DuplicateCheckedVisitor {
    type Value = DuplicateCheckedValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedValue)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedValue)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedValue)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedValue)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedValue)
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedValue)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedValue)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedValue)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<DuplicateCheckedValue>()?.is_some() {}
        Ok(DuplicateCheckedValue)
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = HashSet::new();
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate object key `{key}`"
                )));
            }
            object.next_value::<DuplicateCheckedValue>()?;
        }
        Ok(DuplicateCheckedValue)
    }
}

fn reject_duplicate_keys(bytes: &[u8]) -> Result<(), ScanObservationJsonError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let result: Result<DuplicateCheckedValue, _> =
        serde_path_to_error::deserialize(&mut deserializer);
    if let Err(error) = result {
        let message = error.inner().to_string();
        if let Some(field) = field_from_serde_message(&message, "duplicate object key `") {
            let path = unknown_field_parent_path(stable_json_path(error.path()), &field);
            return Err(ScanObservationJsonError::JsonSyntax {
                path: Some(path),
                message,
            });
        }
    }
    Ok(())
}

fn map_json_error(
    error: serde_path_to_error::Error<serde_json::Error>,
) -> ScanObservationJsonError {
    let path = stable_json_path(error.path());
    let inner = error.inner();
    let message = inner.to_string();

    if let Some(field) = field_from_serde_message(&message, "unknown field `") {
        let path = unknown_field_parent_path(path, &field);
        return ScanObservationJsonError::UnknownField { path, field };
    }
    if message.starts_with("duplicate field `") {
        return ScanObservationJsonError::JsonSyntax {
            path: Some(path),
            message,
        };
    }
    if message.contains("number out of range") {
        return ScanObservationJsonError::ContractViolation { path, message };
    }

    match inner.classify() {
        serde_json::error::Category::Data => {
            ScanObservationJsonError::ContractViolation { path, message }
        }
        serde_json::error::Category::Io
        | serde_json::error::Category::Syntax
        | serde_json::error::Category::Eof => ScanObservationJsonError::JsonSyntax {
            path: Some(path),
            message,
        },
    }
}

fn unknown_field_parent_path(path: String, field: &str) -> String {
    let suffix = format!(".{field}");
    path.strip_suffix(&suffix)
        .map_or(path.clone(), str::to_string)
}

fn field_from_serde_message(message: &str, prefix: &str) -> Option<String> {
    message
        .strip_prefix(prefix)
        .and_then(|rest| rest.split('`').next())
        .map(str::to_string)
}

fn stable_json_path(path: &SerdePath) -> String {
    let mut rendered = String::from("$");
    for segment in path {
        match segment {
            Segment::Seq { index } => rendered.push_str(&format!("[{index}]")),
            Segment::Map { key } => {
                rendered.push('.');
                rendered.push_str(key);
            }
            Segment::Enum { .. } | Segment::Unknown => {}
        }
    }
    rendered
}

fn enforce_budgets(
    dto: &ScanObservationDto,
    limits: &ScanObservationLimits,
) -> Result<(), ScanObservationJsonError> {
    budget("units", limits.max_units, dto.units.len())?;
    budget(
        "provenance_records",
        limits.max_provenance_records,
        dto.provenance.len(),
    )?;

    let geometry_points = dto.units.iter().fold(0usize, |total, unit| {
        total
            .saturating_add(claim_points(&unit.geometry.polygon))
            .saturating_add(claim_points(&unit.geometry.baseline))
    });
    budget(
        "geometry_points",
        limits.max_geometry_points,
        geometry_points,
    )?;

    let text_bytes = dto
        .units
        .iter()
        .fold(claim_detail_bytes(&dto.page_mapping), |total, unit| {
            total
                .saturating_add(claim_detail_bytes(&unit.reading_order))
                .saturating_add(claim_text_bytes(&unit.text))
                .saturating_add(claim_detail_bytes(&unit.span_in_parent))
                .saturating_add(claim_detail_bytes(&unit.geometry.bbox))
                .saturating_add(claim_detail_bytes(&unit.geometry.polygon))
                .saturating_add(claim_detail_bytes(&unit.geometry.baseline))
        })
        .saturating_add(dto.diagnostics.iter().fold(0usize, |total, diagnostic| {
            total.saturating_add(diagnostic.detail.len())
        }));
    budget("text_bytes", limits.max_total_text_bytes, text_bytes)
}

fn budget(resource: &str, limit: usize, actual: usize) -> Result<(), ScanObservationJsonError> {
    if actual > limit {
        Err(ScanObservationJsonError::BudgetExceeded {
            resource: resource.to_string(),
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}

fn claim_points(claim: &ClaimDto<FramedPointsDto>) -> usize {
    match claim {
        ClaimDto::Known { value, .. } => value.points.len(),
        ClaimDto::Unknown { .. } => 0,
    }
}

fn claim_detail_bytes<T>(claim: &ClaimDto<T>) -> usize {
    match claim {
        ClaimDto::Known { .. } => 0,
        ClaimDto::Unknown { detail, .. } => detail.as_deref().map_or(0, str::len),
    }
}

fn claim_text_bytes(claim: &ClaimDto<String>) -> usize {
    match claim {
        ClaimDto::Known { value, .. } => value.len(),
        ClaimDto::Unknown { detail, .. } => detail.as_deref().map_or(0, str::len),
    }
}

fn contract_error<T>(
    path: impl Into<String>,
    message: impl Into<String>,
) -> Result<T, ScanObservationJsonError> {
    Err(ScanObservationJsonError::ContractViolation {
        path: path.into(),
        message: message.into(),
    })
}

fn validate_dto_contract(dto: &ScanObservationDto) -> Result<(), ScanObservationJsonError> {
    non_empty(
        "$.source.raster.artifact_id",
        &dto.source.raster.artifact_id,
    )?;
    valid_sha256("$.source.raster.sha256", &dto.source.raster.sha256)?;
    non_empty("$.source.raster.media_type", &dto.source.raster.media_type)?;
    if dto.source.raster.width_px == 0 {
        return contract_error("$.source.raster.width_px", "width must be positive");
    }
    if dto.source.raster.height_px == 0 {
        return contract_error("$.source.raster.height_px", "height must be positive");
    }

    non_empty("$.producer.name", &dto.producer.name)?;
    non_empty("$.producer.version", &dto.producer.version)?;
    non_empty("$.producer.run_id", &dto.producer.run_id)?;
    validate_raster_frame(dto)?;

    for (index, diagnostic) in dto.diagnostics.iter().enumerate() {
        non_empty(format!("$.diagnostics[{index}].code"), &diagnostic.code)?;
        non_empty(format!("$.diagnostics[{index}].detail"), &diagnostic.detail)?;
    }

    let provenance_ids = validate_provenance(dto)?;
    validate_claim(&dto.page_mapping, "$.page_mapping", &provenance_ids)?;
    if let ClaimDto::Known { value, .. } = &dto.page_mapping {
        validate_page_mapping(
            value,
            &dto.raster_frame.id,
            dto.source.raster.width_px,
            dto.source.raster.height_px,
        )?;
    }
    validate_units(dto, &provenance_ids)
}

fn validate_raster_frame(dto: &ScanObservationDto) -> Result<(), ScanObservationJsonError> {
    let frame = &dto.raster_frame;
    non_empty("$.raster_frame.id", &frame.id)?;
    exact_string("$.raster_frame.unit", &frame.unit, "px")?;
    exact_string("$.raster_frame.origin", &frame.origin, "top-left")?;
    exact_string("$.raster_frame.x_direction", &frame.x_direction, "right")?;
    exact_string("$.raster_frame.y_direction", &frame.y_direction, "down")?;
    exact_string(
        "$.raster_frame.coordinate_basis",
        &frame.coordinate_basis,
        "pixel-edges",
    )?;
    if frame.extent != [dto.source.raster.width_px, dto.source.raster.height_px] {
        return contract_error(
            "$.raster_frame.extent",
            "extent must equal source raster dimensions",
        );
    }
    Ok(())
}

fn validate_provenance(
    dto: &ScanObservationDto,
) -> Result<HashSet<&str>, ScanObservationJsonError> {
    let mut ids = HashSet::with_capacity(dto.provenance.len());
    for (index, record) in dto.provenance.iter().enumerate() {
        let base = format!("$.provenance[{index}]");
        non_empty(format!("{base}.id"), &record.id)?;
        if !ids.insert(record.id.as_str()) {
            return contract_error(format!("{base}.id"), "duplicate provenance id");
        }
        for (field, value) in [
            ("tool_name", &record.tool_name),
            ("tool_version", &record.tool_version),
            ("model_identifier", &record.model_identifier),
            ("method", &record.method),
        ] {
            non_empty(format!("{base}.{field}"), value)?;
        }
        valid_sha256(
            format!("{base}.parameters_sha256"),
            &record.parameters_sha256,
        )?;
        for (input_index, artifact_id) in record.input_artifact_ids.iter().enumerate() {
            if artifact_id != &dto.source.raster.artifact_id {
                return contract_error(
                    format!("{base}.input_artifact_ids[{input_index}]"),
                    "input artifact does not reference the source raster",
                );
            }
        }
        if record.input_artifact_ids.is_empty() && record.parent_provenance_ids.is_empty() {
            return contract_error(
                format!("{base}.parent_provenance_ids"),
                "record without a raster input must have a provenance parent",
            );
        }
        if record.model_identifier == "none"
            && !matches!(
                record.stage,
                ProvenanceStageDto::Segmentation
                    | ProvenanceStageDto::CoordinateTransform
                    | ProvenanceStageDto::ManualAnnotation
            )
        {
            return contract_error(
                format!("{base}.model_identifier"),
                "model_identifier `none` is only valid for deterministic or manual stages",
            );
        }
    }

    for (index, record) in dto.provenance.iter().enumerate() {
        for parent_id in &record.parent_provenance_ids {
            if !ids.contains(parent_id.as_str()) {
                return contract_error(
                    format!("$.provenance[{index}].parent_provenance_ids"),
                    format!("unknown provenance parent `{parent_id}`"),
                );
            }
        }
    }

    let parents: HashMap<&str, Vec<&str>> = dto
        .provenance
        .iter()
        .map(|record| {
            (
                record.id.as_str(),
                record
                    .parent_provenance_ids
                    .iter()
                    .map(String::as_str)
                    .collect(),
            )
        })
        .collect();
    if graph_has_cycle(&parents) {
        return contract_error("$.provenance", "provenance graph contains a cycle");
    }
    Ok(ids)
}

fn validate_units(
    dto: &ScanObservationDto,
    provenance_ids: &HashSet<&str>,
) -> Result<(), ScanObservationJsonError> {
    let mut unit_indices = HashMap::with_capacity(dto.units.len());
    for (index, unit) in dto.units.iter().enumerate() {
        non_empty(format!("$.units[{index}].id"), &unit.id)?;
        if unit_indices.insert(unit.id.as_str(), index).is_some() {
            return contract_error(format!("$.units[{index}].id"), "duplicate unit id");
        }
    }

    let mut parents = HashMap::with_capacity(dto.units.len());
    for (index, unit) in dto.units.iter().enumerate() {
        if let Some(parent_id) = unit.parent_id.as_deref() {
            if !unit_indices.contains_key(parent_id) {
                return contract_error(
                    format!("$.units[{index}].parent_id"),
                    format!("unknown parent unit `{parent_id}`"),
                );
            }
            parents.insert(unit.id.as_str(), vec![parent_id]);
        } else {
            parents.insert(unit.id.as_str(), Vec::new());
        }

        let base = format!("$.units[{index}");
        validate_claim(
            &unit.reading_order,
            &format!("{base}].reading_order"),
            provenance_ids,
        )?;
        validate_claim(&unit.text, &format!("{base}].text"), provenance_ids)?;
        validate_claim(
            &unit.span_in_parent,
            &format!("{base}].span_in_parent"),
            provenance_ids,
        )?;
        validate_claim(
            &unit.geometry.bbox,
            &format!("{base}].geometry.bbox"),
            provenance_ids,
        )?;
        validate_claim(
            &unit.geometry.polygon,
            &format!("{base}].geometry.polygon"),
            provenance_ids,
        )?;
        validate_claim(
            &unit.geometry.baseline,
            &format!("{base}].geometry.baseline"),
            provenance_ids,
        )?;

        if let ClaimDto::Known { value, .. } = &unit.text {
            non_empty(format!("{base}].text.value"), value)?;
        }
        validate_unit_geometry(dto, index, unit)?;
    }

    if graph_has_cycle(&parents) {
        return contract_error("$.units", "unit parent graph contains a cycle");
    }
    validate_reading_order(dto)?;
    validate_spans_and_child_geometry(dto, &unit_indices)
}

fn validate_claim<T>(
    claim: &ClaimDto<T>,
    path: &str,
    provenance_ids: &HashSet<&str>,
) -> Result<(), ScanObservationJsonError> {
    let evidence = match claim {
        ClaimDto::Known {
            evidence,
            confidence,
            ..
        } => {
            if evidence.is_empty() {
                return contract_error(format!("{path}.evidence"), "known claim needs evidence");
            }
            validate_confidence(confidence, &format!("{path}.confidence"))?;
            evidence
        }
        ClaimDto::Unknown { evidence, .. } => evidence,
    };
    for evidence_id in evidence {
        if !provenance_ids.contains(evidence_id.as_str()) {
            return contract_error(
                format!("{path}.evidence"),
                format!("unknown provenance reference `{evidence_id}`"),
            );
        }
    }
    Ok(())
}

fn validate_confidence(
    confidence: &ConfidenceDto,
    path: &str,
) -> Result<(), ScanObservationJsonError> {
    if let ConfidenceDto::Known { value, semantics } = confidence {
        finite_inclusive_unit_interval(format!("{path}.value"), *value)?;
        non_empty(format!("{path}.semantics"), semantics)?;
    }
    Ok(())
}

fn validate_page_mapping(
    mapping: &PageMappingDto,
    raster_frame_id: &str,
    raster_width: u64,
    raster_height: u64,
) -> Result<(), ScanObservationJsonError> {
    if mapping.source_frame_id != raster_frame_id {
        return contract_error(
            "$.page_mapping.value.source_frame_id",
            "source frame does not reference raster_frame",
        );
    }
    non_empty(
        "$.page_mapping.value.target_frame.id",
        &mapping.target_frame.id,
    )?;
    exact_string(
        "$.page_mapping.value.target_frame.unit",
        &mapping.target_frame.unit,
        "pt",
    )?;
    exact_string(
        "$.page_mapping.value.target_frame.origin",
        &mapping.target_frame.origin,
        "top-left",
    )?;
    exact_string(
        "$.page_mapping.value.target_frame.x_direction",
        &mapping.target_frame.x_direction,
        "right",
    )?;
    exact_string(
        "$.page_mapping.value.target_frame.y_direction",
        &mapping.target_frame.y_direction,
        "down",
    )?;
    for (index, extent) in mapping.target_frame.extent.iter().copied().enumerate() {
        if !extent.is_finite() || extent <= 0.0 {
            return contract_error(
                format!("$.page_mapping.value.target_frame.extent[{index}]"),
                "physical extent must be finite and positive",
            );
        }
    }
    for (index, coefficient) in mapping.matrix.iter().copied().enumerate() {
        if !coefficient.is_finite() {
            return contract_error(
                format!("$.page_mapping.value.matrix[{index}]"),
                "homography coefficient must be finite",
            );
        }
    }
    if !mapping.max_error_pt.is_finite() || mapping.max_error_pt < 0.0 {
        return contract_error(
            "$.page_mapping.value.max_error_pt",
            "mapping error must be finite and non-negative",
        );
    }

    let m = &mapping.matrix;
    let determinant = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
        + m[2] * (m[3] * m[7] - m[4] * m[6]);
    if !determinant.is_finite() || determinant == 0.0 {
        return contract_error("$.page_mapping.value.matrix", "homography is singular");
    }
    let _ = (raster_width, raster_height);
    Ok(())
}

fn validate_unit_geometry(
    dto: &ScanObservationDto,
    index: usize,
    unit: &ObservationUnitDto,
) -> Result<(), ScanObservationJsonError> {
    let base = format!("$.units[{index}].geometry");
    let bbox = match &unit.geometry.bbox {
        ClaimDto::Known { value, .. } => {
            validate_bbox(dto, value, &format!("{base}.bbox.value"))?;
            Some(value)
        }
        ClaimDto::Unknown { .. } => None,
    };
    let polygon = match &unit.geometry.polygon {
        ClaimDto::Known { value, .. } => {
            validate_points(dto, value, 3, &format!("{base}.polygon.value"))?;
            Some(value)
        }
        ClaimDto::Unknown { .. } => None,
    };
    if let ClaimDto::Known { value, .. } = &unit.geometry.baseline {
        validate_points(dto, value, 2, &format!("{base}.baseline.value"))?;
    }
    if let (Some(bbox), Some(polygon)) = (bbox, polygon) {
        if bbox.frame_id != polygon.frame_id {
            return contract_error(
                format!("{base}.polygon.value.frame_id"),
                "bbox and polygon must use the same frame",
            );
        }
        let min_x = polygon
            .points
            .iter()
            .map(|point| point[0])
            .fold(f64::INFINITY, f64::min);
        let max_x = polygon
            .points
            .iter()
            .map(|point| point[0])
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = polygon
            .points
            .iter()
            .map(|point| point[1])
            .fold(f64::INFINITY, f64::min);
        let max_y = polygon
            .points
            .iter()
            .map(|point| point[1])
            .fold(f64::NEG_INFINITY, f64::max);
        let epsilon = 1.0e-9;
        if (bbox.x0 - min_x).abs() > epsilon
            || (bbox.y0 - min_y).abs() > epsilon
            || (bbox.x1 - max_x).abs() > epsilon
            || (bbox.y1 - max_y).abs() > epsilon
        {
            return contract_error(
                format!("{base}.bbox.value"),
                "bbox must be the polygon envelope",
            );
        }
    }
    Ok(())
}

fn validate_bbox(
    dto: &ScanObservationDto,
    bbox: &FramedBboxDto,
    path: &str,
) -> Result<(), ScanObservationJsonError> {
    if bbox.frame_id != dto.raster_frame.id {
        return contract_error(format!("{path}.frame_id"), "unknown raster frame");
    }
    for (field, value) in [
        ("x0", bbox.x0),
        ("y0", bbox.y0),
        ("x1", bbox.x1),
        ("y1", bbox.y1),
    ] {
        if !value.is_finite() {
            return contract_error(format!("{path}.{field}"), "coordinate must be finite");
        }
    }
    if !(0.0 <= bbox.x0 && bbox.x0 < bbox.x1 && bbox.x1 <= dto.source.raster.width_px as f64) {
        return contract_error(format!("{path}.x0"), "bbox x coordinates are invalid");
    }
    if !(0.0 <= bbox.y0 && bbox.y0 < bbox.y1 && bbox.y1 <= dto.source.raster.height_px as f64) {
        return contract_error(format!("{path}.y0"), "bbox y coordinates are invalid");
    }
    Ok(())
}

fn validate_points(
    dto: &ScanObservationDto,
    geometry: &FramedPointsDto,
    minimum: usize,
    path: &str,
) -> Result<(), ScanObservationJsonError> {
    if geometry.frame_id != dto.raster_frame.id {
        return contract_error(format!("{path}.frame_id"), "unknown raster frame");
    }
    if geometry.points.len() < minimum {
        return contract_error(
            format!("{path}.points"),
            format!("geometry needs at least {minimum} points"),
        );
    }
    for (index, [x, y]) in geometry.points.iter().copied().enumerate() {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x > dto.source.raster.width_px as f64
            || y > dto.source.raster.height_px as f64
        {
            return contract_error(
                format!("{path}.points[{index}]"),
                "point must be finite and inside the raster frame",
            );
        }
    }
    Ok(())
}

fn validate_reading_order(dto: &ScanObservationDto) -> Result<(), ScanObservationJsonError> {
    let mut seen = HashSet::new();
    for (index, unit) in dto.units.iter().enumerate() {
        if let ClaimDto::Known { value, .. } = &unit.reading_order {
            let key = (unit.parent_id.as_deref(), *value);
            if !seen.insert(key) {
                return contract_error(
                    format!("$.units[{index}].reading_order.value"),
                    "known reading order must be unique among siblings",
                );
            }
        }
    }
    Ok(())
}

fn validate_spans_and_child_geometry(
    dto: &ScanObservationDto,
    unit_indices: &HashMap<&str, usize>,
) -> Result<(), ScanObservationJsonError> {
    for (index, unit) in dto.units.iter().enumerate() {
        if let ClaimDto::Known { value: span, .. } = &unit.span_in_parent {
            let Some(parent_id) = unit.parent_id.as_deref() else {
                return contract_error(
                    format!("$.units[{index}].span_in_parent"),
                    "known span requires a parent unit",
                );
            };
            if span.start >= span.end {
                return contract_error(
                    format!("$.units[{index}].span_in_parent.value"),
                    "span must be non-empty and semi-open",
                );
            }
            let parent = &dto.units[unit_indices[parent_id]];
            if let (
                ClaimDto::Known {
                    value: child_text, ..
                },
                ClaimDto::Known {
                    value: parent_text, ..
                },
            ) = (&unit.text, &parent.text)
            {
                let start = usize::try_from(span.start).map_err(|_| {
                    ScanObservationJsonError::ContractViolation {
                        path: format!("$.units[{index}].span_in_parent.value.start"),
                        message: "span start is not representable".to_string(),
                    }
                })?;
                let end = usize::try_from(span.end).map_err(|_| {
                    ScanObservationJsonError::ContractViolation {
                        path: format!("$.units[{index}].span_in_parent.value.end"),
                        message: "span end is not representable".to_string(),
                    }
                })?;
                let scalars: Vec<char> = parent_text.chars().collect();
                if end > scalars.len() {
                    return contract_error(
                        format!("$.units[{index}].span_in_parent.value.end"),
                        "span exceeds parent text",
                    );
                }
                let substring: String = scalars[start..end].iter().collect();
                if substring != *child_text {
                    return contract_error(
                        format!("$.units[{index}].span_in_parent.value"),
                        "span does not select the child text",
                    );
                }
            }
        }

        let Some(parent_id) = unit.parent_id.as_deref() else {
            continue;
        };
        let parent = &dto.units[unit_indices[parent_id]];
        if copied_bbox(&unit.geometry.bbox, &parent.geometry.bbox) {
            return contract_error(
                format!("$.units[{index}].geometry.bbox"),
                "child bbox duplicates its parent without independent evidence",
            );
        }
    }
    Ok(())
}

fn copied_bbox(child: &ClaimDto<FramedBboxDto>, parent: &ClaimDto<FramedBboxDto>) -> bool {
    match (child, parent) {
        (
            ClaimDto::Known {
                value: child_value,
                evidence: child_evidence,
                ..
            },
            ClaimDto::Known {
                value: parent_value,
                evidence: parent_evidence,
                ..
            },
        ) => {
            child_evidence == parent_evidence
                && child_value.frame_id == parent_value.frame_id
                && child_value.x0 == parent_value.x0
                && child_value.y0 == parent_value.y0
                && child_value.x1 == parent_value.x1
                && child_value.y1 == parent_value.y1
        }
        _ => false,
    }
}

fn graph_has_cycle(graph: &HashMap<&str, Vec<&str>>) -> bool {
    fn visit<'a>(
        node: &'a str,
        graph: &HashMap<&'a str, Vec<&'a str>>,
        active: &mut HashSet<&'a str>,
        finished: &mut HashSet<&'a str>,
    ) -> bool {
        if finished.contains(node) {
            return false;
        }
        if !active.insert(node) {
            return true;
        }
        if graph
            .get(node)
            .into_iter()
            .flatten()
            .any(|parent| visit(parent, graph, active, finished))
        {
            return true;
        }
        active.remove(node);
        finished.insert(node);
        false
    }

    let mut active = HashSet::new();
    let mut finished = HashSet::new();
    graph
        .keys()
        .copied()
        .any(|node| visit(node, graph, &mut active, &mut finished))
}

fn non_empty(path: impl Into<String>, value: &str) -> Result<(), ScanObservationJsonError> {
    if value.trim().is_empty() {
        contract_error(path, "string must not be empty")
    } else {
        Ok(())
    }
}

fn exact_string(
    path: impl Into<String>,
    actual: &str,
    expected: &str,
) -> Result<(), ScanObservationJsonError> {
    if actual == expected {
        Ok(())
    } else {
        contract_error(path, format!("expected `{expected}`, found `{actual}`"))
    }
}

fn valid_sha256(path: impl Into<String>, value: &str) -> Result<(), ScanObservationJsonError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        contract_error(path, "SHA-256 must be 64 lowercase hexadecimal characters")
    }
}

fn finite_inclusive_unit_interval(
    path: impl Into<String>,
    value: f64,
) -> Result<(), ScanObservationJsonError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        contract_error(path, "confidence must be finite and in [0, 1]")
    }
}

fn into_domain(dto: ScanObservationDto) -> Result<ScanObservation, ScanObservationJsonError> {
    let page_index = checked_u32("$.source.page_index", dto.source.page_index)?;
    let width_px = checked_u32("$.source.raster.width_px", dto.source.raster.width_px)?;
    let height_px = checked_u32("$.source.raster.height_px", dto.source.raster.height_px)?;
    let extent_width = checked_u32("$.raster_frame.extent[0]", dto.raster_frame.extent[0])?;
    let extent_height = checked_u32("$.raster_frame.extent[1]", dto.raster_frame.extent[1])?;

    let page_mapping = into_claim(dto.page_mapping, "$.page_mapping", |mapping| {
        Ok(PageMapping {
            source_frame_id: mapping.source_frame_id,
            target_frame: PageFrame {
                id: mapping.target_frame.id,
                extent: (
                    mapping.target_frame.extent[0],
                    mapping.target_frame.extent[1],
                ),
            },
            kind: match mapping.kind {
                PageMappingKindDto::Homography3x3 => PageMappingKind::Homography3x3,
            },
            matrix: mapping.matrix,
            max_error_pt: mapping.max_error_pt,
        })
    })?;

    let provenance = dto
        .provenance
        .into_iter()
        .map(|record| ProvenanceRecord {
            id: record.id,
            stage: match record.stage {
                ProvenanceStageDto::LayoutDetection => ProvenanceStage::LayoutDetection,
                ProvenanceStageDto::TextRecognition => ProvenanceStage::TextRecognition,
                ProvenanceStageDto::Segmentation => ProvenanceStage::Segmentation,
                ProvenanceStageDto::CoordinateTransform => ProvenanceStage::CoordinateTransform,
                ProvenanceStageDto::ManualAnnotation => ProvenanceStage::ManualAnnotation,
                ProvenanceStageDto::Other => ProvenanceStage::Other,
            },
            tool_name: record.tool_name,
            tool_version: record.tool_version,
            model_identifier: record.model_identifier,
            method: record.method,
            parameters_sha256: record.parameters_sha256,
            input_artifact_ids: record.input_artifact_ids,
            parent_provenance_ids: record.parent_provenance_ids,
        })
        .collect();

    let units = dto
        .units
        .into_iter()
        .enumerate()
        .map(|(index, unit)| into_unit(unit, index))
        .collect::<Result<Vec<_>, _>>()?;

    let diagnostics = dto
        .diagnostics
        .into_iter()
        .map(|diagnostic| ScanObservationDiagnostic {
            level: match diagnostic.level {
                ScanObservationDiagnosticLevelDto::Information => {
                    ScanObservationDiagnosticLevel::Information
                }
                ScanObservationDiagnosticLevelDto::Warning => {
                    ScanObservationDiagnosticLevel::Warning
                }
            },
            code: diagnostic.code,
            detail: diagnostic.detail,
        })
        .collect();

    let observation = ScanObservation {
        source: ScanSource {
            page_index,
            raster: RasterArtifact {
                artifact_id: dto.source.raster.artifact_id,
                sha256: dto.source.raster.sha256,
                media_type: dto.source.raster.media_type,
                width_px,
                height_px,
            },
        },
        producer: ProducerIdentity {
            name: dto.producer.name,
            version: dto.producer.version,
            run_id: dto.producer.run_id,
        },
        raster_frame: RasterFrame {
            id: dto.raster_frame.id,
            extent: (extent_width, extent_height),
        },
        page_mapping,
        provenance,
        units,
        diagnostics,
    };

    validate_scan_observation(&observation).map_err(|error| {
        ScanObservationJsonError::ContractViolation {
            path: format!("$.{}", error.path),
            message: error.message,
        }
    })?;
    Ok(observation)
}

fn into_unit(
    unit: ObservationUnitDto,
    index: usize,
) -> Result<ObservationUnit, ScanObservationJsonError> {
    let base = format!("$.units[{index}]");
    Ok(ObservationUnit {
        id: unit.id,
        kind: match unit.kind {
            ObservationKindDto::Region => ObservationKind::Region,
            ObservationKindDto::Line => ObservationKind::Line,
            ObservationKindDto::Word => ObservationKind::Word,
            ObservationKindDto::Glyph => ObservationKind::Glyph,
        },
        parent_id: unit.parent_id,
        reading_order: into_claim(unit.reading_order, &format!("{base}.reading_order"), Ok)?,
        text: into_claim(unit.text, &format!("{base}.text"), Ok)?,
        span_in_parent: into_claim(
            unit.span_in_parent,
            &format!("{base}.span_in_parent"),
            |span| {
                Ok(UnicodeRange {
                    start: checked_usize(
                        &format!("{base}.span_in_parent.value.start"),
                        span.start,
                    )?,
                    end: checked_usize(&format!("{base}.span_in_parent.value.end"), span.end)?,
                })
            },
        )?,
        geometry: GeometryClaims {
            bbox: into_claim(
                unit.geometry.bbox,
                &format!("{base}.geometry.bbox"),
                |bbox| {
                    Ok(FramedBbox {
                        frame_id: bbox.frame_id,
                        x0: bbox.x0,
                        y0: bbox.y0,
                        x1: bbox.x1,
                        y1: bbox.y1,
                    })
                },
            )?,
            polygon: into_claim(
                unit.geometry.polygon,
                &format!("{base}.geometry.polygon"),
                |polygon| {
                    Ok(FramedPolygon {
                        frame_id: polygon.frame_id,
                        points: polygon.points.into_iter().map(|[x, y]| (x, y)).collect(),
                    })
                },
            )?,
            baseline: into_claim(
                unit.geometry.baseline,
                &format!("{base}.geometry.baseline"),
                |baseline| {
                    Ok(FramedPolyline {
                        frame_id: baseline.frame_id,
                        points: baseline.points.into_iter().map(|[x, y]| (x, y)).collect(),
                    })
                },
            )?,
        },
    })
}

fn into_claim<T, U>(
    claim: ClaimDto<T>,
    _path: &str,
    convert: impl FnOnce(T) -> Result<U, ScanObservationJsonError>,
) -> Result<Claim<U>, ScanObservationJsonError> {
    match claim {
        ClaimDto::Known {
            value,
            basis,
            evidence,
            confidence,
        } => Ok(Claim::Known {
            value: convert(value)?,
            basis: match basis {
                EvidenceBasisDto::Observed => EvidenceBasis::Observed,
                EvidenceBasisDto::Inferred => EvidenceBasis::Inferred,
                EvidenceBasisDto::Derived => EvidenceBasis::Derived,
                EvidenceBasisDto::Asserted => EvidenceBasis::Asserted,
            },
            evidence,
            confidence: match confidence {
                ConfidenceDto::Known { value, semantics } => Confidence::Known { value, semantics },
                ConfidenceDto::Unknown { reason } => Confidence::Unknown {
                    reason: into_unknown_reason(reason),
                },
            },
        }),
        ClaimDto::Unknown {
            reason,
            evidence,
            detail,
        } => Ok(Claim::Unknown {
            reason: into_unknown_reason(reason),
            evidence,
            detail,
        }),
    }
}

fn into_unknown_reason(reason: UnknownReasonDto) -> UnknownReason {
    match reason {
        UnknownReasonDto::NotObserved => UnknownReason::NotObserved,
        UnknownReasonDto::Ambiguous => UnknownReason::Ambiguous,
        UnknownReasonDto::Unsupported => UnknownReason::Unsupported,
        UnknownReasonDto::Invalid => UnknownReason::Invalid,
        UnknownReasonDto::BudgetExhausted => UnknownReason::BudgetExhausted,
        UnknownReasonDto::Redacted => UnknownReason::Redacted,
        UnknownReasonDto::BelowPolicyThreshold => UnknownReason::BelowPolicyThreshold,
    }
}

fn checked_u32(path: &str, value: u64) -> Result<u32, ScanObservationJsonError> {
    u32::try_from(value).map_err(|_| ScanObservationJsonError::ContractViolation {
        path: path.to_string(),
        message: "integer is not representable by the domain model".to_string(),
    })
}

fn checked_usize(path: &str, value: u64) -> Result<usize, ScanObservationJsonError> {
    usize::try_from(value).map_err(|_| ScanObservationJsonError::ContractViolation {
        path: path.to_string(),
        message: "integer is not representable by the domain model".to_string(),
    })
}
