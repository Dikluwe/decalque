//! Emissao Typst segura e deterministica para planos de reconstrucao prontos.

use decalque_core::{FontStyleHypothesis, FontWeightHypothesis, ReconstructionPlan};
use std::fmt::Write as _;

/// Serializa um plano completo como fonte Typst, sem consultar o host nem
/// executar o compilador.
pub fn render_typst_source(plan: &ReconstructionPlan) -> Result<String, String> {
    render_typst_source_with_fallback_policy(plan, false)
}

/// Serializa um plano completo como fonte Typst com fallback de fontes
/// explicitamente proibido.
pub fn render_strict_typst_source(plan: &ReconstructionPlan) -> Result<String, String> {
    render_typst_source_with_fallback_policy(plan, true)
}

fn render_typst_source_with_fallback_policy(
    plan: &ReconstructionPlan,
    forbid_fallback: bool,
) -> Result<String, String> {
    validate_plan(plan)?;

    let page_width = canonical_number(plan.page.extent.0, "page.extent.0")?;
    let page_height = canonical_number(plan.page.extent.1, "page.extent.1")?;
    let font_size = canonical_number(plan.typography.size_pt, "typography.size_pt")?;
    let tracking = canonical_number(plan.typography.tracking_pt, "typography.tracking_pt")?;
    let weight = match plan.typography.weight {
        FontWeightHypothesis::Regular => "regular",
        FontWeightHypothesis::Bold => "bold",
    };
    let style = match plan.typography.style {
        FontStyleHypothesis::Normal => "normal",
        FontStyleHypothesis::Italic => "italic",
        FontStyleHypothesis::Oblique => "oblique",
    };

    let mut source = String::new();
    write!(
        source,
        "#set page(width: {page_width}pt, height: {page_height}pt, margin: 0pt)\n\
         #set text(font: \""
    )
    .expect("writing to a String cannot fail");
    push_typst_string_contents(&mut source, &plan.typography.font_family);
    let fallback_policy = if forbid_fallback {
        "fallback: false, "
    } else {
        ""
    };
    write!(
        source,
        "\", size: {font_size}pt, weight: \"{weight}\", style: \"{style}\", \
         tracking: {tracking}pt, {fallback_policy}hyphenate: false, top-edge: \"bounds\", \
         bottom-edge: \"bounds\")\n\n"
    )
    .expect("writing to a String cannot fail");
    source.push_str(
        "#let physical-line(body) = context {\n  let natural-size = measure(body)\n  \
         box(width: natural-size.width, body)\n}\n\n",
    );

    for line in &plan.lines {
        let dx = canonical_number(line.target_bbox.x0, "line.target_bbox.x0")?;
        let dy = canonical_number(line.target_bbox.y0, "line.target_bbox.y0")?;
        write!(
            source,
            "#place(top + left, dx: {dx}pt, dy: {dy}pt)[\n  #physical-line(text(\""
        )
        .expect("writing to a String cannot fail");
        push_typst_string_contents(&mut source, &line.text);
        source.push_str("\"))\n]\n");
    }

    Ok(source)
}

fn validate_plan(plan: &ReconstructionPlan) -> Result<(), String> {
    let (page_width, page_height) = plan.page.extent;
    if !page_width.is_finite() || page_width <= 0.0 {
        return Err("page width must be finite and strictly positive".to_string());
    }
    if !page_height.is_finite() || page_height <= 0.0 {
        return Err("page height must be finite and strictly positive".to_string());
    }
    if plan.typography.font_family.trim().is_empty() {
        return Err("font family must not be empty".to_string());
    }
    if !plan.typography.size_pt.is_finite() || plan.typography.size_pt <= 0.0 {
        return Err("font size must be finite and strictly positive".to_string());
    }
    if !plan.typography.tracking_pt.is_finite() {
        return Err("tracking must be finite".to_string());
    }
    if !plan.mapping_max_error_pt.is_finite() || plan.mapping_max_error_pt < 0.0 {
        return Err("mapping maximum error must be finite and non-negative".to_string());
    }
    if plan.lines.is_empty() {
        return Err("a reconstruction plan must contain at least one line".to_string());
    }
    if plan.coverage.planned_lines != plan.lines.len()
        || plan.coverage.total_lines != plan.lines.len()
    {
        return Err("a reconstruction plan must have complete line coverage".to_string());
    }

    for line in &plan.lines {
        if line.source_unit_id.is_empty() {
            return Err("a reconstruction line must have a source unit id".to_string());
        }
        if line.reading_key.is_empty() {
            return Err(format!(
                "reconstruction line `{}` must have a reading key",
                line.source_unit_id
            ));
        }
        if line.text.is_empty() {
            return Err(format!(
                "reconstruction line `{}` must have non-empty text",
                line.source_unit_id
            ));
        }
        if let Some(separator) = line.text.chars().find(|character| {
            matches!(
                character,
                '\n' | '\u{000b}' | '\u{000c}' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}'
            )
        }) {
            return Err(format!(
                "reconstruction line `{}` contains explicit physical line separator U+{:04X}",
                line.source_unit_id,
                u32::from(separator)
            ));
        }
        if line.target_bbox.frame_id != plan.page.id {
            return Err(format!(
                "reconstruction line `{}` is not in the page frame",
                line.source_unit_id
            ));
        }

        let bbox = &line.target_bbox;
        if ![bbox.x0, bbox.y0, bbox.x1, bbox.y1]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(format!(
                "reconstruction line `{}` has a non-finite target bbox",
                line.source_unit_id
            ));
        }
        if bbox.x0 >= bbox.x1 || bbox.y0 >= bbox.y1 {
            return Err(format!(
                "reconstruction line `{}` has a degenerate target bbox",
                line.source_unit_id
            ));
        }
    }

    Ok(())
}

fn canonical_number(value: f64, field: &str) -> Result<String, String> {
    if !value.is_finite() {
        return Err(format!("{field} must be finite"));
    }
    if value == 0.0 {
        return Ok("0".to_string());
    }
    Ok(value.to_string())
}

fn push_typst_string_contents(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                write!(output, "\\u{{{:x}}}", u32::from(character))
                    .expect("writing to a String cannot fail");
            }
            character => output.push(character),
        }
    }
}
