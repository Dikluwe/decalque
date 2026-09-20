//! Planejamento puro da reconstrução geométrica por linhas.

use std::collections::HashMap;

use crate::entities::{
    validate_scan_observation, Claim, FramedBbox, FramedPolyline, ObservationKind, ObservationUnit,
    PageMapping, ReconstructionClaimKind, ReconstructionCoverage, ReconstructionDiagnostic,
    ReconstructionInputError, ReconstructionLine, ReconstructionOutcome, ReconstructionPlan,
    ReconstructionUnknownReport, ScanObservation, TypographyHypothesis,
};

pub fn plan_scan_lines(
    scan: &ScanObservation,
    typography: &TypographyHypothesis,
) -> Result<ReconstructionOutcome, ReconstructionInputError> {
    validate_typography(typography)?;
    validate_scan_observation(scan)?;

    let mut lines = scan
        .units
        .iter()
        .filter(|unit| unit.kind == ObservationKind::Line)
        .collect::<Vec<_>>();
    lines.sort_by(|left, right| left.id.cmp(&right.id));

    let total_lines = lines.len();
    if lines.is_empty() {
        return Ok(unknown_outcome(
            ReconstructionCoverage {
                planned_lines: 0,
                total_lines: 0,
            },
            vec![ReconstructionDiagnostic::NoLines],
        ));
    }

    let mapping = match &scan.page_mapping {
        Claim::Known { value, .. } => value,
        Claim::Unknown { .. } => {
            return Ok(unknown_outcome(
                ReconstructionCoverage {
                    planned_lines: 0,
                    total_lines,
                },
                vec![ReconstructionDiagnostic::PageMappingUnknown],
            ));
        }
    };

    let units_by_id = scan
        .units
        .iter()
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<HashMap<_, _>>();
    let mut planned_lines = Vec::with_capacity(total_lines);
    let mut diagnostics = Vec::new();

    for line in lines {
        let text = required_claim(
            &line.text,
            &line.id,
            ReconstructionClaimKind::Text,
            &mut diagnostics,
        );
        let bbox = required_claim(
            &line.geometry.bbox,
            &line.id,
            ReconstructionClaimKind::Bbox,
            &mut diagnostics,
        );
        let reading_key = match hierarchical_reading_key(line, &units_by_id) {
            Some(key) => Some(key),
            None => {
                diagnostics.push(ReconstructionDiagnostic::RequiredClaimUnknown {
                    unit_id: line.id.clone(),
                    claim: ReconstructionClaimKind::ReadingOrder,
                });
                None
            }
        };

        let (Some(text), Some(bbox), Some(reading_key)) = (text, bbox, reading_key) else {
            continue;
        };
        if let Some((scalar_index, code_point)) = explicit_line_separator(text) {
            diagnostics.push(ReconstructionDiagnostic::ExplicitLineSeparator {
                unit_id: line.id.clone(),
                scalar_index,
                code_point,
            });
            continue;
        }

        let Some(target_bbox) = project_bbox(mapping, bbox) else {
            diagnostics.push(ReconstructionDiagnostic::ProjectionFailed {
                unit_id: line.id.clone(),
                claim: ReconstructionClaimKind::Bbox,
            });
            continue;
        };
        let Some(target_baseline) = project_baseline(mapping, &line.geometry.baseline) else {
            diagnostics.push(ReconstructionDiagnostic::ProjectionFailed {
                unit_id: line.id.clone(),
                claim: ReconstructionClaimKind::Baseline,
            });
            continue;
        };

        planned_lines.push(ReconstructionLine {
            source_unit_id: line.id.clone(),
            reading_key,
            text: text.clone(),
            target_bbox,
            target_baseline,
        });
    }

    planned_lines.sort_by(|left, right| left.reading_key.cmp(&right.reading_key));
    append_ambiguous_reading_order_diagnostics(&planned_lines, &mut diagnostics);

    let coverage = ReconstructionCoverage {
        planned_lines: planned_lines.len(),
        total_lines,
    };
    if !diagnostics.is_empty() {
        return Ok(unknown_outcome(coverage, diagnostics));
    }

    Ok(ReconstructionOutcome::Materializable(ReconstructionPlan {
        page: mapping.target_frame.clone(),
        mapping_max_error_pt: mapping.max_error_pt,
        typography: typography.clone(),
        lines: planned_lines,
        coverage,
    }))
}

fn validate_typography(typography: &TypographyHypothesis) -> Result<(), ReconstructionInputError> {
    if typography.font_family.trim().is_empty() {
        return Err(ReconstructionInputError::EmptyFontFamily);
    }
    if !typography.size_pt.is_finite() || typography.size_pt <= 0.0 {
        return Err(ReconstructionInputError::InvalidFontSizePt);
    }
    if !typography.tracking_pt.is_finite() {
        return Err(ReconstructionInputError::InvalidTrackingPt);
    }
    Ok(())
}

fn explicit_line_separator(text: &str) -> Option<(usize, u32)> {
    text.chars().enumerate().find_map(|(scalar_index, scalar)| {
        matches!(
            scalar,
            '\n' | '\u{000b}' | '\u{000c}' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}'
        )
        .then_some((scalar_index, u32::from(scalar)))
    })
}

fn required_claim<'a, T>(
    claim: &'a Claim<T>,
    unit_id: &str,
    kind: ReconstructionClaimKind,
    diagnostics: &mut Vec<ReconstructionDiagnostic>,
) -> Option<&'a T> {
    match claim {
        Claim::Known { value, .. } => Some(value),
        Claim::Unknown { .. } => {
            diagnostics.push(ReconstructionDiagnostic::RequiredClaimUnknown {
                unit_id: unit_id.to_string(),
                claim: kind,
            });
            None
        }
    }
}

fn hierarchical_reading_key(
    line: &ObservationUnit,
    units_by_id: &HashMap<&str, &ObservationUnit>,
) -> Option<Vec<u32>> {
    let mut key = Vec::new();
    let mut current = Some(line);
    while let Some(unit) = current {
        key.push(*unit.reading_order.known_value()?);
        current = unit
            .parent_id
            .as_deref()
            .and_then(|parent_id| units_by_id.get(parent_id).copied());
    }
    key.reverse();
    Some(key)
}

fn project_bbox(mapping: &PageMapping, bbox: &FramedBbox) -> Option<FramedBbox> {
    let source_corners = [
        (bbox.x0, bbox.y0),
        (bbox.x1, bbox.y0),
        (bbox.x0, bbox.y1),
        (bbox.x1, bbox.y1),
    ];
    if !mapping_covers_points(mapping, &source_corners) {
        return None;
    }

    let target_corners = source_corners
        .map(|(x, y)| project_point(mapping, x, y))
        .into_iter()
        .collect::<Option<Vec<_>>>()?;
    let mut x0 = f64::INFINITY;
    let mut y0 = f64::INFINITY;
    let mut x1 = f64::NEG_INFINITY;
    let mut y1 = f64::NEG_INFINITY;
    for (x, y) in target_corners {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    if x0 >= x1 || y0 >= y1 {
        return None;
    }

    Some(FramedBbox {
        frame_id: mapping.target_frame.id.clone(),
        x0,
        y0,
        x1,
        y1,
    })
}

fn project_baseline(
    mapping: &PageMapping,
    baseline: &Claim<FramedPolyline>,
) -> Option<Claim<FramedPolyline>> {
    match baseline {
        Claim::Unknown {
            reason,
            evidence,
            detail,
        } => Some(Claim::Unknown {
            reason: *reason,
            evidence: evidence.clone(),
            detail: detail.clone(),
        }),
        Claim::Known {
            value,
            basis,
            evidence,
            confidence,
        } => {
            if !mapping_covers_points(mapping, &value.points) {
                return None;
            }
            let points = value
                .points
                .iter()
                .map(|&(x, y)| project_point(mapping, x, y))
                .collect::<Option<Vec<_>>>()?;
            Some(Claim::Known {
                value: FramedPolyline {
                    frame_id: mapping.target_frame.id.clone(),
                    points,
                },
                basis: *basis,
                evidence: evidence.clone(),
                confidence: confidence.clone(),
            })
        }
    }
}

fn mapping_covers_points(mapping: &PageMapping, points: &[(f64, f64)]) -> bool {
    let mut sign = None;
    for &(x, y) in points {
        let denominator = mapping.matrix[6] * x + mapping.matrix[7] * y + mapping.matrix[8];
        if !denominator.is_finite() || denominator.abs() <= f64::EPSILON {
            return false;
        }
        let current_sign = denominator.is_sign_positive();
        if sign.is_some_and(|expected| expected != current_sign) {
            return false;
        }
        sign = Some(current_sign);
    }
    true
}

fn project_point(mapping: &PageMapping, x: f64, y: f64) -> Option<(f64, f64)> {
    let matrix = &mapping.matrix;
    let denominator = matrix[6] * x + matrix[7] * y + matrix[8];
    if !denominator.is_finite() || denominator.abs() <= f64::EPSILON {
        return None;
    }
    let projected_x = (matrix[0] * x + matrix[1] * y + matrix[2]) / denominator;
    let projected_y = (matrix[3] * x + matrix[4] * y + matrix[5]) / denominator;
    (projected_x.is_finite() && projected_y.is_finite()).then_some((projected_x, projected_y))
}

fn append_ambiguous_reading_order_diagnostics(
    lines: &[ReconstructionLine],
    diagnostics: &mut Vec<ReconstructionDiagnostic>,
) {
    let mut index = 0;
    while index < lines.len() {
        let mut end = index + 1;
        while end < lines.len() && lines[end].reading_key == lines[index].reading_key {
            end += 1;
        }
        if end - index > 1 {
            let mut unit_ids = lines[index..end]
                .iter()
                .map(|line| line.source_unit_id.clone())
                .collect::<Vec<_>>();
            unit_ids.sort();
            diagnostics.push(ReconstructionDiagnostic::AmbiguousReadingOrder { unit_ids });
        }
        index = end;
    }
}

fn unknown_outcome(
    coverage: ReconstructionCoverage,
    diagnostics: Vec<ReconstructionDiagnostic>,
) -> ReconstructionOutcome {
    ReconstructionOutcome::Unknown(ReconstructionUnknownReport {
        coverage,
        diagnostics,
    })
}
