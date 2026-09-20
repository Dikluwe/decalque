//! Comparação pura entre observação de scan e geometria candidata, sem
//! serialização, I/O, OCR ou fabricação de glifos.

use std::collections::{HashMap, HashSet};

use crate::entities::{
    Claim, Confidence, DocumentGeometry, FramedBbox, FramedPolyline, ObservationKind,
    ObservationUnit, PageMapping, PageRotation, ScanObservation, TextMappingStatus, UnknownReason,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanGranularity {
    Line,
    Word,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfidenceRequirement {
    Any,
    KnownAtLeast(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextNormalization {
    Exact,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScanComparisonPolicy {
    pub granularity: ScanGranularity,
    pub horizontal_tolerance_pt: f64,
    pub baseline_tolerance_pt: f64,
    pub text_confidence: ConfidenceRequirement,
    pub geometry_confidence: ConfidenceRequirement,
    pub text_normalization: TextNormalization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceStatus {
    Preserved,
    Violated,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanCoverage {
    pub matched_scan: usize,
    pub total_scan: usize,
    pub matched_candidate: usize,
    pub total_candidate: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScanMatch {
    pub scan_unit_id: String,
    pub candidate_line_index: usize,
    pub candidate_scalar_range: (usize, usize),
    pub dx_start: Option<f64>,
    pub dx_end: Option<f64>,
    pub width_delta: Option<f64>,
    pub baseline_delta: Option<f64>,
    pub horizontal_status: EvidenceStatus,
    pub baseline_status: EvidenceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CandidateUnitRef {
    pub candidate_line_index: usize,
    pub candidate_scalar_range: (usize, usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflowWitness {
    pub scan_line_id: String,
    pub candidate_line_indices: Vec<usize>,
    pub scan_word_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryComponent {
    PageMapping,
    Horizontal,
    Baseline,
    CandidateInterval,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScanComparisonDiagnostic {
    TextClaimUnknown {
        scan_unit_id: String,
        reason: UnknownReason,
    },
    TextConfidenceBelowPolicy {
        scan_unit_id: String,
    },
    AmbiguousTextMatch {
        scan_unit_id: String,
        candidate_line_indices: Vec<usize>,
    },
    UnmatchedText {
        scan_unit_id: String,
    },
    CandidateUnmappedGlyph {
        candidate_line_index: usize,
    },
    GeometryClaimUnknown {
        scan_unit_id: String,
        component: GeometryComponent,
        reason: UnknownReason,
    },
    GeometryConfidenceBelowPolicy {
        scan_unit_id: String,
        component: GeometryComponent,
    },
    PageMappingUnknown {
        scan_unit_id: String,
        reason: UnknownReason,
    },
    UnsupportedCandidateCoordinateMapping {
        scan_unit_id: String,
    },
    ReflowDetected {
        scan_line_id: String,
        candidate_line_indices: Vec<usize>,
    },
    EmptyComparisonScope,
    InvalidPolicy {
        field: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScanComparisonReport {
    pub content_status: EvidenceStatus,
    pub geometry_status: EvidenceStatus,
    pub overall_status: EvidenceStatus,
    pub coverage: ScanCoverage,
    pub matches: Vec<ScanMatch>,
    pub unmatched_scan: Vec<String>,
    pub unmatched_candidate: Vec<CandidateUnitRef>,
    pub reflow: Vec<ReflowWitness>,
    pub diagnostics: Vec<ScanComparisonDiagnostic>,
}

#[derive(Debug, Clone, Copy)]
struct CandidateGeometry {
    x0: f64,
    x1: f64,
    baseline_y: f64,
}

#[derive(Debug)]
struct CandidateWord {
    scalar_range: (usize, usize),
    text: Option<String>,
    geometry: Option<CandidateGeometry>,
}

#[derive(Debug)]
struct CandidateLine {
    index: usize,
    text: Option<String>,
    scalar_glyphs: Vec<usize>,
    geometry: Option<CandidateGeometry>,
    words: Vec<CandidateWord>,
}

#[derive(Debug, Clone)]
enum AccessFailure {
    Unknown(UnknownReason),
    BelowPolicy,
}

#[derive(Debug, Clone)]
enum LineAssociationFailure {
    Claim(AccessFailure),
    Unmatched,
    Ambiguous(Vec<usize>),
    CandidateCollision(usize),
}

#[derive(Debug)]
struct ParentAssociation {
    candidate_line: Option<usize>,
    failure: Option<LineAssociationFailure>,
}

#[derive(Debug, Clone, Copy)]
struct ReportContext<'a> {
    scan: &'a ScanObservation,
    policy: &'a ScanComparisonPolicy,
}

pub fn compare_scan_observation(
    scan: &ScanObservation,
    candidate: &DocumentGeometry,
    policy: &ScanComparisonPolicy,
) -> ScanComparisonReport {
    let candidate_lines = build_candidate_lines(candidate);
    if let Some(field) = invalid_policy_field(policy) {
        return invalid_policy_report(scan, &candidate_lines, policy.granularity, field);
    }

    match policy.granularity {
        ScanGranularity::Line => compare_lines(scan, candidate, policy, &candidate_lines),
        ScanGranularity::Word => compare_words(scan, candidate, policy, &candidate_lines),
    }
}

fn compare_lines(
    scan: &ScanObservation,
    candidate: &DocumentGeometry,
    policy: &ScanComparisonPolicy,
    candidate_lines: &[CandidateLine],
) -> ScanComparisonReport {
    let units = sorted_scan_units(scan, ObservationKind::Line);
    let mut diagnostics = candidate_text_diagnostics(candidate_lines);
    let mut unmatched_scan = Vec::new();
    let mut provisional = Vec::new();

    for unit in &units {
        match claim_value(&unit.text, policy.text_confidence) {
            Ok(text) => {
                let candidates: Vec<usize> = candidate_lines
                    .iter()
                    .filter(|line| line.text.as_deref() == Some(text.as_str()))
                    .map(|line| line.index)
                    .collect();
                match candidates.as_slice() {
                    [line_index] => provisional.push((*unit, *line_index)),
                    [] => {
                        unmatched_scan.push(unit.id.clone());
                        diagnostics.push(ScanComparisonDiagnostic::UnmatchedText {
                            scan_unit_id: unit.id.clone(),
                        });
                    }
                    _ => {
                        unmatched_scan.push(unit.id.clone());
                        diagnostics.push(ScanComparisonDiagnostic::AmbiguousTextMatch {
                            scan_unit_id: unit.id.clone(),
                            candidate_line_indices: candidates,
                        });
                    }
                }
            }
            Err(failure) => {
                unmatched_scan.push(unit.id.clone());
                push_text_access_diagnostic(&mut diagnostics, &unit.id, failure);
            }
        }
    }

    let collisions = collision_counts(provisional.iter().map(|(_, line)| (*line, 0, 0)));
    let mut matches = Vec::new();
    let mut matched_candidates = HashSet::new();
    for (unit, line_index) in provisional {
        if collisions[&(line_index, 0, 0)] > 1 {
            unmatched_scan.push(unit.id.clone());
            diagnostics.push(ScanComparisonDiagnostic::AmbiguousTextMatch {
                scan_unit_id: unit.id.clone(),
                candidate_line_indices: vec![line_index],
            });
            continue;
        }
        let line = &candidate_lines[line_index];
        let scalar_range = (0, line.scalar_glyphs.len());
        matches.push(build_match(
            unit,
            CandidateUnitRef {
                candidate_line_index: line.index,
                candidate_scalar_range: scalar_range,
            },
            line.geometry,
            scan,
            candidate,
            policy,
            &mut diagnostics,
        ));
        matched_candidates.insert((line.index, 0, 0));
    }

    unmatched_scan.sort();
    unmatched_scan.dedup();
    let unmatched_candidate = candidate_lines
        .iter()
        .filter(|line| !matched_candidates.contains(&(line.index, 0, 0)))
        .map(line_reference)
        .collect::<Vec<_>>();
    let coverage = ScanCoverage {
        matched_scan: matches.len(),
        total_scan: units.len(),
        matched_candidate: matched_candidates.len(),
        total_candidate: candidate_lines.len(),
    };
    finish_report(
        ReportContext { scan, policy },
        coverage,
        matches,
        unmatched_scan,
        unmatched_candidate,
        Vec::new(),
        diagnostics,
    )
}

fn compare_words(
    scan: &ScanObservation,
    candidate: &DocumentGeometry,
    policy: &ScanComparisonPolicy,
    candidate_lines: &[CandidateLine],
) -> ScanComparisonReport {
    let units = sorted_scan_units(scan, ObservationKind::Word);
    let unit_by_id: HashMap<&str, &ObservationUnit> = scan
        .units
        .iter()
        .map(|unit| (unit.id.as_str(), unit))
        .collect();
    let parent_ids: HashSet<&str> = units
        .iter()
        .filter_map(|unit| unit.parent_id.as_deref())
        .collect();
    let parent_associations = associate_parent_lines(
        &parent_ids,
        &unit_by_id,
        candidate_lines,
        policy.text_confidence,
    );

    let mut diagnostics = candidate_text_diagnostics(candidate_lines);
    let mut unmatched_scan = Vec::new();
    let mut provisional: Vec<(&ObservationUnit, usize, usize)> = Vec::new();

    for unit in &units {
        let text = match claim_value(&unit.text, policy.text_confidence) {
            Ok(value) => value,
            Err(failure) => {
                unmatched_scan.push(unit.id.clone());
                push_text_access_diagnostic(&mut diagnostics, &unit.id, failure);
                continue;
            }
        };
        let span = match claim_value(&unit.span_in_parent, policy.text_confidence) {
            Ok(value) => *value,
            Err(failure) => {
                unmatched_scan.push(unit.id.clone());
                push_text_access_diagnostic(&mut diagnostics, &unit.id, failure);
                continue;
            }
        };
        let Some(parent_id) = unit.parent_id.as_deref() else {
            unmatched_scan.push(unit.id.clone());
            diagnostics.push(ScanComparisonDiagnostic::UnmatchedText {
                scan_unit_id: unit.id.clone(),
            });
            continue;
        };
        let Some(parent) = parent_associations.get(parent_id) else {
            unmatched_scan.push(unit.id.clone());
            diagnostics.push(ScanComparisonDiagnostic::UnmatchedText {
                scan_unit_id: unit.id.clone(),
            });
            continue;
        };
        let Some(line_index) = parent.candidate_line else {
            unmatched_scan.push(unit.id.clone());
            push_parent_failure_diagnostic(&mut diagnostics, &unit.id, parent.failure.as_ref());
            continue;
        };
        let line = &candidate_lines[line_index];
        let matches: Vec<usize> = line
            .words
            .iter()
            .enumerate()
            .filter(|(_, word)| {
                word.scalar_range == (span.start, span.end)
                    && word.text.as_deref() == Some(text.as_str())
            })
            .map(|(index, _)| index)
            .collect();
        match matches.as_slice() {
            [word_index] => provisional.push((*unit, line_index, *word_index)),
            [] => {
                unmatched_scan.push(unit.id.clone());
                diagnostics.push(ScanComparisonDiagnostic::UnmatchedText {
                    scan_unit_id: unit.id.clone(),
                });
            }
            _ => {
                unmatched_scan.push(unit.id.clone());
                diagnostics.push(ScanComparisonDiagnostic::AmbiguousTextMatch {
                    scan_unit_id: unit.id.clone(),
                    candidate_line_indices: vec![line_index],
                });
            }
        }
    }

    let collisions = collision_counts(provisional.iter().map(|(_, line_index, word_index)| {
        let word = &candidate_lines[*line_index].words[*word_index];
        (*line_index, word.scalar_range.0, word.scalar_range.1)
    }));
    let mut matches = Vec::new();
    let mut matched_candidates = HashSet::new();
    for (unit, line_index, word_index) in provisional {
        let line = &candidate_lines[line_index];
        let word = &line.words[word_index];
        let key = (line_index, word.scalar_range.0, word.scalar_range.1);
        if collisions[&key] > 1 {
            unmatched_scan.push(unit.id.clone());
            diagnostics.push(ScanComparisonDiagnostic::AmbiguousTextMatch {
                scan_unit_id: unit.id.clone(),
                candidate_line_indices: vec![line_index],
            });
            continue;
        }
        matches.push(build_match(
            unit,
            CandidateUnitRef {
                candidate_line_index: line_index,
                candidate_scalar_range: word.scalar_range,
            },
            word.geometry,
            scan,
            candidate,
            policy,
            &mut diagnostics,
        ));
        matched_candidates.insert(key);
    }

    let reflow = detect_reflow(
        &units,
        &parent_associations,
        candidate_lines,
        policy.text_confidence,
        &mut diagnostics,
    );
    unmatched_scan.sort();
    unmatched_scan.dedup();

    let all_candidate_words = candidate_word_references(candidate_lines);
    let unmatched_candidate = all_candidate_words
        .iter()
        .filter(|reference| {
            !matched_candidates.contains(&(
                reference.candidate_line_index,
                reference.candidate_scalar_range.0,
                reference.candidate_scalar_range.1,
            ))
        })
        .cloned()
        .collect::<Vec<_>>();
    let coverage = ScanCoverage {
        matched_scan: matches.len(),
        total_scan: units.len(),
        matched_candidate: matched_candidates.len(),
        total_candidate: all_candidate_words.len(),
    };
    finish_report(
        ReportContext { scan, policy },
        coverage,
        matches,
        unmatched_scan,
        unmatched_candidate,
        reflow,
        diagnostics,
    )
}

fn finish_report(
    context: ReportContext<'_>,
    coverage: ScanCoverage,
    matches: Vec<ScanMatch>,
    unmatched_scan: Vec<String>,
    unmatched_candidate: Vec<CandidateUnitRef>,
    reflow: Vec<ReflowWitness>,
    mut diagnostics: Vec<ScanComparisonDiagnostic>,
) -> ScanComparisonReport {
    let empty_scope = coverage.total_scan == 0 && coverage.total_candidate == 0;
    let nonempty_scope = coverage.total_scan > 0 && coverage.total_candidate > 0;
    let complete = nonempty_scope
        && coverage.matched_scan == coverage.total_scan
        && coverage.matched_candidate == coverage.total_candidate;

    if empty_scope {
        diagnostics.push(ScanComparisonDiagnostic::EmptyComparisonScope);
    }

    let page_mapping_available = match claim_value(
        &context.scan.page_mapping,
        context.policy.geometry_confidence,
    ) {
        Ok(_) => true,
        Err(AccessFailure::Unknown(reason)) => {
            if matches.is_empty() {
                diagnostics.push(ScanComparisonDiagnostic::PageMappingUnknown {
                    scan_unit_id: unmatched_scan
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "<comparison-scope>".to_string()),
                    reason,
                });
            }
            false
        }
        Err(AccessFailure::BelowPolicy) => {
            if matches.is_empty() {
                diagnostics.push(ScanComparisonDiagnostic::GeometryConfidenceBelowPolicy {
                    scan_unit_id: unmatched_scan
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "<comparison-scope>".to_string()),
                    component: GeometryComponent::PageMapping,
                });
            }
            false
        }
    };

    let content_status = if reflow.is_empty() {
        if complete {
            EvidenceStatus::Preserved
        } else {
            EvidenceStatus::Unknown
        }
    } else {
        EvidenceStatus::Violated
    };

    let has_geometry_violation = matches.iter().any(|matched| {
        matched.horizontal_status == EvidenceStatus::Violated
            || matched.baseline_status == EvidenceStatus::Violated
    });
    let has_geometry_unknown = matches.iter().any(|matched| {
        matched.horizontal_status == EvidenceStatus::Unknown
            || matched.baseline_status == EvidenceStatus::Unknown
    });
    let geometry_status = if has_geometry_violation {
        EvidenceStatus::Violated
    } else if !page_mapping_available
        || content_status != EvidenceStatus::Preserved
        || has_geometry_unknown
    {
        EvidenceStatus::Unknown
    } else {
        EvidenceStatus::Preserved
    };
    let overall_status = aggregate_status(content_status, geometry_status);

    ScanComparisonReport {
        content_status,
        geometry_status,
        overall_status,
        coverage,
        matches,
        unmatched_scan,
        unmatched_candidate,
        reflow,
        diagnostics,
    }
}

fn aggregate_status(left: EvidenceStatus, right: EvidenceStatus) -> EvidenceStatus {
    if left == EvidenceStatus::Violated || right == EvidenceStatus::Violated {
        EvidenceStatus::Violated
    } else if left == EvidenceStatus::Unknown || right == EvidenceStatus::Unknown {
        EvidenceStatus::Unknown
    } else {
        EvidenceStatus::Preserved
    }
}

fn build_match(
    unit: &ObservationUnit,
    candidate_unit: CandidateUnitRef,
    candidate_geometry: Option<CandidateGeometry>,
    scan: &ScanObservation,
    candidate: &DocumentGeometry,
    policy: &ScanComparisonPolicy,
    diagnostics: &mut Vec<ScanComparisonDiagnostic>,
) -> ScanMatch {
    let mut matched = ScanMatch {
        scan_unit_id: unit.id.clone(),
        candidate_line_index: candidate_unit.candidate_line_index,
        candidate_scalar_range: candidate_unit.candidate_scalar_range,
        dx_start: None,
        dx_end: None,
        width_delta: None,
        baseline_delta: None,
        horizontal_status: EvidenceStatus::Unknown,
        baseline_status: EvidenceStatus::Unknown,
    };

    if candidate.page.rotation != PageRotation::Deg0 || candidate.page.user_unit != 1.0 {
        diagnostics.push(
            ScanComparisonDiagnostic::UnsupportedCandidateCoordinateMapping {
                scan_unit_id: unit.id.clone(),
            },
        );
        return matched;
    }

    let mapping = match claim_value(&scan.page_mapping, policy.geometry_confidence) {
        Ok(mapping) => mapping,
        Err(AccessFailure::Unknown(reason)) => {
            diagnostics.push(ScanComparisonDiagnostic::PageMappingUnknown {
                scan_unit_id: unit.id.clone(),
                reason,
            });
            return matched;
        }
        Err(AccessFailure::BelowPolicy) => {
            diagnostics.push(ScanComparisonDiagnostic::GeometryConfidenceBelowPolicy {
                scan_unit_id: unit.id.clone(),
                component: GeometryComponent::PageMapping,
            });
            return matched;
        }
    };

    let Some(candidate_geometry) = candidate_geometry else {
        diagnostics.push(ScanComparisonDiagnostic::GeometryClaimUnknown {
            scan_unit_id: unit.id.clone(),
            component: GeometryComponent::CandidateInterval,
            reason: UnknownReason::Ambiguous,
        });
        return matched;
    };

    match claim_value(&unit.geometry.bbox, policy.geometry_confidence) {
        Ok(bbox) => {
            if let Some(projected) = project_bbox(mapping, bbox) {
                let dx_start = candidate_geometry.x0 - projected.x0;
                let dx_end = candidate_geometry.x1 - projected.x1;
                let candidate_width = candidate_geometry.x1 - candidate_geometry.x0;
                let scan_width = projected.x1 - projected.x0;
                let width_delta = candidate_width - scan_width;
                matched.dx_start = Some(dx_start);
                matched.dx_end = Some(dx_end);
                matched.width_delta = Some(width_delta);
                matched.horizontal_status = if dx_start.abs() <= policy.horizontal_tolerance_pt
                    && dx_end.abs() <= policy.horizontal_tolerance_pt
                    && width_delta.abs() <= policy.horizontal_tolerance_pt
                {
                    EvidenceStatus::Preserved
                } else {
                    EvidenceStatus::Violated
                };
            } else {
                diagnostics.push(ScanComparisonDiagnostic::GeometryClaimUnknown {
                    scan_unit_id: unit.id.clone(),
                    component: GeometryComponent::Horizontal,
                    reason: UnknownReason::Invalid,
                });
            }
        }
        Err(failure) => push_geometry_access_diagnostic(
            diagnostics,
            &unit.id,
            GeometryComponent::Horizontal,
            failure,
        ),
    }

    match claim_value(&unit.geometry.baseline, policy.geometry_confidence) {
        Ok(baseline) => {
            if let Some(scan_baseline_y) = project_baseline(mapping, baseline) {
                let delta = candidate_geometry.baseline_y - scan_baseline_y;
                matched.baseline_delta = Some(delta);
                matched.baseline_status = if delta.abs() <= policy.baseline_tolerance_pt {
                    EvidenceStatus::Preserved
                } else {
                    EvidenceStatus::Violated
                };
            } else {
                diagnostics.push(ScanComparisonDiagnostic::GeometryClaimUnknown {
                    scan_unit_id: unit.id.clone(),
                    component: GeometryComponent::Baseline,
                    reason: UnknownReason::Invalid,
                });
            }
        }
        Err(failure) => push_geometry_access_diagnostic(
            diagnostics,
            &unit.id,
            GeometryComponent::Baseline,
            failure,
        ),
    }

    matched
}

fn project_bbox(mapping: &PageMapping, bbox: &FramedBbox) -> Option<FramedBbox> {
    let points = [
        project_point(mapping, bbox.x0, bbox.y0)?,
        project_point(mapping, bbox.x1, bbox.y0)?,
        project_point(mapping, bbox.x0, bbox.y1)?,
        project_point(mapping, bbox.x1, bbox.y1)?,
    ];
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in points {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    Some(FramedBbox {
        frame_id: mapping.target_frame.id.clone(),
        x0: min_x,
        y0: min_y,
        x1: max_x,
        y1: max_y,
    })
}

fn project_baseline(mapping: &PageMapping, baseline: &FramedPolyline) -> Option<f64> {
    let mut ys = baseline
        .points
        .iter()
        .map(|&(x, y)| project_point(mapping, x, y).map(|(_, projected_y)| projected_y))
        .collect::<Option<Vec<_>>>()?;
    median(&mut ys)
}

fn project_point(mapping: &PageMapping, x: f64, y: f64) -> Option<(f64, f64)> {
    let matrix = &mapping.matrix;
    let denominator = matrix[6] * x + matrix[7] * y + matrix[8];
    if !denominator.is_finite() || denominator.abs() <= f64::EPSILON {
        return None;
    }
    let projected_x = (matrix[0] * x + matrix[1] * y + matrix[2]) / denominator;
    let projected_y = (matrix[3] * x + matrix[4] * y + matrix[5]) / denominator;
    if projected_x.is_finite() && projected_y.is_finite() {
        Some((projected_x, projected_y))
    } else {
        None
    }
}

fn build_candidate_lines(candidate: &DocumentGeometry) -> Vec<CandidateLine> {
    let mut ordered_indices: Vec<usize> = (0..candidate.glyphs.len()).collect();
    ordered_indices.sort_by(|left, right| {
        candidate.glyphs[*left]
            .position
            .1
            .total_cmp(&candidate.glyphs[*right].position.1)
            .then_with(|| {
                candidate.glyphs[*left]
                    .position
                    .0
                    .total_cmp(&candidate.glyphs[*right].position.0)
            })
            .then_with(|| left.cmp(right))
    });

    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for glyph_index in ordered_indices {
        let glyph = &candidate.glyphs[glyph_index];
        let starts_new_line = clusters.last().is_some_and(|cluster| {
            let previous = &candidate.glyphs[*cluster.last().expect("cluster is not empty")];
            (glyph.position.1 - previous.position.1).abs() > 0.5 * glyph.font_size_pt.abs()
        });
        if clusters.is_empty() || starts_new_line {
            clusters.push(vec![glyph_index]);
        } else if let Some(cluster) = clusters.last_mut() {
            cluster.push(glyph_index);
        }
    }

    clusters
        .into_iter()
        .enumerate()
        .map(|(line_index, mut glyph_indices)| {
            glyph_indices.sort_by(|left, right| {
                candidate.glyphs[*left]
                    .position
                    .0
                    .total_cmp(&candidate.glyphs[*right].position.0)
                    .then_with(|| left.cmp(right))
            });
            build_candidate_line(line_index, glyph_indices, candidate)
        })
        .collect()
}

fn build_candidate_line(
    index: usize,
    glyph_indices: Vec<usize>,
    candidate: &DocumentGeometry,
) -> CandidateLine {
    let geometry = candidate_geometry(&glyph_indices, candidate);
    let mut scalars = Vec::new();
    let mut scalar_glyphs = Vec::new();
    let mut text_known = true;
    for &glyph_index in &glyph_indices {
        let glyph = &candidate.glyphs[glyph_index];
        if !matches!(glyph.mapping_status, TextMappingStatus::Mapped) {
            text_known = false;
            continue;
        }
        let Some(codepoints) = &glyph.codepoints else {
            text_known = false;
            continue;
        };
        for &scalar in codepoints {
            scalars.push(scalar);
            scalar_glyphs.push(glyph_index);
        }
    }

    let (text, words) = if text_known {
        let text: String = scalars.iter().collect();
        let words = build_candidate_words(&scalars, &scalar_glyphs, candidate);
        (Some(text), words)
    } else {
        (
            None,
            vec![CandidateWord {
                scalar_range: (0, 0),
                text: None,
                geometry,
            }],
        )
    };

    CandidateLine {
        index,
        text,
        scalar_glyphs,
        geometry,
        words,
    }
}

fn build_candidate_words(
    scalars: &[char],
    scalar_glyphs: &[usize],
    candidate: &DocumentGeometry,
) -> Vec<CandidateWord> {
    let mut words = Vec::new();
    let mut start = 0;
    while start < scalars.len() {
        while start < scalars.len() && scalars[start].is_whitespace() {
            start += 1;
        }
        if start == scalars.len() {
            break;
        }
        let mut end = start;
        while end < scalars.len() && !scalars[end].is_whitespace() {
            end += 1;
        }
        let text: String = scalars[start..end].iter().collect();
        let glyph_set: HashSet<usize> = scalar_glyphs[start..end].iter().copied().collect();
        let clean_boundaries =
            scalar_glyphs
                .iter()
                .enumerate()
                .all(|(scalar_index, glyph_index)| {
                    !glyph_set.contains(glyph_index) || (start..end).contains(&scalar_index)
                });
        let mut glyph_indices: Vec<usize> = glyph_set.into_iter().collect();
        glyph_indices.sort_by(|left, right| {
            candidate.glyphs[*left]
                .position
                .0
                .total_cmp(&candidate.glyphs[*right].position.0)
                .then_with(|| left.cmp(right))
        });
        words.push(CandidateWord {
            scalar_range: (start, end),
            text: Some(text),
            geometry: clean_boundaries
                .then(|| candidate_geometry(&glyph_indices, candidate))
                .flatten(),
        });
        start = end;
    }
    words
}

fn candidate_geometry(
    glyph_indices: &[usize],
    candidate: &DocumentGeometry,
) -> Option<CandidateGeometry> {
    if glyph_indices.is_empty() {
        return None;
    }
    let mut x0 = f64::INFINITY;
    let mut x1 = f64::NEG_INFINITY;
    let mut baselines = Vec::with_capacity(glyph_indices.len());
    for &glyph_index in glyph_indices {
        let glyph = &candidate.glyphs[glyph_index];
        let start = glyph.position.0;
        let end = glyph.position.0 + glyph.advance;
        x0 = x0.min(start.min(end));
        x1 = x1.max(start.max(end));
        baselines.push(glyph.position.1);
    }
    Some(CandidateGeometry {
        x0,
        x1,
        baseline_y: median(&mut baselines)?,
    })
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        Some((values[middle - 1] + values[middle]) / 2.0)
    } else {
        Some(values[middle])
    }
}

fn associate_parent_lines(
    parent_ids: &HashSet<&str>,
    unit_by_id: &HashMap<&str, &ObservationUnit>,
    candidate_lines: &[CandidateLine],
    requirement: ConfidenceRequirement,
) -> HashMap<String, ParentAssociation> {
    let mut result = HashMap::new();
    let mut provisional = Vec::new();
    let mut ordered_parent_ids: Vec<&str> = parent_ids.iter().copied().collect();
    ordered_parent_ids.sort();

    for parent_id in ordered_parent_ids {
        let Some(parent) = unit_by_id.get(parent_id).copied() else {
            result.insert(
                parent_id.to_string(),
                ParentAssociation {
                    candidate_line: None,
                    failure: Some(LineAssociationFailure::Unmatched),
                },
            );
            continue;
        };
        match claim_value(&parent.text, requirement) {
            Ok(text) => {
                let candidates: Vec<usize> = candidate_lines
                    .iter()
                    .filter(|line| line.text.as_deref() == Some(text.as_str()))
                    .map(|line| line.index)
                    .collect();
                match candidates.as_slice() {
                    [line_index] => provisional.push((parent_id.to_string(), *line_index)),
                    [] => {
                        result.insert(
                            parent_id.to_string(),
                            ParentAssociation {
                                candidate_line: None,
                                failure: Some(LineAssociationFailure::Unmatched),
                            },
                        );
                    }
                    _ => {
                        result.insert(
                            parent_id.to_string(),
                            ParentAssociation {
                                candidate_line: None,
                                failure: Some(LineAssociationFailure::Ambiguous(candidates)),
                            },
                        );
                    }
                }
            }
            Err(failure) => {
                result.insert(
                    parent_id.to_string(),
                    ParentAssociation {
                        candidate_line: None,
                        failure: Some(LineAssociationFailure::Claim(failure)),
                    },
                );
            }
        }
    }

    let mut counts = HashMap::new();
    for (_, line_index) in &provisional {
        *counts.entry(*line_index).or_insert(0_usize) += 1;
    }
    for (parent_id, line_index) in provisional {
        if counts[&line_index] == 1 {
            result.insert(
                parent_id,
                ParentAssociation {
                    candidate_line: Some(line_index),
                    failure: None,
                },
            );
        } else {
            result.insert(
                parent_id,
                ParentAssociation {
                    candidate_line: None,
                    failure: Some(LineAssociationFailure::CandidateCollision(line_index)),
                },
            );
        }
    }
    result
}

fn detect_reflow(
    words: &[&ObservationUnit],
    associations: &HashMap<String, ParentAssociation>,
    candidate_lines: &[CandidateLine],
    requirement: ConfidenceRequirement,
    diagnostics: &mut Vec<ScanComparisonDiagnostic>,
) -> Vec<ReflowWitness> {
    let mut by_parent: HashMap<&str, Vec<&ObservationUnit>> = HashMap::new();
    for word in words {
        if let Some(parent_id) = word.parent_id.as_deref() {
            by_parent.entry(parent_id).or_default().push(*word);
        }
    }
    let mut parent_ids: Vec<&str> = by_parent.keys().copied().collect();
    parent_ids.sort();
    let mut witnesses = Vec::new();

    for parent_id in parent_ids {
        if associations
            .get(parent_id)
            .and_then(|association| association.candidate_line)
            .is_some()
        {
            continue;
        }
        let mut parent_words = by_parent.remove(parent_id).unwrap_or_default();
        parent_words.sort_by(scan_unit_order);
        if parent_words.is_empty() {
            continue;
        }
        let mut candidate_line_indices = Vec::new();
        let mut candidate_keys = HashSet::new();
        let mut all_unique = true;
        for word in &parent_words {
            let Ok(text) = claim_value(&word.text, requirement) else {
                all_unique = false;
                break;
            };
            let occurrences: Vec<(usize, usize, usize)> = candidate_lines
                .iter()
                .flat_map(|line| {
                    line.words.iter().filter_map(move |candidate_word| {
                        (candidate_word.text.as_deref() == Some(text.as_str())).then_some((
                            line.index,
                            candidate_word.scalar_range.0,
                            candidate_word.scalar_range.1,
                        ))
                    })
                })
                .collect();
            if occurrences.len() != 1 || !candidate_keys.insert(occurrences[0]) {
                all_unique = false;
                break;
            }
            candidate_line_indices.push(occurrences[0].0);
        }
        let in_order = candidate_line_indices
            .windows(2)
            .all(|pair| pair[0] <= pair[1]);
        let distinct_lines: HashSet<usize> = candidate_line_indices.iter().copied().collect();
        if all_unique && in_order && distinct_lines.len() >= 2 {
            diagnostics.push(ScanComparisonDiagnostic::ReflowDetected {
                scan_line_id: parent_id.to_string(),
                candidate_line_indices: candidate_line_indices.clone(),
            });
            witnesses.push(ReflowWitness {
                scan_line_id: parent_id.to_string(),
                candidate_line_indices,
                scan_word_ids: parent_words.iter().map(|word| word.id.clone()).collect(),
            });
        }
    }
    witnesses
}

fn sorted_scan_units(scan: &ScanObservation, kind: ObservationKind) -> Vec<&ObservationUnit> {
    let parent_order: HashMap<&str, u32> = scan
        .units
        .iter()
        .filter_map(|unit| {
            let Claim::Known { value, .. } = unit.reading_order else {
                return None;
            };
            Some((unit.id.as_str(), value))
        })
        .collect();
    let mut units: Vec<&ObservationUnit> =
        scan.units.iter().filter(|unit| unit.kind == kind).collect();
    units.sort_by(|left, right| {
        let left_parent_order = left
            .parent_id
            .as_deref()
            .and_then(|id| parent_order.get(id).copied())
            .unwrap_or(0);
        let right_parent_order = right
            .parent_id
            .as_deref()
            .and_then(|id| parent_order.get(id).copied())
            .unwrap_or(0);
        left_parent_order
            .cmp(&right_parent_order)
            .then_with(|| scan_unit_order(left, right))
    });
    units
}

fn scan_unit_order(left: &&ObservationUnit, right: &&ObservationUnit) -> std::cmp::Ordering {
    claim_order(&left.reading_order)
        .cmp(&claim_order(&right.reading_order))
        .then_with(|| left.id.cmp(&right.id))
}

fn claim_order(claim: &Claim<u32>) -> (bool, u32) {
    match claim {
        Claim::Known { value, .. } => (false, *value),
        Claim::Unknown { .. } => (true, u32::MAX),
    }
}

fn claim_value<T>(
    claim: &Claim<T>,
    requirement: ConfidenceRequirement,
) -> Result<&T, AccessFailure> {
    match claim {
        Claim::Unknown { reason, .. } => Err(AccessFailure::Unknown(*reason)),
        Claim::Known {
            value, confidence, ..
        } => match requirement {
            ConfidenceRequirement::Any => Ok(value),
            ConfidenceRequirement::KnownAtLeast(threshold) => match confidence {
                Confidence::Known {
                    value: confidence, ..
                } if *confidence >= threshold => Ok(value),
                _ => Err(AccessFailure::BelowPolicy),
            },
        },
    }
}

fn push_text_access_diagnostic(
    diagnostics: &mut Vec<ScanComparisonDiagnostic>,
    scan_unit_id: &str,
    failure: AccessFailure,
) {
    match failure {
        AccessFailure::Unknown(reason) => {
            diagnostics.push(ScanComparisonDiagnostic::TextClaimUnknown {
                scan_unit_id: scan_unit_id.to_string(),
                reason,
            })
        }
        AccessFailure::BelowPolicy => {
            diagnostics.push(ScanComparisonDiagnostic::TextConfidenceBelowPolicy {
                scan_unit_id: scan_unit_id.to_string(),
            })
        }
    }
}

fn push_parent_failure_diagnostic(
    diagnostics: &mut Vec<ScanComparisonDiagnostic>,
    scan_unit_id: &str,
    failure: Option<&LineAssociationFailure>,
) {
    match failure {
        Some(LineAssociationFailure::Claim(failure)) => {
            push_text_access_diagnostic(diagnostics, scan_unit_id, failure.clone())
        }
        Some(LineAssociationFailure::Ambiguous(indices)) => {
            diagnostics.push(ScanComparisonDiagnostic::AmbiguousTextMatch {
                scan_unit_id: scan_unit_id.to_string(),
                candidate_line_indices: indices.clone(),
            })
        }
        Some(LineAssociationFailure::CandidateCollision(index)) => {
            diagnostics.push(ScanComparisonDiagnostic::AmbiguousTextMatch {
                scan_unit_id: scan_unit_id.to_string(),
                candidate_line_indices: vec![*index],
            })
        }
        _ => diagnostics.push(ScanComparisonDiagnostic::UnmatchedText {
            scan_unit_id: scan_unit_id.to_string(),
        }),
    }
}

fn push_geometry_access_diagnostic(
    diagnostics: &mut Vec<ScanComparisonDiagnostic>,
    scan_unit_id: &str,
    component: GeometryComponent,
    failure: AccessFailure,
) {
    match failure {
        AccessFailure::Unknown(reason) => {
            diagnostics.push(ScanComparisonDiagnostic::GeometryClaimUnknown {
                scan_unit_id: scan_unit_id.to_string(),
                component,
                reason,
            })
        }
        AccessFailure::BelowPolicy => {
            diagnostics.push(ScanComparisonDiagnostic::GeometryConfidenceBelowPolicy {
                scan_unit_id: scan_unit_id.to_string(),
                component,
            })
        }
    }
}

fn candidate_text_diagnostics(candidate_lines: &[CandidateLine]) -> Vec<ScanComparisonDiagnostic> {
    candidate_lines
        .iter()
        .filter(|line| line.text.is_none())
        .map(|line| ScanComparisonDiagnostic::CandidateUnmappedGlyph {
            candidate_line_index: line.index,
        })
        .collect()
}

fn collision_counts(
    keys: impl Iterator<Item = (usize, usize, usize)>,
) -> HashMap<(usize, usize, usize), usize> {
    let mut counts = HashMap::new();
    for key in keys {
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

fn line_reference(line: &CandidateLine) -> CandidateUnitRef {
    CandidateUnitRef {
        candidate_line_index: line.index,
        candidate_scalar_range: (0, line.scalar_glyphs.len()),
    }
}

fn candidate_word_references(candidate_lines: &[CandidateLine]) -> Vec<CandidateUnitRef> {
    candidate_lines
        .iter()
        .flat_map(|line| {
            line.words.iter().map(move |word| CandidateUnitRef {
                candidate_line_index: line.index,
                candidate_scalar_range: word.scalar_range,
            })
        })
        .collect()
}

fn invalid_policy_field(policy: &ScanComparisonPolicy) -> Option<&'static str> {
    if !policy.horizontal_tolerance_pt.is_finite() || policy.horizontal_tolerance_pt < 0.0 {
        return Some("horizontal_tolerance_pt");
    }
    if !policy.baseline_tolerance_pt.is_finite() || policy.baseline_tolerance_pt < 0.0 {
        return Some("baseline_tolerance_pt");
    }
    for (field, requirement) in [
        ("text_confidence", policy.text_confidence),
        ("geometry_confidence", policy.geometry_confidence),
    ] {
        if let ConfidenceRequirement::KnownAtLeast(value) = requirement {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Some(field);
            }
        }
    }
    None
}

fn invalid_policy_report(
    scan: &ScanObservation,
    candidate_lines: &[CandidateLine],
    granularity: ScanGranularity,
    field: &'static str,
) -> ScanComparisonReport {
    let kind = match granularity {
        ScanGranularity::Line => ObservationKind::Line,
        ScanGranularity::Word => ObservationKind::Word,
    };
    let unmatched_scan = sorted_scan_units(scan, kind)
        .into_iter()
        .map(|unit| unit.id.clone())
        .collect::<Vec<_>>();
    let unmatched_candidate = match granularity {
        ScanGranularity::Line => candidate_lines.iter().map(line_reference).collect(),
        ScanGranularity::Word => candidate_word_references(candidate_lines),
    };
    ScanComparisonReport {
        content_status: EvidenceStatus::Unknown,
        geometry_status: EvidenceStatus::Unknown,
        overall_status: EvidenceStatus::Unknown,
        coverage: ScanCoverage {
            matched_scan: 0,
            total_scan: unmatched_scan.len(),
            matched_candidate: 0,
            total_candidate: unmatched_candidate.len(),
        },
        matches: Vec::new(),
        unmatched_scan,
        unmatched_candidate,
        reflow: Vec::new(),
        diagnostics: vec![ScanComparisonDiagnostic::InvalidPolicy { field }],
    }
}
