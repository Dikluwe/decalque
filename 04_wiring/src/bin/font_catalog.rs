//! Exporta a evidência tipográfica estrutural de uma página candidata.
//! @prompt 00_nucleo/prompts/candidate-font-evidence.md

use decalque::materialize_page;
use decalque_core::entities::{display_size, PageRotation};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c < ' ' => write!(output, "\\u{:04x}", c as u32).unwrap(),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

fn fail(message: &str) -> ! {
    eprintln!("erro: {message}");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().unwrap_or_else(|| {
        fail("uso: decalque-font-catalog <candidato.pdf> [--page <indice>]")
    });
    let mut page_index = 0usize;
    if let Some(option) = args.next() {
        if option != "--page" {
            fail(&format!("opção desconhecida: {}", option.to_string_lossy()));
        }
        page_index = args
            .next()
            .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
            .unwrap_or_else(|| fail("--page exige índice inteiro não negativo"));
    }
    if args.next().is_some() {
        fail("argumentos excedentes");
    }

    let source = decalque_infra::load_page_source(Path::new(&path), page_index)
        .unwrap_or_else(|error| fail(&format!("candidato: {error}")));
    let font_names: HashMap<_, _> = source
        .fonts
        .iter()
        .map(|font| (font.resource_name.clone(), font.base_font.clone()))
        .collect();
    let materialized = materialize_page(source);
    let (width_pt, height_pt) = display_size(&materialized.geometry.page);
    let rotation = match materialized.geometry.page.rotation {
        PageRotation::Deg0 => 0,
        PageRotation::Deg90 => 90,
        PageRotation::Deg180 => 180,
        PageRotation::Deg270 => 270,
    };

    print!(
        "{{\"schema_version\":2,\"page_index\":{page_index},\"page\":{{\"width_pt\":{width_pt},\"height_pt\":{height_pt},\"rotation\":{rotation}}},\"glyphs\":["
    );
    for (index, glyph) in materialized.geometry.glyphs.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        let text = glyph
            .codepoints
            .as_ref()
            .map(|points| points.iter().collect::<String>());
        let base_font = font_names.get(&glyph.font_ref).and_then(|name| name.as_deref());
        print!(
            "{{\"text\":{},\"font_ref\":{},\"base_font\":{},\"font_size_pt\":{},\"position\":[{},{}]}}",
            text.as_deref().map(json_string).unwrap_or_else(|| "null".to_string()),
            json_string(&glyph.font_ref),
            base_font.map(json_string).unwrap_or_else(|| "null".to_string()),
            glyph.font_size_pt,
            glyph.position.0,
            glyph.position.1,
        );
    }
    println!("]}}");
}
