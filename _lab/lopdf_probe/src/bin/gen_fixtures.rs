// Gera fixtures sintéticas com o próprio lopdf:
//  - scan.pdf: página contendo apenas um XObject de imagem (sem texto)
use lopdf::{dictionary, Document, Object, Stream};
use std::collections::BTreeMap;

fn main() {
    let mut doc = Document::with_version("1.5");

    // Imagem RGB 4x4, 8 bits por componente, sem compressão.
    let w = 4i64;
    let h = 4i64;
    let mut pixels = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            pixels.push((x * 60) as u8);
            pixels.push((y * 60) as u8);
            pixels.push(200u8);
        }
    }
    let img = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => w,
            "Height" => h,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
        },
        pixels,
    );
    let img_id = doc.add_object(img);

    // Content stream: só desenha a imagem.
    let content = Stream::new(
        dictionary! {},
        b"q\n100 0 0 100 50 50 cm\n/Im0 Do\nQ\n".to_vec(),
    );
    let content_id = doc.add_object(content);

    let resources = dictionary! {
        "XObject" => dictionary! { "Im0" => Object::Reference(img_id) },
        "ProcSet" => vec![Object::from("PDF"), Object::from("ImageC")],
    };

    let pages_id = doc.new_object_id();
    let page = dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 300.into(), 300.into()],
        "Contents" => Object::Reference(content_id),
        "Resources" => resources,
    };
    let page_id = doc.add_object(page);
    let pages = dictionary! {
        "Type" => "Pages",
        "Count" => 1,
        "Kids" => vec![Object::Reference(page_id)],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    };
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut map = BTreeMap::new();
    map.insert(1u32, page_id);
    // get_pages é derivado da árvore; confirmação real acontece no probe.
    let _ = map;

    doc.save("fixtures/scan.pdf").expect("save scan.pdf");
    println!("scan.pdf gerado");

    // textops.pdf: cobre Td e Tj (que o Typst não emite), além de BT/ET/Tf/Tm/TJ.
    let mut doc2 = Document::with_version("1.5");
    let font = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    let font_id = doc2.add_object(font);
    let content2 = Stream::new(
        dictionary! {},
        b"BT\n/F1 18 Tf\n1 0 0 1 50 250 Tm\n(ola) Tj\n10 -20 Td\n[(mundo) 30 (fi)] TJ\nET\n".to_vec(),
    );
    let content2_id = doc2.add_object(content2);
    let pages2_id = doc2.new_object_id();
    let page2 = dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages2_id),
        "MediaBox" => vec![0.into(), 0.into(), 300.into(), 300.into()],
        "Contents" => Object::Reference(content2_id),
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => Object::Reference(font_id) } },
    };
    let page2_id = doc2.add_object(page2);
    doc2.objects.insert(pages2_id, Object::Dictionary(dictionary! {
        "Type" => "Pages",
        "Count" => 1,
        "Kids" => vec![Object::Reference(page2_id)],
    }));
    let catalog2_id = doc2.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages2_id),
    });
    doc2.trailer.set("Root", Object::Reference(catalog2_id));
    doc2.save("fixtures/textops.pdf").expect("save textops.pdf");
    println!("textops.pdf gerado");
}
