use decalque_core::{
    ConfidenceRequirement, EvidenceStatus, GeometryComponent, ScanComparisonDiagnostic,
    ScanComparisonPolicy, ScanComparisonReport, ScanGranularity, TextNormalization, UnknownReason,
};
use std::fmt::{self, Write};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReportDiagnostic {
    pub code: String,
    pub level: Option<String>,
    pub detail: String,
}

impl ScanReportDiagnostic {
    /// Preserva o diagnóstico integral e deriva um código estável do nome da
    /// variante. A forma `Debug` dos diagnósticos de domínio é determinística.
    pub fn from_debug(value: &impl fmt::Debug) -> Self {
        let detail = format!("{value:?}");
        let variant = detail
            .split(|character: char| {
                character == '{' || character == '(' || character.is_whitespace()
            })
            .next()
            .unwrap_or("diagnostic");
        Self {
            code: kebab_case(variant),
            level: None,
            detail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReportRenderError {
    field: &'static str,
}

impl fmt::Display for ScanReportRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "valor não finito ao serializar o campo {}",
            self.field
        )
    }
}

impl std::error::Error for ScanReportRenderError {}

pub struct ScanReportContext<'a> {
    pub page_index: usize,
    pub raster_sha256: &'a str,
    pub observation_diagnostics: &'a [ScanReportDiagnostic],
    pub pdf_diagnostics: &'a [ScanReportDiagnostic],
}

/// Renderiza o relatório v1 com ordem fixa de campos e sem dependência de
/// serialização. Deltas `None` são sempre omitidos.
pub fn render_scan_comparison_report(
    context: &ScanReportContext<'_>,
    policy: &ScanComparisonPolicy,
    report: &ScanComparisonReport,
) -> Result<String, ScanReportRenderError> {
    let mut json = String::new();
    json.push_str("{\"schema\":\"decalque.scan-comparison-report\",\"schema_version\":1");

    write!(json, ",\"source\":{{\"page_index\":{}", context.page_index).unwrap();
    json.push_str(",\"raster_sha256\":");
    push_json_string(&mut json, context.raster_sha256);
    json.push('}');

    json.push_str(",\"policy\":{");
    json.push_str("\"granularity\":");
    push_json_string(
        &mut json,
        match &policy.granularity {
            ScanGranularity::Line => "line",
            ScanGranularity::Word => "word",
        },
    );
    json.push_str(",\"text_normalization\":");
    push_json_string(
        &mut json,
        match &policy.text_normalization {
            TextNormalization::Exact => "exact",
        },
    );
    json.push_str(",\"horizontal_tolerance_pt\":");
    push_f64(
        &mut json,
        policy.horizontal_tolerance_pt,
        "policy.horizontal_tolerance_pt",
    )?;
    json.push_str(",\"baseline_tolerance_pt\":");
    push_f64(
        &mut json,
        policy.baseline_tolerance_pt,
        "policy.baseline_tolerance_pt",
    )?;
    json.push_str(",\"min_text_confidence\":");
    push_confidence_requirement(
        &mut json,
        &policy.text_confidence,
        "policy.min_text_confidence",
    )?;
    json.push_str(",\"min_geometry_confidence\":");
    push_confidence_requirement(
        &mut json,
        &policy.geometry_confidence,
        "policy.min_geometry_confidence",
    )?;
    json.push('}');

    json.push_str(",\"content_status\":");
    push_json_string(&mut json, status(&report.content_status));
    json.push_str(",\"geometry_status\":");
    push_json_string(&mut json, status(&report.geometry_status));
    json.push_str(",\"overall_status\":");
    push_json_string(&mut json, status(&report.overall_status));

    write!(
        json,
        ",\"coverage\":{{\"matched_scan\":{},\"total_scan\":{},\"matched_candidate\":{},\"total_candidate\":{}}}",
        report.coverage.matched_scan,
        report.coverage.total_scan,
        report.coverage.matched_candidate,
        report.coverage.total_candidate,
    )
    .unwrap();

    json.push_str(",\"matches\":[");
    for (index, scan_match) in report.matches.iter().enumerate() {
        push_separator(&mut json, index);
        json.push_str("{\"scan_unit_id\":");
        push_json_string(&mut json, &scan_match.scan_unit_id);
        write!(
            json,
            ",\"candidate_line_index\":{},\"candidate_scalar_range\":[{},{}]",
            scan_match.candidate_line_index,
            scan_match.candidate_scalar_range.0,
            scan_match.candidate_scalar_range.1,
        )
        .unwrap();
        push_optional_f64(&mut json, "dx_start", scan_match.dx_start)?;
        push_optional_f64(&mut json, "dx_end", scan_match.dx_end)?;
        push_optional_f64(&mut json, "width_delta", scan_match.width_delta)?;
        push_optional_f64(&mut json, "baseline_delta", scan_match.baseline_delta)?;
        json.push_str(",\"horizontal_status\":");
        push_json_string(&mut json, status(&scan_match.horizontal_status));
        json.push_str(",\"baseline_status\":");
        push_json_string(&mut json, status(&scan_match.baseline_status));
        json.push('}');
    }
    json.push(']');

    json.push_str(",\"unmatched_scan\":[");
    for (index, scan_unit_id) in report.unmatched_scan.iter().enumerate() {
        push_separator(&mut json, index);
        push_json_string(&mut json, scan_unit_id);
    }
    json.push(']');

    json.push_str(",\"unmatched_candidate\":[");
    for (index, candidate) in report.unmatched_candidate.iter().enumerate() {
        push_separator(&mut json, index);
        write!(
            json,
            "{{\"candidate_line_index\":{},\"candidate_scalar_range\":[{},{}]}}",
            candidate.candidate_line_index,
            candidate.candidate_scalar_range.0,
            candidate.candidate_scalar_range.1,
        )
        .unwrap();
    }
    json.push(']');

    json.push_str(",\"reflow\":[");
    for (index, reflow) in report.reflow.iter().enumerate() {
        push_separator(&mut json, index);
        json.push_str("{\"scan_line_id\":");
        push_json_string(&mut json, &reflow.scan_line_id);
        json.push_str(",\"candidate_line_indices\":[");
        for (line_index, candidate_line_index) in reflow.candidate_line_indices.iter().enumerate() {
            push_separator(&mut json, line_index);
            write!(json, "{candidate_line_index}").unwrap();
        }
        json.push_str("],\"scan_word_ids\":[");
        for (word_index, scan_word_id) in reflow.scan_word_ids.iter().enumerate() {
            push_separator(&mut json, word_index);
            push_json_string(&mut json, scan_word_id);
        }
        json.push_str("]}");
    }
    json.push(']');

    json.push_str(",\"observation_diagnostics\":");
    push_diagnostics(&mut json, context.observation_diagnostics);
    json.push_str(",\"pdf_diagnostics\":");
    push_diagnostics(&mut json, context.pdf_diagnostics);
    json.push_str(",\"comparison_diagnostics\":[");
    for (index, diagnostic) in report.diagnostics.iter().enumerate() {
        push_separator(&mut json, index);
        push_comparison_diagnostic(&mut json, diagnostic);
    }
    json.push_str("]}");

    Ok(json)
}

fn status(status: &EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Preserved => "preserved",
        EvidenceStatus::Violated => "violated",
        EvidenceStatus::Unknown => "unknown",
    }
}

fn push_confidence_requirement(
    json: &mut String,
    requirement: &ConfidenceRequirement,
    field: &'static str,
) -> Result<(), ScanReportRenderError> {
    match requirement {
        ConfidenceRequirement::Any => json.push_str("{\"status\":\"not-required\"}"),
        ConfidenceRequirement::KnownAtLeast(value) => {
            json.push_str("{\"status\":\"known\",\"value\":");
            push_f64(json, *value, field)?;
            json.push('}');
        }
    }
    Ok(())
}

fn push_optional_f64(
    json: &mut String,
    name: &'static str,
    value: Option<f64>,
) -> Result<(), ScanReportRenderError> {
    if let Some(value) = value {
        write!(json, ",\"{name}\":").unwrap();
        push_f64(json, value, name)?;
    }
    Ok(())
}

fn push_f64(
    json: &mut String,
    value: f64,
    field: &'static str,
) -> Result<(), ScanReportRenderError> {
    if !value.is_finite() {
        return Err(ScanReportRenderError { field });
    }
    write!(json, "{value}").unwrap();
    Ok(())
}

fn push_diagnostics(json: &mut String, diagnostics: &[ScanReportDiagnostic]) {
    json.push('[');
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        push_separator(json, index);
        push_diagnostic(json, diagnostic);
    }
    json.push(']');
}

fn push_diagnostic(json: &mut String, diagnostic: &ScanReportDiagnostic) {
    json.push_str("{\"code\":");
    push_json_string(json, &diagnostic.code);
    if let Some(level) = &diagnostic.level {
        json.push_str(",\"level\":");
        push_json_string(json, level);
    }
    json.push_str(",\"detail\":");
    push_json_string(json, &diagnostic.detail);
    json.push('}');
}

fn push_comparison_diagnostic(json: &mut String, diagnostic: &ScanComparisonDiagnostic) {
    json.push_str("{\"code\":");
    let code = match diagnostic {
        ScanComparisonDiagnostic::TextClaimUnknown { .. } => "text-claim-unknown",
        ScanComparisonDiagnostic::TextConfidenceBelowPolicy { .. } => {
            "text-confidence-below-policy"
        }
        ScanComparisonDiagnostic::AmbiguousTextMatch { .. } => "ambiguous-text-match",
        ScanComparisonDiagnostic::UnmatchedText { .. } => "unmatched-text",
        ScanComparisonDiagnostic::CandidateUnmappedGlyph { .. } => "candidate-unmapped-glyph",
        ScanComparisonDiagnostic::GeometryClaimUnknown { .. } => "geometry-claim-unknown",
        ScanComparisonDiagnostic::GeometryConfidenceBelowPolicy { .. } => {
            "geometry-confidence-below-policy"
        }
        ScanComparisonDiagnostic::PageMappingUnknown { .. } => "page-mapping-unknown",
        ScanComparisonDiagnostic::UnsupportedCandidateCoordinateMapping { .. } => {
            "unsupported-candidate-coordinate-mapping"
        }
        ScanComparisonDiagnostic::ReflowDetected { .. } => "reflow-detected",
        ScanComparisonDiagnostic::InvalidPolicy { .. } => "invalid-policy",
        ScanComparisonDiagnostic::EmptyComparisonScope => "empty-comparison-scope",
    };
    push_json_string(json, code);

    match diagnostic {
        ScanComparisonDiagnostic::TextClaimUnknown {
            scan_unit_id,
            reason,
        }
        | ScanComparisonDiagnostic::PageMappingUnknown {
            scan_unit_id,
            reason,
        } => {
            push_string_field(json, "scan_unit_id", scan_unit_id);
            push_string_field(json, "reason", unknown_reason(*reason));
        }
        ScanComparisonDiagnostic::TextConfidenceBelowPolicy { scan_unit_id }
        | ScanComparisonDiagnostic::UnmatchedText { scan_unit_id }
        | ScanComparisonDiagnostic::UnsupportedCandidateCoordinateMapping { scan_unit_id } => {
            push_string_field(json, "scan_unit_id", scan_unit_id);
        }
        ScanComparisonDiagnostic::AmbiguousTextMatch {
            scan_unit_id,
            candidate_line_indices,
        } => {
            push_string_field(json, "scan_unit_id", scan_unit_id);
            push_usize_array_field(json, "candidate_line_indices", candidate_line_indices);
        }
        ScanComparisonDiagnostic::CandidateUnmappedGlyph {
            candidate_line_index,
        } => {
            write!(json, ",\"candidate_line_index\":{candidate_line_index}").unwrap();
        }
        ScanComparisonDiagnostic::GeometryClaimUnknown {
            scan_unit_id,
            component,
            reason,
        } => {
            push_string_field(json, "scan_unit_id", scan_unit_id);
            push_string_field(json, "component", geometry_component(*component));
            push_string_field(json, "reason", unknown_reason(*reason));
        }
        ScanComparisonDiagnostic::GeometryConfidenceBelowPolicy {
            scan_unit_id,
            component,
        } => {
            push_string_field(json, "scan_unit_id", scan_unit_id);
            push_string_field(json, "component", geometry_component(*component));
        }
        ScanComparisonDiagnostic::ReflowDetected {
            scan_line_id,
            candidate_line_indices,
        } => {
            push_string_field(json, "scan_line_id", scan_line_id);
            push_usize_array_field(json, "candidate_line_indices", candidate_line_indices);
        }
        ScanComparisonDiagnostic::InvalidPolicy { field } => {
            push_string_field(json, "field", field);
        }
        ScanComparisonDiagnostic::EmptyComparisonScope => {}
    }
    json.push('}');
}

fn push_string_field(json: &mut String, name: &str, value: &str) {
    write!(json, ",\"{name}\":").unwrap();
    push_json_string(json, value);
}

fn push_usize_array_field(json: &mut String, name: &str, values: &[usize]) {
    write!(json, ",\"{name}\":").unwrap();
    json.push('[');
    for (index, value) in values.iter().enumerate() {
        push_separator(json, index);
        write!(json, "{value}").unwrap();
    }
    json.push(']');
}

fn unknown_reason(reason: UnknownReason) -> &'static str {
    match reason {
        UnknownReason::NotObserved => "not-observed",
        UnknownReason::Ambiguous => "ambiguous",
        UnknownReason::Unsupported => "unsupported",
        UnknownReason::Invalid => "invalid",
        UnknownReason::BudgetExhausted => "budget-exhausted",
        UnknownReason::Redacted => "redacted",
        UnknownReason::BelowPolicyThreshold => "below-policy-threshold",
    }
}

fn geometry_component(component: GeometryComponent) -> &'static str {
    match component {
        GeometryComponent::PageMapping => "page-mapping",
        GeometryComponent::Horizontal => "horizontal",
        GeometryComponent::Baseline => "baseline",
        GeometryComponent::CandidateInterval => "candidate-interval",
    }
}

fn push_separator(json: &mut String, index: usize) {
    if index != 0 {
        json.push(',');
    }
}

pub(crate) fn push_json_string(json: &mut String, value: &str) {
    json.push('"');
    for character in value.chars() {
        match character {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\u{08}' => json.push_str("\\b"),
            '\u{0c}' => json.push_str("\\f"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            '\u{00}'..='\u{1f}' => write!(json, "\\u{:04x}", character as u32).unwrap(),
            _ => json.push(character),
        }
    }
    json.push('"');
}

fn kebab_case(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for (index, character) in value.chars().enumerate() {
        if character.is_uppercase() {
            if index != 0 {
                result.push('-');
            }
            for lowercase in character.to_lowercase() {
                result.push(lowercase);
            }
        } else if character == '_' {
            result.push('-');
        } else {
            result.push(character);
        }
    }
    if result.is_empty() {
        "diagnostic".to_string()
    } else {
        result
    }
}
