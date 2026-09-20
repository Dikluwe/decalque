//! Verificação estrutural pura de fonte, sem I/O, JSON ou Typst.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::content::{FontModelDiagnostic, TextInterpreterDiagnostic};
use crate::entities::{
    DocumentGeometry, FontStyleHypothesis, FontWeightHypothesis, TextMappingStatus,
    TypographyHypothesis,
};

use super::EvidenceStatus;

/// Entrada estrutural de um recurso `/Font` do PDF candidato.
///
/// O valor bruto de `/BaseFont` permanece intacto. A normalizacao existe
/// apenas na evidencia produzida por [`attest_scan_font`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfFontResource {
    pub resource_name: String,
    pub base_font: Option<String>,
}

/// Faces reconhecidas pela gramatica nominal fechada da atestacao v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanFontFace {
    Regular,
    Bold,
    Italic,
    BoldItalic,
    Oblique,
    BoldOblique,
}

/// Evidencia agregada para um recurso efetivamente usado por ao menos um
/// glifo. Recursos do catalogo que nao foram usados nao aparecem aqui.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanFontAttestationResource {
    pub resource_name: String,
    pub raw_base_font: Option<String>,
    pub normalized_stem: Option<String>,
    pub face: Option<ScanFontFace>,
    pub glyph_count: usize,
}

/// Testemunhas publicas da atestacao estrutural.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanFontAttestationDiagnostic {
    TextSequenceMismatch {
        expected_scalar_count: usize,
        candidate_scalar_count: usize,
        first_differing_scalar_index: usize,
    },
    UnmappedGlyph {
        glyph_index: usize,
    },
    EmptyResourceName,
    MissingFontResource {
        resource_name: String,
        first_glyph_index: usize,
    },
    DuplicateFontResource {
        resource_name: String,
        occurrences: usize,
    },
    MissingBaseFont {
        resource_name: String,
    },
    EmptyBaseFont {
        resource_name: String,
    },
    UnclassifiableBaseFont {
        resource_name: String,
        raw_base_font: String,
    },
    UnclassifiableRequestedFamily {
        raw_family: String,
    },
    MissingRequiredFace {
        resource_name: String,
        expected: ScanFontFace,
    },
    FaceMismatch {
        resource_name: String,
        expected: ScanFontFace,
        actual: ScanFontFace,
    },
    FamilyMismatch {
        resource_name: String,
        expected_normalized_family: String,
        actual_normalized_stem: String,
    },
    OpaqueFontModel {
        diagnostic: FontModelDiagnostic,
    },
    OpaqueTextInterpretation {
        diagnostic: TextInterpreterDiagnostic,
    },
}

/// Entradas puras necessarias para atestar uma unica hipotese tipografica.
#[derive(Debug, Clone, Copy)]
pub struct ScanFontAttestationInput<'a> {
    pub expected_line_texts: &'a [String],
    pub typography: &'a TypographyHypothesis,
    pub candidate: &'a DocumentGeometry,
    pub font_resources: &'a [PdfFontResource],
    pub font_model_diagnostics: &'a [FontModelDiagnostic],
    pub text_interpreter_diagnostics: &'a [TextInterpreterDiagnostic],
}

/// Resultado deterministico da atestacao estrutural de fonte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanFontAttestationReport {
    pub status: EvidenceStatus,
    pub expected_scalar_count: usize,
    pub candidate_scalar_count: Option<usize>,
    pub diagnostics: Vec<ScanFontAttestationDiagnostic>,
    pub used_resources: Vec<ScanFontAttestationResource>,
}

#[derive(Debug, Clone, Copy)]
struct ResourceUsage {
    glyph_count: usize,
    first_glyph_index: usize,
}

#[derive(Debug)]
struct ClassifiedBaseFont {
    normalized_stem: String,
    declared_face: Option<ScanFontFace>,
}

const FACE_SUFFIXES: [(&str, ScanFontFace); 6] = [
    ("BoldItalic", ScanFontFace::BoldItalic),
    ("BoldOblique", ScanFontFace::BoldOblique),
    ("Regular", ScanFontFace::Regular),
    ("Bold", ScanFontFace::Bold),
    ("Italic", ScanFontFace::Italic),
    ("Oblique", ScanFontFace::Oblique),
];

/// Atesta cobertura Unicode e todos os recursos de fonte usados pelo PDF.
///
/// A funcao nao consulta o sistema, nao resolve aliases e nao tenta inferir
/// nomes opacos. Incompatibilidades observaveis vencem lacunas de evidencia;
/// sem incompatibilidade, qualquer lacuna mantem o resultado `Unknown`.
pub fn attest_scan_font(input: &ScanFontAttestationInput<'_>) -> ScanFontAttestationReport {
    let expected_scalars = input
        .expected_line_texts
        .iter()
        .flat_map(|line| line.chars())
        .collect::<Vec<_>>();
    let expected_scalar_count = expected_scalars.len();

    let mut diagnostics = Vec::new();
    let candidate_scalars = collect_candidate_scalars(input.candidate, &mut diagnostics);
    let candidate_scalar_count = candidate_scalars.as_ref().map(Vec::len);

    if let Some(candidate_scalars) = candidate_scalars.as_ref() {
        if candidate_scalars != &expected_scalars {
            diagnostics.push(ScanFontAttestationDiagnostic::TextSequenceMismatch {
                expected_scalar_count,
                candidate_scalar_count: candidate_scalars.len(),
                first_differing_scalar_index: first_differing_scalar_index(
                    &expected_scalars,
                    candidate_scalars,
                ),
            });
        }
    }

    append_opaque_diagnostics(input, &mut diagnostics);

    let normalized_requested_family = normalize_family(&input.typography.font_family);
    if normalized_requested_family.is_none() {
        diagnostics.push(
            ScanFontAttestationDiagnostic::UnclassifiableRequestedFamily {
                raw_family: input.typography.font_family.clone(),
            },
        );
    }

    let expected_face = expected_face(input.typography);
    let used_resources = audit_used_resources(
        input,
        normalized_requested_family.as_deref(),
        expected_face,
        &mut diagnostics,
    );

    canonicalize_diagnostics(&mut diagnostics);
    let status = if diagnostics.iter().any(is_violation) {
        EvidenceStatus::Violated
    } else if diagnostics.is_empty() {
        EvidenceStatus::Preserved
    } else {
        EvidenceStatus::Unknown
    };

    ScanFontAttestationReport {
        status,
        expected_scalar_count,
        candidate_scalar_count,
        diagnostics,
        used_resources,
    }
}

fn collect_candidate_scalars(
    candidate: &DocumentGeometry,
    diagnostics: &mut Vec<ScanFontAttestationDiagnostic>,
) -> Option<Vec<char>> {
    let mut scalars = Vec::new();
    let mut complete = true;

    for (glyph_index, glyph) in candidate.glyphs.iter().enumerate() {
        match (&glyph.codepoints, glyph.mapping_status) {
            (Some(codepoints), TextMappingStatus::Mapped) => {
                scalars.extend(codepoints.iter().copied());
            }
            _ => {
                complete = false;
                diagnostics.push(ScanFontAttestationDiagnostic::UnmappedGlyph { glyph_index });
            }
        }
    }

    complete.then_some(scalars)
}

fn first_differing_scalar_index(expected: &[char], candidate: &[char]) -> usize {
    expected
        .iter()
        .zip(candidate)
        .position(|(expected, candidate)| expected != candidate)
        .unwrap_or_else(|| expected.len().min(candidate.len()))
}

fn append_opaque_diagnostics(
    input: &ScanFontAttestationInput<'_>,
    diagnostics: &mut Vec<ScanFontAttestationDiagnostic>,
) {
    diagnostics.extend(
        input
            .font_model_diagnostics
            .iter()
            .copied()
            .filter(|diagnostic| *diagnostic != FontModelDiagnostic::NoWidths)
            .map(|diagnostic| ScanFontAttestationDiagnostic::OpaqueFontModel { diagnostic }),
    );
    diagnostics.extend(
        input
            .text_interpreter_diagnostics
            .iter()
            .copied()
            .map(
                |diagnostic| ScanFontAttestationDiagnostic::OpaqueTextInterpretation { diagnostic },
            ),
    );
}

fn audit_used_resources(
    input: &ScanFontAttestationInput<'_>,
    normalized_requested_family: Option<&str>,
    expected_face: ScanFontFace,
    diagnostics: &mut Vec<ScanFontAttestationDiagnostic>,
) -> Vec<ScanFontAttestationResource> {
    let mut usage_by_name = BTreeMap::<String, ResourceUsage>::new();
    for (glyph_index, glyph) in input.candidate.glyphs.iter().enumerate() {
        usage_by_name
            .entry(glyph.font_ref.clone())
            .and_modify(|usage| usage.glyph_count += 1)
            .or_insert(ResourceUsage {
                glyph_count: 1,
                first_glyph_index: glyph_index,
            });
    }

    usage_by_name
        .into_iter()
        .map(|(resource_name, usage)| {
            audit_resource(
                &resource_name,
                usage,
                input.font_resources,
                normalized_requested_family,
                expected_face,
                diagnostics,
            )
        })
        .collect()
}

fn audit_resource(
    resource_name: &str,
    usage: ResourceUsage,
    catalog: &[PdfFontResource],
    normalized_requested_family: Option<&str>,
    expected_face: ScanFontFace,
    diagnostics: &mut Vec<ScanFontAttestationDiagnostic>,
) -> ScanFontAttestationResource {
    if resource_name.is_empty() {
        diagnostics.push(ScanFontAttestationDiagnostic::EmptyResourceName);
    }

    let matches = catalog
        .iter()
        .filter(|resource| resource.resource_name == resource_name)
        .collect::<Vec<_>>();

    if matches.is_empty() {
        diagnostics.push(ScanFontAttestationDiagnostic::MissingFontResource {
            resource_name: resource_name.to_string(),
            first_glyph_index: usage.first_glyph_index,
        });
        return unresolved_resource(resource_name, usage.glyph_count);
    }

    if matches.len() > 1 {
        diagnostics.push(ScanFontAttestationDiagnostic::DuplicateFontResource {
            resource_name: resource_name.to_string(),
            occurrences: matches.len(),
        });
        return unresolved_resource(resource_name, usage.glyph_count);
    }

    let raw_base_font = matches[0].base_font.clone();
    let mut evidence = ScanFontAttestationResource {
        resource_name: resource_name.to_string(),
        raw_base_font: raw_base_font.clone(),
        normalized_stem: None,
        face: None,
        glyph_count: usage.glyph_count,
    };

    // Um identificador vazio nao resolve um recurso de modo estruturalmente
    // confiavel, mesmo que exista uma entrada vazia no catalogo.
    if resource_name.is_empty() {
        return evidence;
    }

    let Some(raw_base_font) = raw_base_font else {
        diagnostics.push(ScanFontAttestationDiagnostic::MissingBaseFont {
            resource_name: resource_name.to_string(),
        });
        return evidence;
    };

    if raw_base_font.is_empty() {
        diagnostics.push(ScanFontAttestationDiagnostic::EmptyBaseFont {
            resource_name: resource_name.to_string(),
        });
        return evidence;
    }

    let Some(classified) = classify_base_font(&raw_base_font, normalized_requested_family) else {
        diagnostics.push(ScanFontAttestationDiagnostic::UnclassifiableBaseFont {
            resource_name: resource_name.to_string(),
            raw_base_font,
        });
        return evidence;
    };

    evidence.normalized_stem = Some(classified.normalized_stem.clone());
    evidence.face = classified.declared_face;

    match classified.declared_face {
        Some(actual) if actual != expected_face => {
            diagnostics.push(ScanFontAttestationDiagnostic::FaceMismatch {
                resource_name: resource_name.to_string(),
                expected: expected_face,
                actual,
            });
        }
        None if expected_face != ScanFontFace::Regular => {
            diagnostics.push(ScanFontAttestationDiagnostic::MissingRequiredFace {
                resource_name: resource_name.to_string(),
                expected: expected_face,
            });
        }
        None => evidence.face = Some(ScanFontFace::Regular),
        Some(_) => {}
    }

    if let Some(expected_normalized_family) = normalized_requested_family {
        if classified.normalized_stem != expected_normalized_family {
            diagnostics.push(ScanFontAttestationDiagnostic::FamilyMismatch {
                resource_name: resource_name.to_string(),
                expected_normalized_family: expected_normalized_family.to_string(),
                actual_normalized_stem: classified.normalized_stem,
            });
        }
    }

    evidence
}

fn unresolved_resource(resource_name: &str, glyph_count: usize) -> ScanFontAttestationResource {
    ScanFontAttestationResource {
        resource_name: resource_name.to_string(),
        raw_base_font: None,
        normalized_stem: None,
        face: None,
        glyph_count,
    }
}

fn classify_base_font(
    raw_base_font: &str,
    normalized_requested_family: Option<&str>,
) -> Option<ClassifiedBaseFont> {
    if !raw_base_font.is_ascii() {
        return None;
    }

    let mut structural_name = strip_subset_prefix(raw_base_font);
    if structural_name.contains('+') || structural_name.is_empty() {
        return None;
    }

    structural_name = strip_identity_suffix(structural_name);
    if structural_name.is_empty() || contains_identity_token(structural_name) {
        return None;
    }

    let (stem, declared_face) = strip_face_suffix(structural_name);
    if stem.is_empty() || contains_face_token(stem) {
        return None;
    }

    let normalized_stem = normalize_family(stem)?;

    // Quando o prefixo coincide exatamente com a familia solicitada, um
    // componente terminal separado por hifen que nao pertence ao conjunto
    // fechado de faces e um sufixo opaco, nao uma familia diferente.
    if declared_face.is_none()
        && has_unclassifiable_suffix_for_requested_family(
            stem,
            &normalized_stem,
            normalized_requested_family,
        )
    {
        return None;
    }

    Some(ClassifiedBaseFont {
        normalized_stem,
        declared_face,
    })
}

fn strip_subset_prefix(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 7 && bytes[..6].iter().all(u8::is_ascii_uppercase) && bytes[6] == b'+' {
        &value[7..]
    } else {
        value
    }
}

fn strip_identity_suffix(value: &str) -> &str {
    value
        .strip_suffix("-Identity-H")
        .or_else(|| value.strip_suffix("-Identity-V"))
        .unwrap_or(value)
}

fn strip_face_suffix(value: &str) -> (&str, Option<ScanFontFace>) {
    for (suffix, face) in FACE_SUFFIXES {
        let mut marked_suffix = String::with_capacity(suffix.len() + 1);
        marked_suffix.push('-');
        marked_suffix.push_str(suffix);
        if let Some(stem) = value.strip_suffix(&marked_suffix) {
            return (stem, Some(face));
        }
    }

    (value, None)
}

fn contains_identity_token(value: &str) -> bool {
    let components = value.split('-').collect::<Vec<_>>();
    components.windows(2).any(|window| {
        window[0].eq_ignore_ascii_case("Identity")
            && (window[1].eq_ignore_ascii_case("H") || window[1].eq_ignore_ascii_case("V"))
    })
}

fn contains_face_token(value: &str) -> bool {
    value.split('-').any(|component| {
        FACE_SUFFIXES
            .iter()
            .any(|(suffix, _)| component.eq_ignore_ascii_case(suffix))
    })
}

fn has_unclassifiable_suffix_for_requested_family(
    stem: &str,
    normalized_stem: &str,
    normalized_requested_family: Option<&str>,
) -> bool {
    let Some(expected) = normalized_requested_family else {
        return false;
    };
    if normalized_stem == expected {
        return false;
    }
    let Some((prefix, suffix)) = stem.rsplit_once('-') else {
        return false;
    };
    !suffix.is_empty() && normalize_family(prefix).as_deref() == Some(expected)
}

fn normalize_family(value: &str) -> Option<String> {
    let mut normalized = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() {
            normalized.push((byte as char).to_ascii_lowercase());
        } else if matches!(byte, b' ' | b'-' | b'_') {
            continue;
        } else {
            return None;
        }
    }

    (!normalized.is_empty()).then_some(normalized)
}

fn expected_face(typography: &TypographyHypothesis) -> ScanFontFace {
    match (typography.weight, typography.style) {
        (FontWeightHypothesis::Regular, FontStyleHypothesis::Normal) => ScanFontFace::Regular,
        (FontWeightHypothesis::Bold, FontStyleHypothesis::Normal) => ScanFontFace::Bold,
        (FontWeightHypothesis::Regular, FontStyleHypothesis::Italic) => ScanFontFace::Italic,
        (FontWeightHypothesis::Bold, FontStyleHypothesis::Italic) => ScanFontFace::BoldItalic,
        (FontWeightHypothesis::Regular, FontStyleHypothesis::Oblique) => ScanFontFace::Oblique,
        (FontWeightHypothesis::Bold, FontStyleHypothesis::Oblique) => ScanFontFace::BoldOblique,
    }
}

fn is_violation(diagnostic: &ScanFontAttestationDiagnostic) -> bool {
    matches!(
        diagnostic,
        ScanFontAttestationDiagnostic::TextSequenceMismatch { .. }
            | ScanFontAttestationDiagnostic::FaceMismatch { .. }
            | ScanFontAttestationDiagnostic::FamilyMismatch { .. }
    )
}

fn canonicalize_diagnostics(diagnostics: &mut Vec<ScanFontAttestationDiagnostic>) {
    diagnostics.sort_by(compare_diagnostics);
    diagnostics.dedup();
}

fn compare_diagnostics(
    left: &ScanFontAttestationDiagnostic,
    right: &ScanFontAttestationDiagnostic,
) -> Ordering {
    diagnostic_code(left)
        .cmp(diagnostic_code(right))
        .then_with(|| diagnostic_resource(left).cmp(diagnostic_resource(right)))
        .then_with(|| diagnostic_position(left).cmp(&diagnostic_position(right)))
        .then_with(|| format!("{left:?}").cmp(&format!("{right:?}")))
}

fn diagnostic_code(diagnostic: &ScanFontAttestationDiagnostic) -> &'static str {
    match diagnostic {
        ScanFontAttestationDiagnostic::DuplicateFontResource { .. } => "duplicate_font_resource",
        ScanFontAttestationDiagnostic::EmptyBaseFont { .. } => "empty_base_font",
        ScanFontAttestationDiagnostic::EmptyResourceName => "empty_resource_name",
        ScanFontAttestationDiagnostic::FaceMismatch { .. } => "face_mismatch",
        ScanFontAttestationDiagnostic::FamilyMismatch { .. } => "family_mismatch",
        ScanFontAttestationDiagnostic::MissingBaseFont { .. } => "missing_base_font",
        ScanFontAttestationDiagnostic::MissingFontResource { .. } => "missing_font_resource",
        ScanFontAttestationDiagnostic::MissingRequiredFace { .. } => "missing_required_face",
        ScanFontAttestationDiagnostic::OpaqueFontModel { .. } => "opaque_font_model",
        ScanFontAttestationDiagnostic::OpaqueTextInterpretation { .. } => {
            "opaque_text_interpretation"
        }
        ScanFontAttestationDiagnostic::TextSequenceMismatch { .. } => "text_sequence_mismatch",
        ScanFontAttestationDiagnostic::UnclassifiableBaseFont { .. } => "unclassifiable_base_font",
        ScanFontAttestationDiagnostic::UnclassifiableRequestedFamily { .. } => {
            "unclassifiable_requested_family"
        }
        ScanFontAttestationDiagnostic::UnmappedGlyph { .. } => "unmapped_glyph",
    }
}

fn diagnostic_resource(diagnostic: &ScanFontAttestationDiagnostic) -> &str {
    match diagnostic {
        ScanFontAttestationDiagnostic::MissingFontResource { resource_name, .. }
        | ScanFontAttestationDiagnostic::DuplicateFontResource { resource_name, .. }
        | ScanFontAttestationDiagnostic::MissingBaseFont { resource_name }
        | ScanFontAttestationDiagnostic::EmptyBaseFont { resource_name }
        | ScanFontAttestationDiagnostic::UnclassifiableBaseFont { resource_name, .. }
        | ScanFontAttestationDiagnostic::MissingRequiredFace { resource_name, .. }
        | ScanFontAttestationDiagnostic::FaceMismatch { resource_name, .. }
        | ScanFontAttestationDiagnostic::FamilyMismatch { resource_name, .. } => resource_name,
        _ => "",
    }
}

fn diagnostic_position(diagnostic: &ScanFontAttestationDiagnostic) -> usize {
    match diagnostic {
        ScanFontAttestationDiagnostic::TextSequenceMismatch {
            first_differing_scalar_index,
            ..
        } => *first_differing_scalar_index,
        ScanFontAttestationDiagnostic::UnmappedGlyph { glyph_index } => *glyph_index,
        ScanFontAttestationDiagnostic::MissingFontResource {
            first_glyph_index, ..
        } => *first_glyph_index,
        _ => 0,
    }
}
