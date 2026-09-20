//! Testes comportamentais do renderer JSON/Typst do perfil geométrico.

use decalque_core::{
    derive_scan_layout, Claim, Confidence, EvidenceBasis, FramedBbox, GeometryClaims,
    ObservationKind, ObservationUnit, PageFrame, PageMapping, PageMappingKind, ProducerIdentity,
    ProvenanceRecord, ProvenanceStage, RasterArtifact, RasterFrame, ScanLayoutProfile,
    ScanObservation, ScanSource, UnknownReason,
};
use decalque_shell::render_scan_layout_profile;

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FRAME: &str = "frame\"\\\n\u{1}\u{7f}e";
const REGION: &str = "region\"\\\t\u{2}\u{7f}c";

fn known<T>(value: T) -> Claim<T> {
    Claim::Known {
        value,
        basis: EvidenceBasis::Observed,
        evidence: vec!["p".into()],
        confidence: Confidence::Known {
            value: 1.0,
            semantics: "oracle-observation".into(),
        },
    }
}

fn unknown<T>() -> Claim<T> {
    Claim::Unknown {
        reason: UnknownReason::NotObserved,
        evidence: Vec::new(),
        detail: None,
    }
}

fn mapping(frame_id: &str) -> Claim<PageMapping> {
    known(PageMapping {
        source_frame_id: "raster-px".into(),
        target_frame: PageFrame {
            id: frame_id.into(),
            extent: (200.0, 300.0),
        },
        kind: PageMappingKind::Homography3x3,
        matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        max_error_pt: -0.0,
    })
}

fn observation(page_mapping: Claim<PageMapping>, units: Vec<ObservationUnit>) -> ScanObservation {
    ScanObservation {
        source: ScanSource {
            page_index: 73,
            raster: RasterArtifact {
                artifact_id: "raster".into(),
                sha256: HASH.into(),
                media_type: "image/x-portable-graymap".into(),
                width_px: 200,
                height_px: 300,
            },
        },
        producer: ProducerIdentity {
            name: "oracle".into(),
            version: "1".into(),
            run_id: "renderer-layout".into(),
        },
        raster_frame: RasterFrame {
            id: "raster-px".into(),
            extent: (200, 300),
        },
        page_mapping,
        provenance: vec![ProvenanceRecord {
            id: "p".into(),
            stage: ProvenanceStage::ManualAnnotation,
            tool_name: "independent-oracle".into(),
            tool_version: "1".into(),
            model_identifier: "none".into(),
            method: "fixture".into(),
            parameters_sha256: HASH.into(),
            input_artifact_ids: vec!["raster".into()],
            parent_provenance_ids: Vec::new(),
        }],
        units,
        diagnostics: Vec::new(),
    }
}

fn region(id: &str, order: Option<u32>) -> ObservationUnit {
    ObservationUnit {
        id: id.into(),
        kind: ObservationKind::Region,
        parent_id: None,
        reading_order: order.map_or_else(unknown, known),
        text: unknown(),
        span_in_parent: unknown(),
        geometry: GeometryClaims {
            bbox: unknown(),
            polygon: unknown(),
            baseline: unknown(),
        },
    }
}

fn line(
    id: &str,
    parent: Option<&str>,
    order: Option<u32>,
    bbox: Option<[f64; 4]>,
) -> ObservationUnit {
    ObservationUnit {
        id: id.into(),
        kind: ObservationKind::Line,
        parent_id: parent.map(str::to_owned),
        reading_order: order.map_or_else(unknown, known),
        text: unknown(),
        span_in_parent: unknown(),
        geometry: GeometryClaims {
            bbox: bbox.map_or_else(unknown, |v| {
                known(FramedBbox {
                    frame_id: "raster-px".into(),
                    x0: v[0],
                    y0: v[1],
                    x1: v[2],
                    y1: v[3],
                })
            }),
            polygon: unknown(),
            baseline: unknown(),
        },
    }
}

fn derive(observation: &ScanObservation) -> ScanLayoutProfile {
    derive_scan_layout(observation)
        .expect("a fixture de comportamento deve ser uma observacao valida")
}

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Record(Vec<(String, Node)>),
    Tuple(Vec<Node>),
    String(String),
    Number(String),
    Bool(bool),
}

struct TypstParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> TypstParser<'a> {
    fn parse(source: &'a str) -> Result<Node, String> {
        let mut parser = Self {
            bytes: source.as_bytes(),
            pos: 0,
        };
        parser.take(b"#let scan_layout_profile = ")?;
        let node = parser.paren(0)?;
        parser.take(b"\n")?;
        if parser.pos != parser.bytes.len() {
            return Err("bytes depois da unica declaracao".into());
        }
        Ok(node)
    }

    fn paren(&mut self, depth: usize) -> Result<Node, String> {
        self.take(b"(")?;
        if self.peek() == Some(b')') {
            self.pos += 1;
            return Ok(Node::Tuple(Vec::new()));
        }
        self.take(b"\n")?;
        self.indent(depth + 1)?;
        let record = self.looks_like_member();
        let mut fields = Vec::new();
        let mut values = Vec::new();
        loop {
            if record {
                let key = self.identifier()?;
                self.take(b": ")?;
                fields.push((key, self.value(depth + 1)?));
            } else {
                values.push(self.value(depth + 1)?);
            }
            self.take(b",\n")?;
            let saved = self.pos;
            if self.indent(depth).is_ok() && self.peek() == Some(b')') {
                self.pos += 1;
                break;
            }
            self.pos = saved;
            self.indent(depth + 1)?;
        }
        if record {
            Ok(Node::Record(fields))
        } else {
            Ok(Node::Tuple(values))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Node, String> {
        match self.peek() {
            Some(b'(') => self.paren(depth),
            Some(b'"') => self.string().map(Node::String),
            Some(b'-' | b'0'..=b'9') => self.number().map(Node::Number),
            Some(b't') => {
                self.take(b"true")?;
                Ok(Node::Bool(true))
            }
            Some(b'f') => {
                self.take(b"false")?;
                Ok(Node::Bool(false))
            }
            _ => Err(format!("valor nao literal no byte {}", self.pos)),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.take(b"\"")?;
        let mut result = String::new();
        while self.peek() != Some(b'"') {
            if self.peek().is_none() {
                return Err("string sem fechamento".into());
            }
            if self.peek() == Some(b'\\') {
                self.pos += 1;
                match self.peek().ok_or("escape truncado")? {
                    b'"' => {
                        result.push('"');
                        self.pos += 1;
                    }
                    b'\\' => {
                        result.push('\\');
                        self.pos += 1;
                    }
                    b'n' => {
                        result.push('\n');
                        self.pos += 1;
                    }
                    b'r' => {
                        result.push('\r');
                        self.pos += 1;
                    }
                    b't' => {
                        result.push('\t');
                        self.pos += 1;
                    }
                    b'u' => {
                        self.pos += 1;
                        self.take(b"{")?;
                        let start = self.pos;
                        while self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
                            self.pos += 1;
                        }
                        let value = u32::from_str_radix(
                            std::str::from_utf8(&self.bytes[start..self.pos]).unwrap(),
                            16,
                        )
                        .map_err(|_| "escape unicode invalido")?;
                        self.take(b"}")?;
                        result.push(char::from_u32(value).ok_or("escalar unicode invalido")?);
                    }
                    _ => return Err("escape Typst fora do contrato".into()),
                }
            } else {
                let rest =
                    std::str::from_utf8(&self.bytes[self.pos..]).map_err(|_| "UTF-8 invalido")?;
                let ch = rest.chars().next().unwrap();
                if ch.is_control() {
                    return Err("controle cru em string Typst".into());
                }
                result.push(ch);
                self.pos += ch.len_utf8();
            }
        }
        self.pos += 1;
        Ok(result)
    }

    fn number(&mut self) -> Result<String, String> {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'-' | b'.' | b'e'))
        {
            self.pos += 1;
        }
        let value = std::str::from_utf8(&self.bytes[start..self.pos])
            .unwrap()
            .to_owned();
        value
            .parse::<f64>()
            .map_err(|_| format!("numero invalido {value}"))?;
        Ok(value)
    }

    fn identifier(&mut self) -> Result<String, String> {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            self.pos += 1;
        }
        if start == self.pos {
            return Err("identificador ausente".into());
        }
        Ok(std::str::from_utf8(&self.bytes[start..self.pos])
            .unwrap()
            .to_owned())
    }

    fn looks_like_member(&self) -> bool {
        let mut pos = self.pos;
        while self
            .bytes
            .get(pos)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            pos += 1;
        }
        pos > self.pos && self.bytes.get(pos..pos + 2) == Some(b": ")
    }

    fn indent(&mut self, depth: usize) -> Result<(), String> {
        for _ in 0..depth * 2 {
            self.take(b" ")?;
        }
        Ok(())
    }

    fn take(&mut self, expected: &[u8]) -> Result<(), String> {
        if self.bytes.get(self.pos..self.pos + expected.len()) != Some(expected) {
            return Err(format!("esperado {:?} no byte {}", expected, self.pos));
        }
        self.pos += expected.len();
        Ok(())
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }
}

fn record(node: &Node) -> Result<&[(String, Node)], String> {
    match node {
        Node::Record(fields) => Ok(fields),
        _ => Err("record esperado".into()),
    }
}

fn field<'a>(node: &'a Node, name: &str) -> Result<&'a Node, String> {
    record(node)?
        .iter()
        .find_map(|(key, value)| (key == name).then_some(value))
        .ok_or_else(|| format!("campo ausente: {name}"))
}

fn string_value(node: &Node) -> Result<&str, String> {
    match node {
        Node::String(value) => Ok(value),
        _ => Err("string esperada".into()),
    }
}

fn exact_keys(node: &Node, expected: &[&str]) -> Result<(), String> {
    let actual: Vec<_> = record(node)?.iter().map(|(key, _)| key.as_str()).collect();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("chaves/ordem divergentes: {actual:?}"))
    }
}

fn measurement(node: &Node) -> Result<(&str, Option<&Node>, Option<&str>), String> {
    let status = string_value(field(node, "status")?)?;
    match status {
        "derived" => {
            exact_keys(node, &["status", "value"])?;
            Ok((status, Some(field(node, "value")?), None))
        }
        "unknown" => {
            exact_keys(node, &["status", "reason"])?;
            Ok((status, None, Some(string_value(field(node, "reason")?)?)))
        }
        _ => Err("status de medicao invalido".into()),
    }
}

fn tuple_len(node: &Node) -> Result<usize, String> {
    match node {
        Node::Tuple(values) => Ok(values.len()),
        _ => Err("tuple esperada".into()),
    }
}

fn validate_typst_schema(root: &Node) -> Result<(), String> {
    exact_keys(
        root,
        &[
            "schema",
            "schema_version",
            "source",
            "coverage",
            "page",
            "observed_text_envelope",
            "observed_text_insets",
            "regions",
        ],
    )?;
    if string_value(field(root, "schema")?)? != "decalque.scan-layout-profile" {
        return Err("schema divergente".into());
    }
    exact_keys(field(root, "source")?, &["page_index", "raster_sha256"])?;
    exact_keys(
        field(root, "coverage")?,
        &["total_lines", "projected_lines"],
    )?;
    if measurement(field(field(root, "coverage")?, "projected_lines")?)?.0 != "derived" {
        return Err("Typst nao admite cobertura unknown".into());
    }
    let page = field(root, "page")?;
    exact_keys(page, &["status", "frame_id", "extent_pt", "max_error_pt"])?;
    if string_value(field(page, "status")?)? != "derived" {
        return Err("Typst nao admite pagina unknown".into());
    }
    for name in ["observed_text_envelope", "observed_text_insets"] {
        let (status, _, reason) = measurement(field(root, name)?)?;
        if status == "unknown"
            && !matches!(
                reason,
                Some("no-lines" | "incomplete-line-bbox" | "projection-failed")
            )
        {
            return Err(format!("razao global invalida em {name}"));
        }
    }
    let regions = match field(root, "regions")? {
        Node::Tuple(values) => values,
        _ => return Err("regions nao e tuple".into()),
    };
    for region in regions {
        exact_keys(
            region,
            &[
                "region_id",
                "line_support_ids",
                "observed_line_envelope",
                "line_starts_pt",
                "signed_line_box_gaps_pt",
                "first_line_start_delta_pt",
            ],
        )?;
        let n = tuple_len(field(region, "line_support_ids")?)?;
        let envelope = measurement(field(region, "observed_line_envelope")?)?;
        let starts = measurement(field(region, "line_starts_pt")?)?;
        let gaps = measurement(field(region, "signed_line_box_gaps_pt")?)?;
        let delta = measurement(field(region, "first_line_start_delta_pt")?)?;
        if envelope.0 == "unknown" {
            let reason = envelope.2.unwrap();
            if !matches!(
                reason,
                "no-lines"
                    | "incomplete-line-reading-order"
                    | "incomplete-line-bbox"
                    | "projection-failed"
            ) || starts.2 != Some(reason)
                || gaps.2 != Some(reason)
                || delta.2 != Some(reason)
                || (reason == "no-lines") != (n == 0)
            {
                return Err("RI/R0 com razoes ou suporte divergentes".into());
            }
        } else {
            if n == 0 || starts.0 != "derived" || gaps.0 != "derived" {
                return Err("estado derivado regional invalido".into());
            }
            if tuple_len(starts.1.unwrap())? != n || tuple_len(gaps.1.unwrap())? != n - 1 {
                return Err("cardinalidade regional invalida".into());
            }
            if n == 1 {
                if delta.0 != "unknown" || delta.2 != Some("insufficient-support") {
                    return Err("delta R1 invalido".into());
                }
            } else if delta.0 != "derived" {
                return Err("delta RN invalido".into());
            }
        }
    }
    Ok(())
}

fn exact_fixture() -> ScanObservation {
    observation(
        mapping(FRAME),
        vec![
            region(REGION, Some(0)),
            line(
                "line-1",
                Some(REGION),
                Some(0),
                Some([10.0, 20.0, 50.0, 30.0]),
            ),
            line(
                "line-2",
                Some(REGION),
                Some(1),
                Some([8.0, 34.0, 60.0, 44.0]),
            ),
        ],
    )
}

const EXPECTED_TYPST: &str = concat!(
    "#let scan_layout_profile = (\n",
    "  schema: \"decalque.scan-layout-profile\",\n",
    "  schema_version: 1,\n",
    "  source: (\n",
    "    page_index: 73,\n",
    "    raster_sha256: \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\n",
    "  ),\n",
    "  coverage: (\n",
    "    total_lines: 2,\n",
    "    projected_lines: (\n",
    "      status: \"derived\",\n",
    "      value: 2,\n",
    "    ),\n",
    "  ),\n",
    "  page: (\n",
    "    status: \"derived\",\n",
    "    frame_id: \"frame\\\"\\\\\\n\\u{1}\\u{7f}e\",\n",
    "    extent_pt: (\n",
    "      width: 200.0,\n",
    "      height: 300.0,\n",
    "    ),\n",
    "    max_error_pt: 0.0,\n",
    "  ),\n",
    "  observed_text_envelope: (\n",
    "    status: \"derived\",\n",
    "    value: (\n",
    "      x0_pt: 8.0,\n",
    "      y0_pt: 20.0,\n",
    "      x1_pt: 60.0,\n",
    "      y1_pt: 44.0,\n",
    "    ),\n",
    "  ),\n",
    "  observed_text_insets: (\n",
    "    status: \"derived\",\n",
    "    value: (\n",
    "      top_pt: 20.0,\n",
    "      left_pt: 8.0,\n",
    "      right_pt: 140.0,\n",
    "      bottom_pt: 256.0,\n",
    "    ),\n",
    "  ),\n",
    "  regions: (\n",
    "    (\n",
    "      region_id: \"region\\\"\\\\\\t\\u{2}\\u{7f}c\",\n",
    "      line_support_ids: (\n",
    "        \"line-1\",\n",
    "        \"line-2\",\n",
    "      ),\n",
    "      observed_line_envelope: (\n",
    "        status: \"derived\",\n",
    "        value: (\n",
    "          x0_pt: 8.0,\n",
    "          y0_pt: 20.0,\n",
    "          x1_pt: 60.0,\n",
    "          y1_pt: 44.0,\n",
    "        ),\n",
    "      ),\n",
    "      line_starts_pt: (\n",
    "        status: \"derived\",\n",
    "        value: (\n",
    "          10.0,\n",
    "          8.0,\n",
    "        ),\n",
    "      ),\n",
    "      signed_line_box_gaps_pt: (\n",
    "        status: \"derived\",\n",
    "        value: (\n",
    "          4.0,\n",
    "        ),\n",
    "      ),\n",
    "      first_line_start_delta_pt: (\n",
    "        status: \"derived\",\n",
    "        value: 2.0,\n",
    "      ),\n",
    "    ),\n",
    "  ),\n",
    ")\n",
);

const EXPECTED_JSON_PREFIX: &str = concat!(
    "{\"schema\":\"decalque.scan-layout-profile\",\"schema_version\":1,",
    "\"source\":{\"page_index\":73,\"raster_sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"},",
    "\"coverage\":{\"total_lines\":2,\"projected_lines\":{\"status\":\"derived\",\"value\":2}},",
    "\"page\":{\"status\":\"derived\",\"frame_id\":\"frame\\\"\\\\\\n\\u0001\u{7f}e\",",
    "\"extent_pt\":{\"width\":200.0,\"height\":300.0},\"max_error_pt\":0.0},",
    "\"observed_text_envelope\":{\"status\":\"derived\",\"value\":{\"x0_pt\":8.0,\"y0_pt\":20.0,\"x1_pt\":60.0,\"y1_pt\":44.0}},",
    "\"observed_text_insets\":{\"status\":\"derived\",\"value\":{\"top_pt\":20.0,\"left_pt\":8.0,\"right_pt\":140.0,\"bottom_pt\":256.0}},",
    "\"regions\":[{\"region_id\":\"region\\\"\\\\\\t\\u0002\u{7f}c\",\"line_support_ids\":[\"line-1\",\"line-2\"],",
    "\"observed_line_envelope\":{\"status\":\"derived\",\"value\":{\"x0_pt\":8.0,\"y0_pt\":20.0,\"x1_pt\":60.0,\"y1_pt\":44.0}},",
    "\"line_starts_pt\":{\"status\":\"derived\",\"value\":[10.0,8.0]},",
    "\"signed_line_box_gaps_pt\":{\"status\":\"derived\",\"value\":[4.0]},",
    "\"first_line_start_delta_pt\":{\"status\":\"derived\",\"value\":2.0}}],",
    "\"typst_artifact\":{\"status\":\"available\",\"sha256\":\"",
);

const EXPECTED_UNKNOWN_JSON: &str = concat!(
    "{\"schema\":\"decalque.scan-layout-profile\",\"schema_version\":1,",
    "\"source\":{\"page_index\":73,\"raster_sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"},",
    "\"coverage\":{\"total_lines\":0,\"projected_lines\":{\"status\":\"unknown\",\"reason\":\"page-mapping-unknown\"}},",
    "\"page\":{\"status\":\"unknown\",\"reason\":\"page-mapping-unknown\"},",
    "\"observed_text_envelope\":{\"status\":\"unknown\",\"reason\":\"page-mapping-unknown\"},",
    "\"observed_text_insets\":{\"status\":\"unknown\",\"reason\":\"page-mapping-unknown\"},",
    "\"regions\":[],\"typst_artifact\":{\"status\":\"unavailable\",\"sha256\":null,\"size_bytes\":null,\"published\":false}}",
);

#[test]
fn derived_bytes_schema_escaping_hash_size_and_reports_are_exact() {
    let profile = derive(&exact_fixture());
    let rendered = render_scan_layout_profile(&profile);
    assert!(std::ptr::eq(rendered.profile(), &profile));
    let artifact = rendered.artifact().expect("pagina derived cria artefato");

    assert_eq!(artifact.source(), EXPECTED_TYPST);
    assert_eq!(artifact.source_bytes(), EXPECTED_TYPST.as_bytes());
    assert_eq!(artifact.size_bytes(), EXPECTED_TYPST.len());
    assert_eq!(artifact.sha256(), sha256_hex(EXPECTED_TYPST.as_bytes()));
    assert!(!artifact.source().contains("-0.0"));

    let expected_unpublished = format!(
        "{}{}\",\"size_bytes\":{},\"published\":false}}}}",
        EXPECTED_JSON_PREFIX,
        artifact.sha256(),
        artifact.size_bytes()
    );
    let expected_published = format!(
        "{}{}\",\"size_bytes\":{},\"published\":true}}}}",
        EXPECTED_JSON_PREFIX,
        artifact.sha256(),
        artifact.size_bytes()
    );
    assert_eq!(rendered.unpublished_report(), expected_unpublished);
    assert_eq!(
        rendered.published_report(),
        Some(expected_published.as_str())
    );
    assert_eq!(
        TypstParser::parse(artifact.source()).and_then(|ast| validate_typst_schema(&ast)),
        Ok(())
    );
}

#[test]
fn unknown_page_is_success_without_artifact_or_published_report() {
    let profile = derive(&observation(unknown(), Vec::new()));
    assert_eq!(profile.source().page_index(), 73);
    assert_eq!(profile.source().raster_sha256(), HASH);
    assert_eq!(profile.coverage().total_lines(), 0);
    let projected = profile.coverage().projected_lines();
    assert_eq!(projected.status(), "unknown");
    assert_eq!(projected.reason(), Some("page-mapping-unknown"));
    assert_eq!(projected.value(), None);
    let page = profile.page();
    assert_eq!(page.status(), "unknown");
    assert_eq!(page.reason(), Some("page-mapping-unknown"));
    assert!(page.extent_pt().is_none());
    let envelope = profile.observed_text_envelope();
    assert_eq!(envelope.status(), "unknown");
    assert_eq!(envelope.reason(), Some("page-mapping-unknown"));
    assert!(envelope.value().is_none());
    let insets = profile.observed_text_insets();
    assert_eq!(insets.status(), "unknown");
    assert_eq!(insets.reason(), Some("page-mapping-unknown"));
    assert!(insets.value().is_none());
    assert!(profile.regions().is_empty());
    let rendered = render_scan_layout_profile(&profile);
    assert!(rendered.artifact().is_none());
    assert!(rendered.published_report().is_none());
    assert_eq!(rendered.unpublished_report(), EXPECTED_UNKNOWN_JSON);
}

#[test]
fn unknown_page_overrides_zero_one_and_many_line_states() {
    let fixtures = vec![
        Vec::new(),
        vec![line("top", None, Some(0), Some([1.0, 2.0, 3.0, 4.0]))],
        vec![
            region("r", Some(0)),
            line("a", Some("r"), None, None),
            line("b", Some("r"), Some(1), Some([2.0, 3.0, 4.0, 5.0])),
        ],
    ];
    for units in fixtures {
        let expected_total = units
            .iter()
            .filter(|unit| unit.kind == ObservationKind::Line)
            .count();
        let profile = derive(&observation(unknown(), units));
        assert_eq!(profile.coverage().total_lines(), expected_total);
        assert_eq!(
            profile.coverage().projected_lines().reason(),
            Some("page-mapping-unknown")
        );
        assert_eq!(
            profile.observed_text_envelope().reason(),
            Some("page-mapping-unknown")
        );
        let regions = profile.regions();
        for index in 0..regions.len() {
            let region = regions.get(index).unwrap();
            assert_eq!(
                region.observed_line_envelope().reason(),
                Some("page-mapping-unknown")
            );
            assert_eq!(
                region.line_starts_pt().reason(),
                Some("page-mapping-unknown")
            );
            assert_eq!(
                region.signed_line_box_gaps_pt().reason(),
                Some("page-mapping-unknown")
            );
            assert_eq!(
                region.first_line_start_delta_pt().reason(),
                Some("page-mapping-unknown")
            );
        }
        let rendered = render_scan_layout_profile(&profile);
        assert!(rendered.artifact().is_none());
        assert!(rendered.published_report().is_none());
    }
}

#[test]
fn semantically_equal_input_permutations_render_identical_bytes() {
    let units = vec![
        region("z", Some(20)),
        line("z-line", Some("z"), Some(0), Some([10.0, 10.0, 20.0, 20.0])),
        region("y", Some(10)),
        line("y-line", Some("y"), Some(0), Some([30.0, 30.0, 40.0, 40.0])),
        region("e", None),
        line("e-line", Some("e"), Some(0), Some([50.0, 50.0, 60.0, 60.0])),
        region("a", None),
        line("a-line", Some("a"), Some(0), Some([70.0, 70.0, 80.0, 80.0])),
    ];
    let mut reversed = units.clone();
    reversed.reverse();
    let first = derive(&observation(mapping("page"), units));
    let second = derive(&observation(mapping("page"), reversed));
    let first = render_scan_layout_profile(&first);
    let second = render_scan_layout_profile(&second);
    assert_eq!(first.unpublished_report(), second.unpublished_report());
    assert_eq!(first.published_report(), second.published_report());
    assert_eq!(
        first.artifact().unwrap().source_bytes(),
        second.artifact().unwrap().source_bytes()
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut data = bytes.to_vec();
    let bit_len = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    let mut h = [
        0x6a09e667_u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in data.chunks_exact(64) {
        let mut w = [0_u32; 64];
        for (index, word) in chunk.chunks_exact(4).enumerate() {
            w[index] = u32::from_be_bytes(word.try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (state, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *state = state.wrapping_add(value);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}

#[test]
fn independent_sha256_oracle_has_a_known_vector() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
