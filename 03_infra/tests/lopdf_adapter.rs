use decalque_core::entities::Rect;
use decalque_core::{ContentOperation, PdfError, RawFontSubtype, XObjectSubtype};
use decalque_infra::load_page_source;
use lopdf::{dictionary, Document, Object, Stream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn loads_typst_page_with_text_fonts_and_tounicode() {
    let source = load_page_source(&fixture("typst.pdf"), 0).expect("Typst deve abrir");
    assert!(source.box_model.media_box.x1 > source.box_model.media_box.x0);
    assert!(source
        .operations
        .iter()
        .any(|operation| matches!(operation, ContentOperation::BeginText)));
    assert!(source
        .operations
        .iter()
        .any(|operation| matches!(operation, ContentOperation::SetFont { .. })));
    assert!(source.hints.has_text_show_operators);
    assert!(source.fonts.iter().any(|font| font.tounicode.is_some()));
    assert!(source
        .operations
        .iter()
        .any(|operation| matches!(operation, ContentOperation::SetTextRenderMode { mode: 0 })));
}

#[test]
fn xref_and_object_streams_are_transparent() {
    let source =
        load_page_source(&fixture("typst_xrefstream.pdf"), 0).expect("xref stream deve abrir");
    assert!(source.hints.has_text_show_operators);
    assert!(!source.operations.is_empty());
    assert!(source.fonts.iter().any(|font| font.tounicode.is_some()));
}

#[test]
fn encrypted_pdf_is_rejected_before_page_processing() {
    assert_eq!(
        load_page_source(&fixture("typst_encrypted.pdf"), 0),
        Err(PdfError::Encrypted)
    );
}

#[test]
fn page_index_is_zero_based_and_checked() {
    assert_eq!(
        load_page_source(&fixture("typst.pdf"), 1),
        Err(PdfError::PageNotFound { page_index: 1 })
    );
}

#[test]
fn scan_page_exposes_image_xobject_and_raw_hints() {
    let source = load_page_source(&fixture("scan.pdf"), 0).expect("scan deve abrir");
    assert!(!source.hints.has_text_show_operators);
    assert!(source.hints.has_do_operator);
    assert!(source
        .xobjects
        .iter()
        .any(|xobject| xobject.subtype == XObjectSubtype::Image));
}

#[test]
fn decoded_text_operations_preserve_order_and_bytes() {
    let source = load_page_source(&fixture("textops.pdf"), 0).expect("fixture deve abrir");
    let begin = source
        .operations
        .iter()
        .position(|operation| matches!(operation, ContentOperation::BeginText))
        .unwrap();
    assert!(matches!(
        &source.operations[begin + 1],
        ContentOperation::SetFont { name, size_pt } if name == "F1" && *size_pt == 18.0
    ));
    assert!(source.operations.iter().any(|operation| {
        matches!(operation, ContentOperation::ShowText { bytes } if bytes == b"ola")
    }));
    assert!(source
        .operations
        .iter()
        .any(|operation| matches!(operation, ContentOperation::MoveText { .. })));
    assert!(source.hints.has_text_show_operators);
    assert_eq!(source.fonts[0].tounicode, None);
}

#[test]
fn inherited_geometry_multiple_streams_and_descendant_metrics_are_preserved() {
    let path = temporary_pdf_path();
    save_contract_pdf(&path);
    let source = load_page_source(&path, 0).expect("PDF sintético deve abrir");
    std::fs::remove_file(&path).ok();

    assert_eq!(
        source.box_model.media_box,
        Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 300.0,
            y1: 400.0
        }
    );
    assert_eq!(source.box_model.crop_box, None);
    assert_eq!(source.box_model.rotate, Some(90));
    assert_eq!(source.box_model.user_unit, Some(2.0));

    assert!(matches!(source.operations[0], ContentOperation::SaveState));
    assert!(matches!(
        source.operations[1],
        ContentOperation::ConcatMatrix { .. }
    ));
    assert!(matches!(source.operations[2], ContentOperation::Other));
    assert!(matches!(source.operations[3], ContentOperation::BeginText));
    assert!(source.operations.iter().any(|operation| {
        matches!(operation, ContentOperation::SetCharSpacing { tc } if *tc == 0.5)
    }));
    assert!(source.operations.iter().any(|operation| {
        matches!(operation, ContentOperation::SetWordSpacing { tw } if *tw == 1.0)
    }));
    assert!(source.operations.iter().any(|operation| {
        matches!(operation, ContentOperation::SetHorizontalScaling { tz_percent } if *tz_percent == 80.0)
    }));
    assert!(source.operations.iter().any(|operation| {
        matches!(operation, ContentOperation::SetTextRise { ts } if *ts == 2.0)
    }));
    assert!(source.operations.iter().any(|operation| {
        matches!(operation, ContentOperation::UnsupportedTextOp { operator } if operator == "'")
    }));

    let font = source
        .fonts
        .iter()
        .find(|font| font.resource_name == "F0")
        .unwrap();
    assert_eq!(font.subtype, RawFontSubtype::Type0);
    assert_eq!(font.default_width, Some(500.0));
    assert_eq!(
        font.widths,
        vec![(1, 600.0), (2, 610.0), (3, 620.0), (4, 620.0)]
    );
}

#[test]
fn crop_box_and_simple_font_widths_are_extracted_raw() {
    let path = temporary_pdf_path();
    save_crop_and_simple_font_pdf(&path);
    let source = load_page_source(&path, 0).expect("PDF sintético deve abrir");
    std::fs::remove_file(&path).ok();

    assert_eq!(
        source.box_model.crop_box,
        Some(Rect {
            x0: 10.0,
            y0: 20.0,
            x1: 290.0,
            y1: 380.0,
        })
    );
    let font = source
        .fonts
        .iter()
        .find(|font| font.resource_name == "F1")
        .unwrap();
    assert_eq!(font.subtype, RawFontSubtype::Type1);
    assert_eq!(font.default_width, Some(250.0));
    assert_eq!(font.widths, vec![(32, 200.0), (33, 300.0)]);
}

fn temporary_pdf_path() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "decalque-lopdf-adapter-{}-{}.pdf",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn save_contract_pdf(path: &Path) {
    let mut document = Document::with_version("1.7");
    let descendant = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "DW" => 500,
        "W" => vec![
            Object::Integer(1),
            Object::Array(vec![600.into(), 610.into()]),
            Object::Integer(3),
            Object::Integer(4),
            Object::Integer(620),
        ],
    });
    let font = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "Synthetic",
        "Encoding" => "Identity-H",
        "DescendantFonts" => vec![Object::Reference(descendant)],
    });
    let first = document.add_object(Stream::new(
        dictionary! {},
        b"q 1 0 0 -1 0 400 cm 0 0 10 10 re".to_vec(),
    ));
    let second = document.add_object(Stream::new(
        dictionary! {},
        br"BT /F0 12 Tf 0.5 Tc 1 Tw 80 Tz 2 Ts 0 Tr (\000\001) Tj ' ET Q".to_vec(),
    ));
    let pages_id = document.new_object_id();
    let page = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "Contents" => vec![Object::Reference(first), Object::Reference(second)],
        "Resources" => dictionary! {
            "Font" => dictionary! { "F0" => Object::Reference(font) },
        },
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => 1,
            "Kids" => vec![Object::Reference(page)],
            "MediaBox" => vec![0.into(), 0.into(), 300.into(), 400.into()],
            "Rotate" => 90,
            "UserUnit" => 2.0,
        }),
    );
    let catalog = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    document.trailer.set("Root", Object::Reference(catalog));
    document.save(path).expect("salvar fixture temporário");
}

fn save_crop_and_simple_font_pdf(path: &Path) {
    let mut document = Document::with_version("1.5");
    let descriptor = document.add_object(dictionary! {
        "Type" => "FontDescriptor",
        "MissingWidth" => 250,
    });
    let font = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "FirstChar" => 32,
        "LastChar" => 33,
        "Widths" => vec![200.into(), 300.into()],
        "FontDescriptor" => Object::Reference(descriptor),
    });
    let content = document.add_object(Stream::new(dictionary! {}, Vec::new()));
    let pages_id = document.new_object_id();
    let page = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 300.into(), 400.into()],
        "CropBox" => vec![10.into(), 20.into(), 290.into(), 380.into()],
        "Contents" => Object::Reference(content),
        "Resources" => dictionary! {
            "Font" => dictionary! { "F1" => Object::Reference(font) },
        },
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => 1,
            "Kids" => vec![Object::Reference(page)],
        }),
    );
    let catalog = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    document.trailer.set("Root", Object::Reference(catalog));
    document.save(path).expect("salvar fixture temporário");
}
