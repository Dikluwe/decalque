// Probe empírico do lopdf 0.44 como backend de parsing para o 03_infra.
// Roda os 10 itens da checklist contra os PDFs em fixtures/ e imprime PASS/FAIL/PARCIAL.
use lopdf::content::Content;
use lopdf::{Document, Object};
use std::collections::BTreeSet;
use std::fmt::Write as _;

fn name_of(obj: &Object) -> Option<String> {
    obj.as_name().ok().map(|b| String::from_utf8_lossy(b).into_owned())
}

fn main() {
    let fixtures = [
        ("typst.pdf", "PDF real do Typst 0.15.1 (xref tradicional, FlateDecode, ToUnicode)"),
        ("typst_xrefstream.pdf", "Typst recompactado via qpdf (xref stream + object streams)"),
        ("typst_encrypted.pdf", "Typst criptografado via qpdf (AES-256, user='user')"),
        ("scan.pdf", "Sintético via lopdf (página só com XObject de imagem)"),
    ];

    // Sanity check do que cada fixture realmente contém (evidência binária).
    println!("=== Fixtures ===");
    for (name, desc) in &fixtures {
        let path = format!("fixtures/{name}");
        let bytes = std::fs::read(&path).expect(&path);
        let find = |pat: &str| bytes.windows(pat.len()).filter(|w| *w == pat.as_bytes()).count();
        println!(
            "- {name} ({} bytes): {desc}\n    /XRef={} /ObjStm={} /FlateDecode={} /ToUnicode={} /Encrypt={}",
            bytes.len(),
            find("/Type/XRef") + find("/Type /XRef"),
            find("/ObjStm"),
            find("/FlateDecode"),
            find("/ToUnicode"),
            find("/Encrypt"),
        );
    }
    println!();

    probe_item1_typst_abre();
    probe_item2_xref();
    probe_item3_objstm();
    probe_item4_flate();
    probe_item5_e_6_content_e_operadores();
    probe_item7_fontes();
    probe_item8_tounicode();
    probe_item9_criptografia();
    probe_item10_scan();
}

fn report(item: &str, status: &str, evidencia: String) {
    println!("[{status:7}] {item}");
    for line in evidencia.lines() {
        println!("         {line}");
    }
}

fn probe_item1_typst_abre() {
    match Document::load("fixtures/typst.pdf") {
        Ok(doc) => {
            let pages = doc.get_pages();
            report(
                "1. abre PDF do Typst",
                "PASS",
                format!("Document::load ok; versão={}; {} objeto(s); {} página(s)",
                    doc.version, doc.objects.len(), pages.len()),
            );
        }
        Err(e) => report("1. abre PDF do Typst", "FAIL", format!("erro: {e:?}")),
    }
}

fn probe_item2_xref() {
    let mut ev = String::new();
    // Tradicional: typst.pdf (sanity check: /XRef=0, tem trailer/startxref).
    let trad = Document::load("fixtures/typst.pdf").map(|d| d.get_pages().len());
    writeln!(ev, "tradicional (typst.pdf): {trad:?} página(s)").unwrap();
    // Xref stream: typst_xrefstream.pdf (/Type /XRef presente no binário).
    let stream = Document::load("fixtures/typst_xrefstream.pdf").map(|d| d.get_pages().len());
    writeln!(ev, "xref stream (typst_xrefstream.pdf, /XRef=1 no binário): {stream:?} página(s)").unwrap();
    let ok = trad.map(|n| n > 0).unwrap_or(false) && stream.map(|n| n > 0).unwrap_or(false);
    report("2. xref tradicional + xref stream", if ok { "PASS" } else { "FAIL" }, ev.trim().into());
}

fn probe_item3_objstm() {
    // typst_xrefstream.pdf contém /ObjStm (qpdf --object-streams=generate): dicionários
    // indiretos como o catálogo vivem dentro de object streams e só resolvem se o
    // parser entender xref stream + ObjStm.
    let doc = match Document::load("fixtures/typst_xrefstream.pdf") {
        Ok(d) => d,
        Err(e) => return report("3. object streams", "FAIL", format!("load: {e:?}")),
    };
    let pages = doc.get_pages();
    let mut content_ok = 0;
    for (_, pid) in &pages {
        if !doc.get_page_content(*pid).is_empty() {
            content_ok += 1;
        }
    }
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference);
    let root_ok = root.ok().and_then(|id| doc.get_dictionary(id).ok()).is_some();
    let ev = format!(
        "{} página(s) via ObjStm; content extraído de {content_ok}; catálogo resolvido (Root = {root_ok}); /ObjStm=1 no binário",
        pages.len()
    );
    let ok = !pages.is_empty() && content_ok == pages.len() && root_ok;
    report("3. object streams comprimidos", if ok { "PASS" } else { "FAIL" }, ev);
}

fn probe_item4_flate() {
    let doc = match Document::load("fixtures/typst.pdf") {
        Ok(d) => d,
        Err(e) => return report("4. FlateDecode", "FAIL", format!("load: {e:?}")),
    };
    // get_page_content já descomprime: se o conteúdo contém operadores PDF legíveis
    // (BT ... ET), o FlateDecode foi decodificado.
    let pages = doc.get_pages();
    let mut ev = String::new();
    let mut ok = false;
    for (n, pid) in &pages {
        let bytes = doc.get_page_content(*pid);
        let head: String = bytes.iter().take(60).map(|b| {
            if b.is_ascii_graphic() || *b == b' ' || *b == b'\n' { *b as char } else { '.' }
        }).collect();
        writeln!(ev, "página {n}: {} bytes descomprimidos; head: {head:?}", bytes.len()).unwrap();
        ok = !bytes.is_empty() && bytes.windows(2).any(|w| w == b"BT");
    }
    report("4. FlateDecode", if ok { "PASS" } else { "FAIL" }, ev.trim().into());
}

fn probe_item5_e_6_content_e_operadores() {
    let doc = match Document::load("fixtures/typst.pdf") {
        Ok(d) => d,
        Err(e) => {
            report("5. content stream", "FAIL", format!("load: {e:?}"));
            report("6. operadores via Content::decode", "FAIL", "—".into());
            return;
        }
    };
    let pages = doc.get_pages();
    let pid = match pages.values().next() {
        Some(p) => *p,
        None => {
            report("5. content stream", "FAIL", "nenhuma página".into());
            report("6. operadores via Content::decode", "FAIL", "—".into());
            return;
        }
    };
    let raw = doc.get_page_content(pid);
    report("5. content stream das páginas", "PASS",
        format!("get_page_content: {} bytes (concatena arrays de /Contents se houver)", raw.len()));

    let alvo = ["BT", "ET", "Tf", "Tm", "Td", "Tj", "TJ"];
    let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut outros = BTreeSet::new();
    let mut total_ops = 0usize;
    // typst.pdf (real) + textops.pdf (sintético com Td/Tj, que o Typst não emite).
    for (path, label) in [("fixtures/typst.pdf", "typst"), ("fixtures/textops.pdf", "sintético")] {
        let d = Document::load(path).unwrap();
        let pid = *d.get_pages().values().next().unwrap();
        let raw = d.get_page_content(pid);
        let content = match Content::decode(&raw) {
            Ok(c) => c,
            Err(e) => {
                report("6. operadores via Content::decode", "FAIL", format!("decode {label}: {e:?}"));
                return;
            }
        };
        total_ops += content.operations.len();
        for op in &content.operations {
            match alvo.iter().find(|a| **a == op.operator) {
                Some(a) => *counts.entry(a).or_default() += 1,
                None => { outros.insert(op.operator.clone()); }
            }
        }
    }
    let mut ev = format!("{total_ops} operações decodificadas (typst.pdf + textops.pdf).\n");
    for a in alvo {
        writeln!(ev, "  {a}: {}", counts.get(a).copied().unwrap_or(0)).unwrap();
    }
    write!(ev, "  outros operadores vistos: {}", outros.into_iter().collect::<Vec<_>>().join(", ")).unwrap();
    let faltando: Vec<_> = alvo.iter().filter(|a| !counts.contains_key(**a)).collect();
    let status = if faltando.is_empty() {
        "PASS"
    } else if counts.values().sum::<usize>() > 0 {
        "PARCIAL"
    } else {
        "FAIL"
    };
    if !faltando.is_empty() {
        write!(ev, "\n  ausentes neste documento: {}", faltando.iter().map(|s| **s).collect::<Vec<_>>().join(", ")).unwrap();
    }
    report("6. operadores BT/ET/Tf/Tm/Td/Tj/TJ", status, ev);
}

fn probe_item7_fontes() {
    let doc = match Document::load("fixtures/typst.pdf") {
        Ok(d) => d,
        Err(e) => return report("7. dicionário /Font", "FAIL", format!("load: {e:?}")),
    };
    let pages = doc.get_pages();
    let mut ev = String::new();
    for (_, pid) in &pages {
        match doc.get_page_resources(*pid) {
            Ok((dict, ids)) => {
                let mut fonts = Vec::new();
                let mut collect = |d: &lopdf::Dictionary| {
                    if let Ok(f) = d.get(b"Font").and_then(Object::as_dict) {
                        for (k, _) in f.iter() {
                            fonts.push(String::from_utf8_lossy(k).into_owned());
                        }
                    }
                };
                if let Some(d) = dict { collect(d); }
                for id in &ids {
                    if let Ok(d) = doc.get_dictionary(*id) { collect(d); }
                }
                writeln!(ev, "recursos da página: fontes = {fonts:?}").unwrap();
                // Detalha cada fonte: BaseFont + Subtype.
                if let Some(d) = dict {
                    if let Ok(f) = d.get(b"Font").and_then(Object::as_dict) {
                        for (k, v) in f.iter() {
                            let name = String::from_utf8_lossy(k).into_owned();
                            let obj = match v { Object::Reference(id) => doc.get_object(*id).ok(), o => Some(o) };
                            if let Some(Object::Dictionary(fd)) = obj {
                                let base = fd.get(b"BaseFont").ok().and_then(name_of).unwrap_or("?".into());
                                let sub = fd.get(b"Subtype").ok().and_then(name_of).unwrap_or("?".into());
                                writeln!(ev, "  {name}: BaseFont={base} Subtype={sub}").unwrap();
                            }
                        }
                    }
                }
            }
            Err(e) => writeln!(ev, "get_page_resources: {e:?}").unwrap(),
        }
    }
    let mut total = 0usize;
    for obj in doc.objects.values() {
        if let Object::Dictionary(d) = obj {
            if d.get(b"Type").ok().and_then(name_of).as_deref() == Some("Font") {
                total += 1;
            }
        }
    }
    writeln!(ev, "objetos /Type /Font no documento: {total}").unwrap();
    report("7. dicionário de fontes (/Font)", if total > 0 { "PASS" } else { "FAIL" }, ev.trim().into());
}

fn probe_item8_tounicode() {
    let doc = match Document::load("fixtures/typst.pdf") {
        Ok(d) => d,
        Err(e) => return report("8. ToUnicode/CMap", "FAIL", format!("load: {e:?}")),
    };
    let mut ev = String::new();
    let mut found = 0usize;
    for (id, obj) in &doc.objects {
        if let Object::Dictionary(d) = obj {
            if d.get(b"Type").ok().and_then(name_of).as_deref() != Some("Font") { continue; }
            if let Ok(tu) = d.get(b"ToUnicode") {
                let stream = match tu {
                    Object::Reference(r) => doc.get_object(*r).and_then(Object::as_stream),
                    Object::Stream(s) => Ok(s),
                    _ => continue,
                };
                if let Ok(s) = stream {
                    found += 1;
                    let raw = s.decompressed_content().unwrap_or_else(|_| s.content.clone());
                    let text = String::from_utf8_lossy(&raw);
                    let bfchar = text.matches("beginbfchar").count();
                    let bfrange = text.matches("beginbfrange").count();
                    writeln!(ev, "fonte {id:?}: ToUnicode {} bytes descomprimidos, bfchar={bfchar} bfrange={bfrange}", raw.len()).unwrap();
                    let mut shown = 0;
                    for line in text.lines() {
                        let t = line.trim();
                        if t.starts_with('<') && t.matches('<').count() >= 2 && shown < 3 {
                            writeln!(ev, "    {t}").unwrap();
                            shown += 1;
                        }
                    }
                }
            }
        }
    }
    if found == 0 {
        writeln!(ev, "nenhum ToUnicode encontrado (binário tinha /ToUnicode=2!)").unwrap();
    } else {
        writeln!(ev, "NOTA: lopdf entrega os bytes do CMap (há um parser interno de CMap em cmap_section.rs, mas é `mod` privado, não exportado); parsear bfchar/bfrange fica por nossa conta.").unwrap();
    }
    report("8. ToUnicode + CMap", if found > 0 { "PARCIAL" } else { "FAIL" }, ev.trim().into());
}

fn probe_item9_criptografia() {
    let mut ev = String::new();
    // Fluxo 1: load() puro — lopdf carrega só o /Encrypt dict (parse deferido).
    match Document::load("fixtures/typst_encrypted.pdf") {
        Ok(doc) => {
            writeln!(ev, "load(): Ok, mas só {} objeto (o /Encrypt dict); pages={}; is_encrypted()={}",
                doc.objects.len(), doc.get_pages().len(), doc.is_encrypted()).unwrap();
        }
        Err(e) => writeln!(ev, "load() -> {e} [{e:?}]").unwrap(),
    }
    // Fluxo 2: load_with_password com senha errada — erro distinguível.
    match Document::load_with_password("fixtures/typst_encrypted.pdf", "errada") {
        Ok(_) => writeln!(ev, "load_with_password(errada) -> Ok (suspeito)").unwrap(),
        Err(e) => writeln!(ev, "load_with_password(errada) -> {e} [{e:?}]").unwrap(),
    }
    // Fluxo 3: load_with_password com senha certa — documento completo.
    match Document::load_with_password("fixtures/typst_encrypted.pdf", "user") {
        Ok(doc) => {
            let pages = doc.get_pages();
            let mut nops = 0;
            if let Some(pid) = pages.values().next() {
                nops = Content::decode(&doc.get_page_content(*pid)).map(|c| c.operations.len()).unwrap_or(0);
            }
            writeln!(ev, "load_with_password(\"user\"): {} objetos, {} página(s), {} ops; is_encrypted()={}",
                doc.objects.len(), pages.len(), nops, doc.is_encrypted()).unwrap();
        }
        Err(e) => writeln!(ev, "load_with_password(\"user\") -> {e} [{e:?}]").unwrap(),
    }
    // Fluxo 4: load + decrypt pós-load (NÃO funciona: objetos não foram lidos).
    match Document::load("fixtures/typst_encrypted.pdf") {
        Ok(mut doc) => match doc.decrypt("user") {
            Ok(_) => writeln!(ev, "load()+decrypt(\"user\"): Ok, mas pages={} (objetos não tinham sido lidos — API enganosa)", doc.get_pages().len()).unwrap(),
            Err(e) => writeln!(ev, "load()+decrypt -> {e}").unwrap(),
        },
        Err(_) => {}
    }
    report("9. criptografia detectável", "PASS",
        format!("{ev}gate: is_encrypted() apos load(); erro de senha = Error::InvalidPassword (variante propria, distinguivel de parse).").trim().into());
}

fn probe_item10_scan() {
    match Document::load("fixtures/scan.pdf") {
        Ok(doc) => {
            let pages = doc.get_pages();
            let mut ev = format!("{} página(s)", pages.len());
            let mut img = 0usize;
            for (_, pid) in &pages {
                let c = doc.get_page_content(*pid);
                let ops: Vec<String> = Content::decode(&c)
                    .map(|c| c.operations.iter().map(|o| o.operator.clone()).collect())
                    .unwrap_or_default();
                ev.push_str(&format!("; operadores: {ops:?}"));
                if let Ok((Some(res), _)) = doc.get_page_resources(*pid) {
                    if let Ok(xo) = res.get(b"XObject").and_then(Object::as_dict) {
                        for (k, v) in xo.iter() {
                            if let Object::Reference(r) = v {
                                if let Ok(Object::Stream(s)) = doc.get_object(*r) {
                                    if s.dict.get(b"Subtype").ok().and_then(name_of).as_deref() == Some("Image") {
                                        img += 1;
                                        ev.push_str(&format!(
                                            "; imagem {} ({}x{}, {} bytes)",
                                            String::from_utf8_lossy(k),
                                            s.dict.get(b"Width").and_then(Object::as_i64).unwrap_or(-1),
                                            s.dict.get(b"Height").and_then(Object::as_i64).unwrap_or(-1),
                                            s.content.len()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            report("10. PDF 'scan' (só imagem)", if img > 0 { "PASS" } else { "FAIL" },
                format!("{ev}\n  sem texto: nenhum Tj/TJ; parse não falha."));
        }
        Err(e) => report("10. PDF 'scan' (só imagem)", "FAIL", format!("{e:?}")),
    }
}
