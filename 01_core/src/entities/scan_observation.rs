//! Observação externa de scan no domínio puro: nenhuma serialização, I/O, OCR
//! ou construção de `GlyphInstance`.

use std::collections::{HashMap, HashSet};
use std::fmt;

pub type ProvenanceId = String;

#[derive(Debug, Clone, PartialEq)]
pub struct ScanObservation {
    pub source: ScanSource,
    pub producer: ProducerIdentity,
    pub raster_frame: RasterFrame,
    pub page_mapping: Claim<PageMapping>,
    pub provenance: Vec<ProvenanceRecord>,
    pub units: Vec<ObservationUnit>,
    pub diagnostics: Vec<ScanObservationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSource {
    pub page_index: u32,
    pub raster: RasterArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterArtifact {
    pub artifact_id: String,
    pub sha256: String,
    pub media_type: String,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerIdentity {
    pub name: String,
    pub version: String,
    pub run_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterFrame {
    pub id: String,
    pub extent: (u32, u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Claim<T> {
    Known {
        value: T,
        basis: EvidenceBasis,
        evidence: Vec<ProvenanceId>,
        confidence: Confidence,
    },
    Unknown {
        reason: UnknownReason,
        evidence: Vec<ProvenanceId>,
        detail: Option<String>,
    },
}

impl<T> Claim<T> {
    pub fn known_value(&self) -> Option<&T> {
        match self {
            Self::Known { value, .. } => Some(value),
            Self::Unknown { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Confidence {
    Known { value: f64, semantics: String },
    Unknown { reason: UnknownReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceBasis {
    Observed,
    Inferred,
    Derived,
    Asserted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    NotObserved,
    Ambiguous,
    Unsupported,
    Invalid,
    BudgetExhausted,
    Redacted,
    BelowPolicyThreshold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservationKind {
    Region,
    Line,
    Word,
    Glyph,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObservationUnit {
    pub id: String,
    pub kind: ObservationKind,
    pub parent_id: Option<String>,
    pub reading_order: Claim<u32>,
    pub text: Claim<String>,
    pub span_in_parent: Claim<UnicodeRange>,
    pub geometry: GeometryClaims,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnicodeRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryClaims {
    pub bbox: Claim<FramedBbox>,
    pub polygon: Claim<FramedPolygon>,
    pub baseline: Claim<FramedPolyline>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FramedBbox {
    pub frame_id: String,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FramedPolygon {
    pub frame_id: String,
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FramedPolyline {
    pub frame_id: String,
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageMapping {
    pub source_frame_id: String,
    pub target_frame: PageFrame,
    pub kind: PageMappingKind,
    pub matrix: [f64; 9],
    pub max_error_pt: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageFrame {
    pub id: String,
    pub extent: (f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageMappingKind {
    Homography3x3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceStage {
    LayoutDetection,
    TextRecognition,
    Segmentation,
    CoordinateTransform,
    ManualAnnotation,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceRecord {
    pub id: ProvenanceId,
    pub stage: ProvenanceStage,
    pub tool_name: String,
    pub tool_version: String,
    pub model_identifier: String,
    pub method: String,
    pub parameters_sha256: String,
    pub input_artifact_ids: Vec<String>,
    pub parent_provenance_ids: Vec<ProvenanceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanObservationDiagnosticLevel {
    Information,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanObservationDiagnostic {
    pub level: ScanObservationDiagnosticLevel,
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanObservationValidationError {
    pub path: String,
    pub message: String,
}

impl ScanObservationValidationError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ScanObservationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for ScanObservationValidationError {}

type ValidationResult = Result<(), ScanObservationValidationError>;

pub fn validate_scan_observation(observation: &ScanObservation) -> ValidationResult {
    validate_envelope(observation)?;

    let provenance_by_id = validate_provenance(observation)?;
    validate_claim(
        &observation.page_mapping,
        "page_mapping",
        &provenance_by_id,
        |mapping| validate_page_mapping(mapping, &observation.raster_frame),
    )?;
    validate_units(observation, &provenance_by_id)?;

    if let Claim::Known { value: mapping, .. } = &observation.page_mapping {
        validate_mapping_horizon(mapping, &observation.units)?;
    }

    Ok(())
}

fn validate_envelope(observation: &ScanObservation) -> ValidationResult {
    require_nonempty(
        "source.raster.artifact_id",
        &observation.source.raster.artifact_id,
    )?;
    require_sha256("source.raster.sha256", &observation.source.raster.sha256)?;
    require_nonempty(
        "source.raster.media_type",
        &observation.source.raster.media_type,
    )?;
    if observation.source.raster.width_px == 0 || observation.source.raster.height_px == 0 {
        return invalid("source.raster", "raster dimensions must be positive");
    }

    require_nonempty("producer.name", &observation.producer.name)?;
    require_nonempty("producer.version", &observation.producer.version)?;
    require_nonempty("producer.run_id", &observation.producer.run_id)?;
    require_nonempty("raster_frame.id", &observation.raster_frame.id)?;
    if observation.raster_frame.extent
        != (
            observation.source.raster.width_px,
            observation.source.raster.height_px,
        )
    {
        return invalid(
            "raster_frame.extent",
            "extent must equal the source raster dimensions",
        );
    }
    Ok(())
}

fn validate_provenance(
    observation: &ScanObservation,
) -> Result<HashMap<&str, usize>, ScanObservationValidationError> {
    let mut by_id = HashMap::new();
    for (index, record) in observation.provenance.iter().enumerate() {
        let path = format!("provenance[{index}]");
        require_nonempty(format!("{path}.id"), &record.id)?;
        if by_id.insert(record.id.as_str(), index).is_some() {
            return invalid(format!("{path}.id"), "duplicate provenance id");
        }
        require_nonempty(format!("{path}.tool_name"), &record.tool_name)?;
        require_nonempty(format!("{path}.tool_version"), &record.tool_version)?;
        require_nonempty(format!("{path}.model_identifier"), &record.model_identifier)?;
        require_nonempty(format!("{path}.method"), &record.method)?;
        require_sha256(
            format!("{path}.parameters_sha256"),
            &record.parameters_sha256,
        )?;
        if record.model_identifier == "none"
            && matches!(
                record.stage,
                ProvenanceStage::LayoutDetection | ProvenanceStage::TextRecognition
            )
        {
            return invalid(
                format!("{path}.model_identifier"),
                "model-based stages may not declare model_identifier as none",
            );
        }
        for artifact_id in &record.input_artifact_ids {
            if artifact_id != &observation.source.raster.artifact_id {
                return invalid(
                    format!("{path}.input_artifact_ids"),
                    "input artifact does not reference the observed raster",
                );
            }
        }
    }

    for (index, record) in observation.provenance.iter().enumerate() {
        let path = format!("provenance[{index}]");
        for parent in &record.parent_provenance_ids {
            if !by_id.contains_key(parent.as_str()) {
                return invalid(
                    format!("{path}.parent_provenance_ids"),
                    "unknown parent provenance id",
                );
            }
        }
        if record.input_artifact_ids.is_empty() && record.parent_provenance_ids.is_empty() {
            return invalid(path, "a provenance root must reference the observed raster");
        }
    }

    let mut state = vec![0_u8; observation.provenance.len()];
    for index in 0..observation.provenance.len() {
        visit_provenance(index, &observation.provenance, &by_id, &mut state)?;
    }

    Ok(by_id)
}

fn visit_provenance(
    index: usize,
    records: &[ProvenanceRecord],
    by_id: &HashMap<&str, usize>,
    state: &mut [u8],
) -> ValidationResult {
    match state[index] {
        1 => return invalid("provenance", "cycle in provenance graph"),
        2 => return Ok(()),
        _ => {}
    }
    state[index] = 1;
    for parent in &records[index].parent_provenance_ids {
        visit_provenance(by_id[parent.as_str()], records, by_id, state)?;
    }
    state[index] = 2;
    Ok(())
}

fn validate_units(
    observation: &ScanObservation,
    provenance_by_id: &HashMap<&str, usize>,
) -> ValidationResult {
    let mut unit_by_id = HashMap::new();
    for (index, unit) in observation.units.iter().enumerate() {
        require_nonempty(format!("units[{index}].id"), &unit.id)?;
        if unit_by_id.insert(unit.id.as_str(), index).is_some() {
            return invalid(format!("units[{index}].id"), "duplicate unit id");
        }
    }

    for (index, unit) in observation.units.iter().enumerate() {
        let path = format!("units[{index}]");
        if let Some(parent_id) = &unit.parent_id {
            let Some(&parent_index) = unit_by_id.get(parent_id.as_str()) else {
                return invalid(format!("{path}.parent_id"), "unknown parent unit id");
            };
            if !valid_parent_child_kind(observation.units[parent_index].kind, unit.kind) {
                return invalid(
                    format!("{path}.parent_id"),
                    "parent and child granularities are incompatible",
                );
            }
        } else if matches!(unit.span_in_parent, Claim::Known { .. }) {
            return invalid(
                format!("{path}.span_in_parent"),
                "a top-level unit cannot have a known parent span",
            );
        }

        validate_claim(
            &unit.reading_order,
            format!("{path}.reading_order"),
            provenance_by_id,
            |_| Ok(()),
        )?;
        validate_claim(
            &unit.text,
            format!("{path}.text"),
            provenance_by_id,
            |text| {
                if text.is_empty() {
                    invalid(format!("{path}.text.value"), "known text must not be empty")
                } else {
                    Ok(())
                }
            },
        )?;
        validate_claim(
            &unit.span_in_parent,
            format!("{path}.span_in_parent"),
            provenance_by_id,
            |span| {
                if span.start >= span.end {
                    invalid(
                        format!("{path}.span_in_parent.value"),
                        "Unicode span must be non-empty and semi-open",
                    )
                } else {
                    Ok(())
                }
            },
        )?;
        validate_geometry(
            &unit.geometry,
            &path,
            &observation.raster_frame,
            provenance_by_id,
        )?;
    }

    let mut state = vec![0_u8; observation.units.len()];
    for index in 0..observation.units.len() {
        visit_unit(index, &observation.units, &unit_by_id, &mut state)?;
    }

    validate_reading_order_uniqueness(&observation.units)?;

    for (index, child) in observation.units.iter().enumerate() {
        let Some(parent_id) = &child.parent_id else {
            continue;
        };
        let parent = &observation.units[unit_by_id[parent_id.as_str()]];
        validate_child_text(index, child, parent)?;
        validate_geometry_independence(index, child, parent)?;
    }

    Ok(())
}

fn valid_parent_child_kind(parent: ObservationKind, child: ObservationKind) -> bool {
    matches!(
        (parent, child),
        (ObservationKind::Region, ObservationKind::Line)
            | (ObservationKind::Line, ObservationKind::Word)
            | (ObservationKind::Word, ObservationKind::Glyph)
    )
}

fn visit_unit(
    index: usize,
    units: &[ObservationUnit],
    by_id: &HashMap<&str, usize>,
    state: &mut [u8],
) -> ValidationResult {
    match state[index] {
        1 => return invalid("units", "cycle in unit parent graph"),
        2 => return Ok(()),
        _ => {}
    }
    state[index] = 1;
    if let Some(parent_id) = &units[index].parent_id {
        visit_unit(by_id[parent_id.as_str()], units, by_id, state)?;
    }
    state[index] = 2;
    Ok(())
}

fn validate_reading_order_uniqueness(units: &[ObservationUnit]) -> ValidationResult {
    let mut seen = HashSet::new();
    for unit in units {
        if let Claim::Known { value, .. } = unit.reading_order {
            let sibling_group = unit.parent_id.as_deref().unwrap_or("");
            if !seen.insert((sibling_group, value)) {
                return invalid(
                    "units.reading_order",
                    "known reading order must be unique among siblings",
                );
            }
        }
    }
    Ok(())
}

fn validate_child_text(
    index: usize,
    child: &ObservationUnit,
    parent: &ObservationUnit,
) -> ValidationResult {
    let (
        Claim::Known {
            value: child_text, ..
        },
        Claim::Known { value: span, .. },
        Claim::Known {
            value: parent_text, ..
        },
    ) = (&child.text, &child.span_in_parent, &parent.text)
    else {
        return Ok(());
    };

    let scalar_count = parent_text.chars().count();
    if span.end > scalar_count {
        return invalid(
            format!("units[{index}].span_in_parent"),
            "span exceeds the parent Unicode scalar sequence",
        );
    }
    let excerpt: String = parent_text
        .chars()
        .skip(span.start)
        .take(span.end - span.start)
        .collect();
    if excerpt != *child_text {
        return invalid(
            format!("units[{index}].span_in_parent"),
            "span does not select the known child text",
        );
    }
    Ok(())
}

fn validate_geometry(
    geometry: &GeometryClaims,
    unit_path: &str,
    frame: &RasterFrame,
    provenance_by_id: &HashMap<&str, usize>,
) -> ValidationResult {
    validate_claim(
        &geometry.bbox,
        format!("{unit_path}.geometry.bbox"),
        provenance_by_id,
        |bbox| validate_bbox(bbox, frame),
    )?;
    validate_claim(
        &geometry.polygon,
        format!("{unit_path}.geometry.polygon"),
        provenance_by_id,
        |polygon| validate_points(&polygon.frame_id, &polygon.points, 3, frame),
    )?;
    validate_claim(
        &geometry.baseline,
        format!("{unit_path}.geometry.baseline"),
        provenance_by_id,
        |baseline| validate_points(&baseline.frame_id, &baseline.points, 2, frame),
    )?;

    if let (Claim::Known { value: bbox, .. }, Claim::Known { value: polygon, .. }) =
        (&geometry.bbox, &geometry.polygon)
    {
        if bbox.frame_id != polygon.frame_id {
            return invalid(
                format!("{unit_path}.geometry"),
                "known bbox and polygon must use the same frame",
            );
        }
        let (min_x, min_y, max_x, max_y) = point_envelope(&polygon.points);
        const EPSILON: f64 = 1.0e-9;
        if (bbox.x0 - min_x).abs() > EPSILON
            || (bbox.y0 - min_y).abs() > EPSILON
            || (bbox.x1 - max_x).abs() > EPSILON
            || (bbox.y1 - max_y).abs() > EPSILON
        {
            return invalid(
                format!("{unit_path}.geometry"),
                "known bbox must be the polygon envelope",
            );
        }
    }
    Ok(())
}

fn validate_bbox(bbox: &FramedBbox, frame: &RasterFrame) -> ValidationResult {
    if bbox.frame_id != frame.id {
        return invalid(
            "geometry.bbox.frame_id",
            "bbox uses an unknown raster frame",
        );
    }
    let values = [bbox.x0, bbox.y0, bbox.x1, bbox.y1];
    if values.iter().any(|value| !value.is_finite()) {
        return invalid("geometry.bbox", "bbox coordinates must be finite");
    }
    if bbox.x0 < 0.0
        || bbox.y0 < 0.0
        || bbox.x0 >= bbox.x1
        || bbox.y0 >= bbox.y1
        || bbox.x1 > f64::from(frame.extent.0)
        || bbox.y1 > f64::from(frame.extent.1)
    {
        return invalid(
            "geometry.bbox",
            "bbox must be non-degenerate and inside the raster frame",
        );
    }
    Ok(())
}

fn validate_points(
    frame_id: &str,
    points: &[(f64, f64)],
    minimum: usize,
    frame: &RasterFrame,
) -> ValidationResult {
    if frame_id != frame.id {
        return invalid("geometry.frame_id", "geometry uses an unknown raster frame");
    }
    if points.len() < minimum {
        return invalid("geometry.points", "geometry has too few points");
    }
    for &(x, y) in points {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x > f64::from(frame.extent.0)
            || y > f64::from(frame.extent.1)
        {
            return invalid(
                "geometry.points",
                "all points must be finite and inside the raster frame",
            );
        }
    }
    Ok(())
}

fn validate_geometry_independence(
    index: usize,
    child: &ObservationUnit,
    parent: &ObservationUnit,
) -> ValidationResult {
    if copied_claim(&child.geometry.bbox, &parent.geometry.bbox)
        || copied_claim(&child.geometry.polygon, &parent.geometry.polygon)
        || copied_claim(&child.geometry.baseline, &parent.geometry.baseline)
    {
        return invalid(
            format!("units[{index}].geometry"),
            "child geometry may not copy parent geometry and evidence",
        );
    }
    Ok(())
}

fn copied_claim<T: PartialEq>(child: &Claim<T>, parent: &Claim<T>) -> bool {
    match (child, parent) {
        (
            Claim::Known {
                value: child_value,
                evidence: child_evidence,
                ..
            },
            Claim::Known {
                value: parent_value,
                evidence: parent_evidence,
                ..
            },
        ) => child_value == parent_value && child_evidence == parent_evidence,
        _ => false,
    }
}

fn validate_page_mapping(mapping: &PageMapping, frame: &RasterFrame) -> ValidationResult {
    if mapping.source_frame_id != frame.id {
        return invalid(
            "page_mapping.value.source_frame_id",
            "mapping source must be the raster frame",
        );
    }
    require_nonempty(
        "page_mapping.value.target_frame.id",
        &mapping.target_frame.id,
    )?;
    let (width, height) = mapping.target_frame.extent;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return invalid(
            "page_mapping.value.target_frame.extent",
            "target extent must be finite and positive",
        );
    }
    if mapping.matrix.iter().any(|value| !value.is_finite()) {
        return invalid(
            "page_mapping.value.matrix",
            "homography coefficients must be finite",
        );
    }
    if !mapping.max_error_pt.is_finite() || mapping.max_error_pt < 0.0 {
        return invalid(
            "page_mapping.value.max_error_pt",
            "maximum error must be finite and non-negative",
        );
    }
    let m = &mapping.matrix;
    let determinant = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
        + m[2] * (m[3] * m[7] - m[4] * m[6]);
    if !determinant.is_finite() || determinant.abs() <= f64::EPSILON {
        return invalid(
            "page_mapping.value.matrix",
            "homography must be non-singular",
        );
    }
    Ok(())
}

fn validate_mapping_horizon(mapping: &PageMapping, units: &[ObservationUnit]) -> ValidationResult {
    let mut points = Vec::new();
    for unit in units {
        if let Claim::Known { value: bbox, .. } = &unit.geometry.bbox {
            points.extend_from_slice(&[
                (bbox.x0, bbox.y0),
                (bbox.x1, bbox.y0),
                (bbox.x0, bbox.y1),
                (bbox.x1, bbox.y1),
            ]);
        }
        if let Claim::Known { value, .. } = &unit.geometry.polygon {
            points.extend(value.points.iter().copied());
        }
        if let Claim::Known { value, .. } = &unit.geometry.baseline {
            points.extend(value.points.iter().copied());
        }
    }
    for (x, y) in points {
        let denominator = mapping.matrix[6] * x + mapping.matrix[7] * y + mapping.matrix[8];
        if !denominator.is_finite() || denominator.abs() <= f64::EPSILON {
            return invalid(
                "page_mapping.value.matrix",
                "homography has a zero denominator at observed geometry",
            );
        }
    }
    Ok(())
}

fn validate_claim<T>(
    claim: &Claim<T>,
    path: impl Into<String>,
    provenance_by_id: &HashMap<&str, usize>,
    validate_value: impl FnOnce(&T) -> ValidationResult,
) -> ValidationResult {
    let path = path.into();
    match claim {
        Claim::Known {
            value,
            evidence,
            confidence,
            ..
        } => {
            if evidence.is_empty() {
                return invalid(format!("{path}.evidence"), "known claim needs evidence");
            }
            validate_evidence(evidence, &path, provenance_by_id)?;
            validate_confidence(confidence, &path)?;
            validate_value(value)
        }
        Claim::Unknown {
            evidence, detail, ..
        } => {
            validate_evidence(evidence, &path, provenance_by_id)?;
            if detail.as_ref().is_some_and(|value| value.trim().is_empty()) {
                return invalid(
                    format!("{path}.detail"),
                    "unknown detail, when present, must not be empty",
                );
            }
            Ok(())
        }
    }
}

fn validate_evidence(
    evidence: &[ProvenanceId],
    path: &str,
    provenance_by_id: &HashMap<&str, usize>,
) -> ValidationResult {
    for id in evidence {
        if !provenance_by_id.contains_key(id.as_str()) {
            return invalid(format!("{path}.evidence"), "unknown provenance id");
        }
    }
    Ok(())
}

fn validate_confidence(confidence: &Confidence, path: &str) -> ValidationResult {
    if let Confidence::Known { value, semantics } = confidence {
        if !value.is_finite() || !(0.0..=1.0).contains(value) {
            return invalid(
                format!("{path}.confidence.value"),
                "known confidence must be finite and in [0, 1]",
            );
        }
        require_nonempty(format!("{path}.confidence.semantics"), semantics)?;
    }
    Ok(())
}

fn point_envelope(points: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for &(x, y) in points {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    (min_x, min_y, max_x, max_y)
}

fn require_nonempty(path: impl Into<String>, value: &str) -> ValidationResult {
    if value.trim().is_empty() {
        invalid(path, "value must not be empty")
    } else {
        Ok(())
    }
}

fn require_sha256(path: impl Into<String>, value: &str) -> ValidationResult {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        invalid(
            path,
            "SHA-256 must have 64 lowercase hexadecimal characters",
        )
    } else {
        Ok(())
    }
}

fn invalid<T>(
    path: impl Into<String>,
    message: impl Into<String>,
) -> Result<T, ScanObservationValidationError> {
    Err(ScanObservationValidationError::new(path, message))
}
