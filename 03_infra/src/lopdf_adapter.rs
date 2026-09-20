//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/lopdf-backend-adapter.md
//! @prompt 00_nucleo/prompts/scan-typst-candidate-evaluation.md
//! @layer L3
//! @updated 2026-09-19

use decalque_core::entities::{PageBoxModel, Rect};
use decalque_core::{
    ContentOperation, PdfError, RawFontData, RawFontEncoding, RawFontSubtype, TjItem, XObjectInfo,
    XObjectSubtype,
};
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSourceHints {
    pub has_text_show_operators: bool,
    pub has_do_operator: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageSource {
    pub box_model: PageBoxModel,
    pub operations: Vec<ContentOperation>,
    pub fonts: Vec<RawFontData>,
    pub xobjects: Vec<XObjectInfo>,
    pub hints: PageSourceHints,
}

pub fn load_page_source(path: &Path, page_index: usize) -> Result<PageSource, PdfError> {
    let document = Document::load(path).map_err(map_load_error)?;
    load_page_source_from_document(&document, page_index)
}

pub fn load_single_page_source_from_pdf_bytes(bytes: &[u8]) -> Result<PageSource, PdfError> {
    let document = Document::load_mem(bytes).map_err(map_load_error)?;
    if document.is_encrypted() {
        return Err(PdfError::Encrypted);
    }
    let page_count = document.get_pages().len();
    if page_count != 1 {
        return Err(PdfError::Parse {
            message: format!(
                "PDF candidato deve conter exatamente uma página; encontrou {page_count}"
            ),
        });
    }
    load_page_source_from_document(&document, 0)
}

fn load_page_source_from_document(
    document: &Document,
    page_index: usize,
) -> Result<PageSource, PdfError> {
    if document.is_encrypted() {
        return Err(PdfError::Encrypted);
    }
    validate_catalog(document)?;

    let page_id = document
        .get_pages()
        .values()
        .nth(page_index)
        .copied()
        .ok_or(PdfError::PageNotFound { page_index })?;
    let box_model = extract_page_box_model(document, page_id)?;
    let operations = extract_operations(document, page_id)?;
    let resources = inherited_object(document, page_id, b"Resources")?
        .map(|object| object.as_dict().map_err(parse_error))
        .transpose()?;
    let fonts = resources
        .map(|dictionary| extract_fonts(document, dictionary))
        .transpose()?
        .unwrap_or_default();
    let xobjects = resources
        .map(|dictionary| extract_xobjects(document, dictionary))
        .transpose()?
        .unwrap_or_default();
    let hints = PageSourceHints {
        has_text_show_operators: operations.iter().any(|operation| {
            matches!(
                operation,
                ContentOperation::ShowText { .. } | ContentOperation::ShowTextAdjusted { .. }
            )
        }),
        has_do_operator: operations
            .iter()
            .any(|operation| matches!(operation, ContentOperation::InvokeXObject { .. })),
    };
    Ok(PageSource {
        box_model,
        operations,
        fonts,
        xobjects,
        hints,
    })
}

fn validate_catalog(document: &Document) -> Result<(), PdfError> {
    let root = document
        .trailer
        .get(b"Root")
        .map_err(parse_error)
        .and_then(|object| dereference(document, object))?;
    root.as_dict().map_err(parse_error)?;
    Ok(())
}

fn extract_page_box_model(
    document: &Document,
    page_id: ObjectId,
) -> Result<PageBoxModel, PdfError> {
    let media_box = inherited_object(document, page_id, b"MediaBox")?
        .ok_or_else(|| PdfError::Parse {
            message: "MediaBox ausente na página e em seus ancestrais".to_string(),
        })
        .and_then(|object| parse_rect(document, object))?;
    let crop_box = inherited_object(document, page_id, b"CropBox")?
        .map(|object| parse_rect(document, object))
        .transpose()?;
    let rotate = inherited_object(document, page_id, b"Rotate")?
        .map(|object| integer(document, object).and_then(to_i32))
        .transpose()?;
    let user_unit = inherited_object(document, page_id, b"UserUnit")?
        .map(|object| number(document, object))
        .transpose()?;
    Ok(PageBoxModel {
        media_box,
        crop_box,
        rotate,
        user_unit,
    })
}

fn inherited_object<'a>(
    document: &'a Document,
    start: ObjectId,
    key: &[u8],
) -> Result<Option<&'a Object>, PdfError> {
    let mut current = start;
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current) {
            return Err(PdfError::Parse {
                message: "ciclo na árvore de páginas".to_string(),
            });
        }
        let dictionary = document.get_dictionary(current).map_err(parse_error)?;
        if let Ok(value) = dictionary.get(key) {
            return dereference(document, value).map(Some);
        }
        match dictionary.get(b"Parent") {
            Ok(parent) => current = reference(document, parent)?,
            Err(_) => return Ok(None),
        }
    }
}

fn extract_operations(
    document: &Document,
    page_id: ObjectId,
) -> Result<Vec<ContentOperation>, PdfError> {
    let page = document.get_dictionary(page_id).map_err(parse_error)?;
    let contents = match page.get(b"Contents") {
        Ok(contents) => contents,
        Err(_) => return Ok(Vec::new()),
    };
    let mut streams = Vec::new();
    collect_content_streams(document, contents, &mut streams)?;
    let mut operations = Vec::new();
    for stream in streams {
        let bytes = stream.decompressed_content().map_err(parse_error)?;
        let content = Content::decode(&bytes).map_err(parse_error)?;
        for operation in &content.operations {
            operations.push(convert_operation(document, operation)?);
        }
    }
    Ok(operations)
}

fn collect_content_streams<'a>(
    document: &'a Document,
    object: &'a Object,
    output: &mut Vec<&'a lopdf::Stream>,
) -> Result<(), PdfError> {
    match dereference(document, object)? {
        Object::Stream(stream) => output.push(stream),
        Object::Array(items) => {
            for item in items {
                collect_content_streams(document, item, output)?;
            }
        }
        _ => {
            return Err(PdfError::Parse {
                message: "Contents não é stream nem array de streams".to_string(),
            });
        }
    }
    Ok(())
}

fn convert_operation(
    document: &Document,
    operation: &Operation,
) -> Result<ContentOperation, PdfError> {
    let operands = &operation.operands;
    let converted = match operation.operator.as_str() {
        "BT" => ContentOperation::BeginText,
        "ET" => ContentOperation::EndText,
        "Tf" => ContentOperation::SetFont {
            name: name(document, required(operands, 0, "Tf")?)?,
            size_pt: number(document, required(operands, 1, "Tf")?)?,
        },
        "Tm" => ContentOperation::SetTextMatrix {
            m: matrix(document, operands, "Tm")?,
        },
        "Td" => ContentOperation::MoveText {
            tx: number(document, required(operands, 0, "Td")?)?,
            ty: number(document, required(operands, 1, "Td")?)?,
        },
        "TD" => ContentOperation::MoveTextSetLeading {
            tx: number(document, required(operands, 0, "TD")?)?,
            ty: number(document, required(operands, 1, "TD")?)?,
        },
        "T*" => ContentOperation::NextLine,
        "TL" => ContentOperation::SetLeading {
            leading: number(document, required(operands, 0, "TL")?)?,
        },
        "Tc" => ContentOperation::SetCharSpacing {
            tc: number(document, required(operands, 0, "Tc")?)?,
        },
        "Tw" => ContentOperation::SetWordSpacing {
            tw: number(document, required(operands, 0, "Tw")?)?,
        },
        "Tz" => ContentOperation::SetHorizontalScaling {
            tz_percent: number(document, required(operands, 0, "Tz")?)?,
        },
        "Ts" => ContentOperation::SetTextRise {
            ts: number(document, required(operands, 0, "Ts")?)?,
        },
        "Tr" => ContentOperation::SetTextRenderMode {
            mode: integer(document, required(operands, 0, "Tr")?).and_then(to_i32)?,
        },
        "Tj" => ContentOperation::ShowText {
            bytes: string_bytes(document, required(operands, 0, "Tj")?)?,
        },
        "TJ" => ContentOperation::ShowTextAdjusted {
            items: tj_items(document, required(operands, 0, "TJ")?)?,
        },
        "q" => ContentOperation::SaveState,
        "Q" => ContentOperation::RestoreState,
        "cm" => ContentOperation::ConcatMatrix {
            m: matrix(document, operands, "cm")?,
        },
        "Do" => ContentOperation::InvokeXObject {
            name: name(document, required(operands, 0, "Do")?)?,
        },
        operator if operator.starts_with('T') || matches!(operator, "'" | "\"") => {
            ContentOperation::UnsupportedTextOp {
                operator: operator.to_string(),
            }
        }
        _ => ContentOperation::Other,
    };
    Ok(converted)
}

fn tj_items(document: &Document, object: &Object) -> Result<Vec<TjItem>, PdfError> {
    let array = dereference(document, object)?
        .as_array()
        .map_err(parse_error)?;
    array
        .iter()
        .map(|item| match dereference(document, item)? {
            Object::String(bytes, _) => Ok(TjItem::Text {
                bytes: bytes.clone(),
            }),
            Object::Integer(value) => Ok(TjItem::Adjustment {
                thousandths: *value as f64,
            }),
            Object::Real(value) => Ok(TjItem::Adjustment {
                thousandths: *value as f64,
            }),
            _ => Err(PdfError::Parse {
                message: "TJ contém item que não é string nem número".to_string(),
            }),
        })
        .collect()
}

fn extract_fonts(
    document: &Document,
    resources: &Dictionary,
) -> Result<Vec<RawFontData>, PdfError> {
    let font_dictionary = match resources.get(b"Font") {
        Ok(value) => dereference(document, value)?
            .as_dict()
            .map_err(parse_error)?,
        Err(_) => return Ok(Vec::new()),
    };
    font_dictionary
        .iter()
        .map(|(resource_name, value)| {
            let font = dereference(document, value)?
                .as_dict()
                .map_err(parse_error)?;
            extract_font(document, resource_name, font)
        })
        .collect()
}

fn extract_font(
    document: &Document,
    resource_name: &[u8],
    font: &Dictionary,
) -> Result<RawFontData, PdfError> {
    let subtype = font
        .get(b"Subtype")
        .ok()
        .and_then(|value| name(document, value).ok())
        .map(|value| match value.as_str() {
            "Type0" => RawFontSubtype::Type0,
            "Type1" => RawFontSubtype::Type1,
            "TrueType" => RawFontSubtype::TrueType,
            "CIDFontType0" => RawFontSubtype::CIDFontType0,
            "CIDFontType2" => RawFontSubtype::CIDFontType2,
            _ => RawFontSubtype::Other,
        })
        .unwrap_or(RawFontSubtype::Other);
    let metrics = if subtype == RawFontSubtype::Type0 {
        descendant_font(document, font)?.unwrap_or(font)
    } else {
        font
    };
    let widths = if subtype == RawFontSubtype::Type0 {
        extract_cid_widths(document, metrics)?
    } else {
        extract_simple_widths(document, metrics)?
    };
    let default_width = if subtype == RawFontSubtype::Type0 {
        metrics
            .get(b"DW")
            .ok()
            .map(|value| number(document, value))
            .transpose()?
    } else {
        missing_width(document, font)?
    };
    let tounicode = font
        .get(b"ToUnicode")
        .ok()
        .and_then(|value| dereference(document, value).ok())
        .and_then(|value| value.as_stream().ok())
        .and_then(|stream| stream.decompressed_content().ok());
    Ok(RawFontData {
        resource_name: String::from_utf8_lossy(resource_name).into_owned(),
        base_font: font
            .get(b"BaseFont")
            .ok()
            .and_then(|value| name(document, value).ok()),
        subtype,
        encoding: extract_encoding(document, font)?,
        default_width,
        widths,
        tounicode,
    })
}

fn descendant_font<'a>(
    document: &'a Document,
    font: &'a Dictionary,
) -> Result<Option<&'a Dictionary>, PdfError> {
    let descendants = match font.get(b"DescendantFonts") {
        Ok(value) => dereference(document, value)?
            .as_array()
            .map_err(parse_error)?,
        Err(_) => return Ok(None),
    };
    descendants
        .first()
        .map(|value| dereference(document, value)?.as_dict().map_err(parse_error))
        .transpose()
}

fn extract_simple_widths(
    document: &Document,
    font: &Dictionary,
) -> Result<Vec<(u32, f64)>, PdfError> {
    let first = match font.get(b"FirstChar") {
        Ok(value) => integer(document, value)?,
        Err(_) => return Ok(Vec::new()),
    };
    let widths = match font.get(b"Widths") {
        Ok(value) => dereference(document, value)?
            .as_array()
            .map_err(parse_error)?,
        Err(_) => return Ok(Vec::new()),
    };
    widths
        .iter()
        .enumerate()
        .map(|(offset, width)| {
            let code = u32::try_from(first + offset as i64).map_err(|error| PdfError::Parse {
                message: format!("código de largura inválido: {error}"),
            })?;
            Ok((code, number(document, width)?))
        })
        .collect()
}

fn extract_cid_widths(document: &Document, font: &Dictionary) -> Result<Vec<(u32, f64)>, PdfError> {
    let items = match font.get(b"W") {
        Ok(value) => dereference(document, value)?
            .as_array()
            .map_err(parse_error)?,
        Err(_) => return Ok(Vec::new()),
    };
    let mut output = Vec::new();
    let mut index = 0;
    while index < items.len() {
        let start = integer(document, &items[index])?;
        index += 1;
        let Some(next) = items.get(index) else {
            return Err(PdfError::Parse {
                message: "array W truncado".to_string(),
            });
        };
        match dereference(document, next)? {
            Object::Array(widths) => {
                for (offset, width) in widths.iter().enumerate() {
                    output.push((to_u32(start + offset as i64)?, number(document, width)?));
                }
                index += 1;
            }
            _ => {
                let end = integer(document, next)?;
                let width = number(
                    document,
                    items.get(index + 1).ok_or_else(|| PdfError::Parse {
                        message: "array W truncado".to_string(),
                    })?,
                )?;
                if end < start {
                    return Err(PdfError::Parse {
                        message: "intervalo decrescente em W".to_string(),
                    });
                }
                for code in start..=end {
                    output.push((to_u32(code)?, width));
                }
                index += 2;
            }
        }
    }
    Ok(output)
}

fn missing_width(document: &Document, font: &Dictionary) -> Result<Option<f64>, PdfError> {
    let descriptor = match font.get(b"FontDescriptor") {
        Ok(value) => dereference(document, value)?
            .as_dict()
            .map_err(parse_error)?,
        Err(_) => return Ok(None),
    };
    descriptor
        .get(b"MissingWidth")
        .ok()
        .map(|value| number(document, value))
        .transpose()
}

fn extract_encoding(document: &Document, font: &Dictionary) -> Result<RawFontEncoding, PdfError> {
    let encoding = match font.get(b"Encoding") {
        Ok(value) => dereference(document, value)?,
        Err(_) => return Ok(RawFontEncoding::Absent),
    };
    match encoding {
        Object::Name(value) => Ok(RawFontEncoding::Name(
            String::from_utf8_lossy(value).into_owned(),
        )),
        Object::Dictionary(dictionary) => {
            let differences = dictionary
                .get(b"Differences")
                .ok()
                .and_then(|value| dereference(document, value).ok())
                .and_then(|value| value.as_array().ok())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| name(document, item).ok())
                        .collect()
                })
                .unwrap_or_default();
            Ok(RawFontEncoding::Differences(differences))
        }
        Object::Stream(stream) => Ok(RawFontEncoding::Stream(
            stream.decompressed_content().map_err(parse_error)?,
        )),
        _ => Ok(RawFontEncoding::Absent),
    }
}

fn extract_xobjects(
    document: &Document,
    resources: &Dictionary,
) -> Result<Vec<XObjectInfo>, PdfError> {
    let dictionary = match resources.get(b"XObject") {
        Ok(value) => dereference(document, value)?
            .as_dict()
            .map_err(parse_error)?,
        Err(_) => return Ok(Vec::new()),
    };
    dictionary
        .iter()
        .map(|(name_bytes, value)| {
            let object = dereference(document, value)?;
            let dictionary = match object {
                Object::Stream(stream) => &stream.dict,
                Object::Dictionary(dictionary) => dictionary,
                _ => {
                    return Err(PdfError::Parse {
                        message: "XObject não é stream nem dicionário".to_string(),
                    });
                }
            };
            let subtype = dictionary
                .get(b"Subtype")
                .ok()
                .and_then(|value| name(document, value).ok())
                .map(|value| match value.as_str() {
                    "Form" => XObjectSubtype::Form,
                    "Image" => XObjectSubtype::Image,
                    _ => XObjectSubtype::Other,
                })
                .unwrap_or(XObjectSubtype::Other);
            Ok(XObjectInfo {
                name: String::from_utf8_lossy(name_bytes).into_owned(),
                subtype,
            })
        })
        .collect()
}

fn parse_rect(document: &Document, object: &Object) -> Result<Rect, PdfError> {
    let values = dereference(document, object)?
        .as_array()
        .map_err(parse_error)?;
    if values.len() != 4 {
        return Err(PdfError::Parse {
            message: "caixa de página deve conter quatro números".to_string(),
        });
    }
    Ok(Rect {
        x0: number(document, &values[0])?,
        y0: number(document, &values[1])?,
        x1: number(document, &values[2])?,
        y1: number(document, &values[3])?,
    })
}

fn matrix(document: &Document, values: &[Object], operator: &str) -> Result<[f64; 6], PdfError> {
    if values.len() < 6 {
        return Err(PdfError::Parse {
            message: format!("{operator} requer seis operandos"),
        });
    }
    Ok([
        number(document, &values[0])?,
        number(document, &values[1])?,
        number(document, &values[2])?,
        number(document, &values[3])?,
        number(document, &values[4])?,
        number(document, &values[5])?,
    ])
}

fn required<'a>(
    values: &'a [Object],
    index: usize,
    operator: &str,
) -> Result<&'a Object, PdfError> {
    values.get(index).ok_or_else(|| PdfError::Parse {
        message: format!("{operator} sem operando {index}"),
    })
}

fn dereference<'a>(document: &'a Document, object: &'a Object) -> Result<&'a Object, PdfError> {
    document
        .dereference(object)
        .map(|(_, value)| value)
        .map_err(parse_error)
}

fn reference(document: &Document, object: &Object) -> Result<ObjectId, PdfError> {
    dereference_reference(document, object).ok_or_else(|| PdfError::Parse {
        message: "Parent não é referência".to_string(),
    })
}

fn dereference_reference(_document: &Document, object: &Object) -> Option<ObjectId> {
    match object {
        Object::Reference(id) => Some(*id),
        _ => None,
    }
}

fn number(document: &Document, object: &Object) -> Result<f64, PdfError> {
    match dereference(document, object)? {
        Object::Integer(value) => Ok(*value as f64),
        Object::Real(value) => Ok(*value as f64),
        value => Err(PdfError::Parse {
            message: format!("esperado número, encontrado {}", value.enum_variant()),
        }),
    }
}

fn integer(document: &Document, object: &Object) -> Result<i64, PdfError> {
    dereference(document, object)?.as_i64().map_err(parse_error)
}

fn name(document: &Document, object: &Object) -> Result<String, PdfError> {
    dereference(document, object)?
        .as_name()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .map_err(parse_error)
}

fn string_bytes(document: &Document, object: &Object) -> Result<Vec<u8>, PdfError> {
    dereference(document, object)?
        .as_str()
        .map(|bytes| bytes.to_vec())
        .map_err(parse_error)
}

fn to_i32(value: i64) -> Result<i32, PdfError> {
    i32::try_from(value).map_err(|error| PdfError::Parse {
        message: format!("inteiro fora do intervalo i32: {error}"),
    })
}

fn to_u32(value: i64) -> Result<u32, PdfError> {
    u32::try_from(value).map_err(|error| PdfError::Parse {
        message: format!("código de glifo inválido: {error}"),
    })
}

fn map_load_error(error: lopdf::Error) -> PdfError {
    match error {
        lopdf::Error::IO(error) => PdfError::Io {
            message: error.to_string(),
        },
        other => parse_error(other),
    }
}

fn parse_error(error: impl std::fmt::Display) -> PdfError {
    PdfError::Parse {
        message: error.to_string(),
    }
}
