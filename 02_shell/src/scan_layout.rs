use decalque_core::{
    ScanLayoutBoxMeasurementView, ScanLayoutBoxView, ScanLayoutInsetsMeasurementView,
    ScanLayoutNumberListMeasurementView, ScanLayoutProfile, ScanLayoutRegionView,
    ScanLayoutScalarMeasurementView, ScanLayoutStringListView,
};
use sha2::{Digest, Sha256};
use std::fmt::Write;

pub struct RenderedScanLayoutTypst {
    source: String,
    source_bytes: Vec<u8>,
    sha256: String,
    size_bytes: usize,
}

impl RenderedScanLayoutTypst {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn source_bytes(&self) -> &[u8] {
        &self.source_bytes
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    pub fn size_bytes(&self) -> usize {
        self.size_bytes
    }
}

pub struct RenderedScanLayoutProfile<'a> {
    profile: &'a ScanLayoutProfile,
    artifact: Option<RenderedScanLayoutTypst>,
    unpublished_report: String,
    published_report: Option<String>,
}

impl<'a> RenderedScanLayoutProfile<'a> {
    pub fn profile(&self) -> &'a ScanLayoutProfile {
        self.profile
    }

    pub fn artifact(&self) -> Option<&RenderedScanLayoutTypst> {
        self.artifact.as_ref()
    }

    pub fn unpublished_report(&self) -> &str {
        &self.unpublished_report
    }

    pub fn published_report(&self) -> Option<&str> {
        self.published_report.as_deref()
    }
}

pub fn render_scan_layout_profile<'a>(
    profile: &'a ScanLayoutProfile,
) -> RenderedScanLayoutProfile<'a> {
    let artifact = if profile.page().status() == "derived" {
        let source = render_typst(profile);
        let source_bytes = source.as_bytes().to_vec();
        let sha256 = sha256_hex(&source_bytes);
        let size_bytes = source_bytes.len();
        Some(RenderedScanLayoutTypst {
            source,
            source_bytes,
            sha256,
            size_bytes,
        })
    } else {
        None
    };
    let unpublished_report = render_json(profile, artifact.as_ref(), false);
    let published_report = artifact
        .as_ref()
        .map(|artifact| render_json(profile, Some(artifact), true));

    RenderedScanLayoutProfile {
        profile,
        artifact,
        unpublished_report,
        published_report,
    }
}

fn render_json(
    profile: &ScanLayoutProfile,
    artifact: Option<&RenderedScanLayoutTypst>,
    published: bool,
) -> String {
    let mut json = String::new();
    json.push_str("{\"schema\":\"decalque.scan-layout-profile\",\"schema_version\":1");
    json.push_str(",\"source\":{\"page_index\":");
    write!(json, "{}", profile.source().page_index()).unwrap();
    json.push_str(",\"raster_sha256\":");
    push_json_string(&mut json, profile.source().raster_sha256());
    json.push('}');

    let coverage = profile.coverage();
    json.push_str(",\"coverage\":{\"total_lines\":");
    write!(json, "{}", coverage.total_lines()).unwrap();
    json.push_str(",\"projected_lines\":");
    let projected = coverage.projected_lines();
    if let Some(value) = projected.value() {
        json.push_str("{\"status\":\"derived\",\"value\":");
        write!(json, "{value}").unwrap();
        json.push('}');
    } else {
        json.push_str("{\"status\":\"unknown\",\"reason\":");
        push_json_string(
            &mut json,
            projected.reason().expect("unknown count has a reason"),
        );
        json.push('}');
    }
    json.push('}');

    json.push_str(",\"page\":");
    let page = profile.page();
    if page.status() == "derived" {
        let extent = page.extent_pt().expect("derived page has an extent");
        json.push_str("{\"status\":\"derived\",\"frame_id\":");
        push_json_string(
            &mut json,
            page.frame_id().expect("derived page has a frame id"),
        );
        json.push_str(",\"extent_pt\":{\"width\":");
        json.push_str(&canonical_number(extent.width()));
        json.push_str(",\"height\":");
        json.push_str(&canonical_number(extent.height()));
        json.push_str("},\"max_error_pt\":");
        json.push_str(&canonical_number(
            page.max_error_pt().expect("derived page has max error"),
        ));
        json.push('}');
    } else {
        json.push_str("{\"status\":\"unknown\",\"reason\":");
        push_json_string(&mut json, page.reason().expect("unknown page has a reason"));
        json.push('}');
    }

    json.push_str(",\"observed_text_envelope\":");
    push_json_box_measurement(&mut json, profile.observed_text_envelope());
    json.push_str(",\"observed_text_insets\":");
    push_json_insets_measurement(&mut json, profile.observed_text_insets());

    json.push_str(",\"regions\":[");
    let regions = profile.regions();
    for index in 0..regions.len() {
        if index != 0 {
            json.push(',');
        }
        push_json_region(
            &mut json,
            regions.get(index).expect("region index is in bounds"),
        );
    }
    json.push(']');

    json.push_str(",\"typst_artifact\":");
    if let Some(artifact) = artifact {
        json.push_str("{\"status\":\"available\",\"sha256\":");
        push_json_string(&mut json, artifact.sha256());
        json.push_str(",\"size_bytes\":");
        write!(json, "{}", artifact.size_bytes()).unwrap();
        json.push_str(",\"published\":");
        json.push_str(if published { "true" } else { "false" });
        json.push('}');
    } else {
        json.push_str(
            "{\"status\":\"unavailable\",\"sha256\":null,\"size_bytes\":null,\"published\":false}",
        );
    }
    json.push('}');
    json
}

fn push_json_region(json: &mut String, region: ScanLayoutRegionView<'_>) {
    json.push_str("{\"region_id\":");
    push_json_string(json, region.region_id());
    json.push_str(",\"line_support_ids\":");
    push_json_string_list(json, region.line_support_ids());
    json.push_str(",\"observed_line_envelope\":");
    push_json_box_measurement(json, region.observed_line_envelope());
    json.push_str(",\"line_starts_pt\":");
    push_json_number_list_measurement(json, region.line_starts_pt());
    json.push_str(",\"signed_line_box_gaps_pt\":");
    push_json_number_list_measurement(json, region.signed_line_box_gaps_pt());
    json.push_str(",\"first_line_start_delta_pt\":");
    push_json_scalar_measurement(json, region.first_line_start_delta_pt());
    json.push('}');
}

fn push_json_box_measurement(json: &mut String, measurement: ScanLayoutBoxMeasurementView<'_>) {
    if let Some(value) = measurement.value() {
        json.push_str("{\"status\":\"derived\",\"value\":");
        push_json_box(json, value);
        json.push('}');
    } else {
        json.push_str("{\"status\":\"unknown\",\"reason\":");
        push_json_string(
            json,
            measurement
                .reason()
                .expect("unknown box measurement has a reason"),
        );
        json.push('}');
    }
}

fn push_json_insets_measurement(
    json: &mut String,
    measurement: ScanLayoutInsetsMeasurementView<'_>,
) {
    if let Some(value) = measurement.value() {
        json.push_str("{\"status\":\"derived\",\"value\":{\"top_pt\":");
        json.push_str(&canonical_number(value.top_pt()));
        json.push_str(",\"left_pt\":");
        json.push_str(&canonical_number(value.left_pt()));
        json.push_str(",\"right_pt\":");
        json.push_str(&canonical_number(value.right_pt()));
        json.push_str(",\"bottom_pt\":");
        json.push_str(&canonical_number(value.bottom_pt()));
        json.push_str("}}");
    } else {
        json.push_str("{\"status\":\"unknown\",\"reason\":");
        push_json_string(
            json,
            measurement
                .reason()
                .expect("unknown insets measurement has a reason"),
        );
        json.push('}');
    }
}

fn push_json_number_list_measurement(
    json: &mut String,
    measurement: ScanLayoutNumberListMeasurementView<'_>,
) {
    if measurement.status() == "derived" {
        json.push_str("{\"status\":\"derived\",\"value\":[");
        for index in 0..measurement.len() {
            if index != 0 {
                json.push(',');
            }
            json.push_str(&canonical_number(
                measurement
                    .get(index)
                    .expect("derived list index is in bounds"),
            ));
        }
        json.push_str("]}");
    } else {
        json.push_str("{\"status\":\"unknown\",\"reason\":");
        push_json_string(
            json,
            measurement
                .reason()
                .expect("unknown list measurement has a reason"),
        );
        json.push('}');
    }
}

fn push_json_scalar_measurement(
    json: &mut String,
    measurement: ScanLayoutScalarMeasurementView<'_>,
) {
    if let Some(value) = measurement.value() {
        json.push_str("{\"status\":\"derived\",\"value\":");
        json.push_str(&canonical_number(value));
        json.push('}');
    } else {
        json.push_str("{\"status\":\"unknown\",\"reason\":");
        push_json_string(
            json,
            measurement
                .reason()
                .expect("unknown scalar measurement has a reason"),
        );
        json.push('}');
    }
}

fn push_json_box(json: &mut String, value: ScanLayoutBoxView) {
    json.push_str("{\"x0_pt\":");
    json.push_str(&canonical_number(value.x0_pt()));
    json.push_str(",\"y0_pt\":");
    json.push_str(&canonical_number(value.y0_pt()));
    json.push_str(",\"x1_pt\":");
    json.push_str(&canonical_number(value.x1_pt()));
    json.push_str(",\"y1_pt\":");
    json.push_str(&canonical_number(value.y1_pt()));
    json.push('}');
}

fn push_json_string_list(json: &mut String, values: ScanLayoutStringListView<'_>) {
    json.push('[');
    for index in 0..values.len() {
        if index != 0 {
            json.push(',');
        }
        push_json_string(
            json,
            values.get(index).expect("string list index is in bounds"),
        );
    }
    json.push(']');
}

fn push_json_string(json: &mut String, value: &str) {
    json.push('"');
    for character in value.chars() {
        match character {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            '\u{00}'..='\u{1f}' => write!(json, "\\u{:04x}", character as u32).unwrap(),
            _ => json.push(character),
        }
    }
    json.push('"');
}

fn render_typst(profile: &ScanLayoutProfile) -> String {
    let mut typst = String::from("#let scan_layout_profile = (\n");
    push_typst_string_field(&mut typst, 1, "schema", "decalque.scan-layout-profile");
    push_typst_integer_field(&mut typst, 1, "schema_version", 1);

    push_typst_record_start(&mut typst, 1, "source");
    push_typst_integer_field(
        &mut typst,
        2,
        "page_index",
        profile.source().page_index() as usize,
    );
    push_typst_string_field(
        &mut typst,
        2,
        "raster_sha256",
        profile.source().raster_sha256(),
    );
    push_typst_record_end(&mut typst, 1);

    let coverage = profile.coverage();
    push_typst_record_start(&mut typst, 1, "coverage");
    push_typst_integer_field(&mut typst, 2, "total_lines", coverage.total_lines());
    push_typst_record_start(&mut typst, 2, "projected_lines");
    push_typst_string_field(&mut typst, 3, "status", "derived");
    push_typst_integer_field(
        &mut typst,
        3,
        "value",
        coverage
            .projected_lines()
            .value()
            .expect("Typst exists only for a derived page"),
    );
    push_typst_record_end(&mut typst, 2);
    push_typst_record_end(&mut typst, 1);

    let page = profile.page();
    let extent = page
        .extent_pt()
        .expect("Typst exists only for a derived page");
    push_typst_record_start(&mut typst, 1, "page");
    push_typst_string_field(&mut typst, 2, "status", "derived");
    push_typst_string_field(
        &mut typst,
        2,
        "frame_id",
        page.frame_id().expect("derived page has a frame id"),
    );
    push_typst_record_start(&mut typst, 2, "extent_pt");
    push_typst_number_field(&mut typst, 3, "width", extent.width());
    push_typst_number_field(&mut typst, 3, "height", extent.height());
    push_typst_record_end(&mut typst, 2);
    push_typst_number_field(
        &mut typst,
        2,
        "max_error_pt",
        page.max_error_pt().expect("derived page has max error"),
    );
    push_typst_record_end(&mut typst, 1);

    push_typst_box_measurement(
        &mut typst,
        1,
        "observed_text_envelope",
        profile.observed_text_envelope(),
    );
    push_typst_insets_measurement(
        &mut typst,
        1,
        "observed_text_insets",
        profile.observed_text_insets(),
    );

    let regions = profile.regions();
    if regions.is_empty() {
        push_indent(&mut typst, 1);
        typst.push_str("regions: (),\n");
    } else {
        push_indent(&mut typst, 1);
        typst.push_str("regions: (\n");
        for index in 0..regions.len() {
            push_indent(&mut typst, 2);
            typst.push_str("(\n");
            push_typst_region(
                &mut typst,
                3,
                regions.get(index).expect("region index is in bounds"),
            );
            push_indent(&mut typst, 2);
            typst.push_str("),\n");
        }
        push_indent(&mut typst, 1);
        typst.push_str("),\n");
    }
    typst.push_str(")\n");
    typst
}

fn push_typst_region(typst: &mut String, level: usize, region: ScanLayoutRegionView<'_>) {
    push_typst_string_field(typst, level, "region_id", region.region_id());
    push_typst_string_tuple_field(typst, level, "line_support_ids", region.line_support_ids());
    push_typst_box_measurement(
        typst,
        level,
        "observed_line_envelope",
        region.observed_line_envelope(),
    );
    push_typst_number_list_measurement(typst, level, "line_starts_pt", region.line_starts_pt());
    push_typst_number_list_measurement(
        typst,
        level,
        "signed_line_box_gaps_pt",
        region.signed_line_box_gaps_pt(),
    );
    push_typst_scalar_measurement(
        typst,
        level,
        "first_line_start_delta_pt",
        region.first_line_start_delta_pt(),
    );
}

fn push_typst_box_measurement(
    typst: &mut String,
    level: usize,
    name: &str,
    measurement: ScanLayoutBoxMeasurementView<'_>,
) {
    push_typst_record_start(typst, level, name);
    if let Some(value) = measurement.value() {
        push_typst_string_field(typst, level + 1, "status", "derived");
        push_typst_record_start(typst, level + 1, "value");
        push_typst_number_field(typst, level + 2, "x0_pt", value.x0_pt());
        push_typst_number_field(typst, level + 2, "y0_pt", value.y0_pt());
        push_typst_number_field(typst, level + 2, "x1_pt", value.x1_pt());
        push_typst_number_field(typst, level + 2, "y1_pt", value.y1_pt());
        push_typst_record_end(typst, level + 1);
    } else {
        push_typst_string_field(typst, level + 1, "status", "unknown");
        push_typst_string_field(
            typst,
            level + 1,
            "reason",
            measurement
                .reason()
                .expect("unknown box measurement has a reason"),
        );
    }
    push_typst_record_end(typst, level);
}

fn push_typst_insets_measurement(
    typst: &mut String,
    level: usize,
    name: &str,
    measurement: ScanLayoutInsetsMeasurementView<'_>,
) {
    push_typst_record_start(typst, level, name);
    if let Some(value) = measurement.value() {
        push_typst_string_field(typst, level + 1, "status", "derived");
        push_typst_record_start(typst, level + 1, "value");
        push_typst_number_field(typst, level + 2, "top_pt", value.top_pt());
        push_typst_number_field(typst, level + 2, "left_pt", value.left_pt());
        push_typst_number_field(typst, level + 2, "right_pt", value.right_pt());
        push_typst_number_field(typst, level + 2, "bottom_pt", value.bottom_pt());
        push_typst_record_end(typst, level + 1);
    } else {
        push_typst_string_field(typst, level + 1, "status", "unknown");
        push_typst_string_field(
            typst,
            level + 1,
            "reason",
            measurement
                .reason()
                .expect("unknown insets measurement has a reason"),
        );
    }
    push_typst_record_end(typst, level);
}

fn push_typst_number_list_measurement(
    typst: &mut String,
    level: usize,
    name: &str,
    measurement: ScanLayoutNumberListMeasurementView<'_>,
) {
    push_typst_record_start(typst, level, name);
    if measurement.status() == "derived" {
        push_typst_string_field(typst, level + 1, "status", "derived");
        push_indent(typst, level + 1);
        if measurement.is_empty() {
            typst.push_str("value: (),\n");
        } else {
            typst.push_str("value: (\n");
            for index in 0..measurement.len() {
                push_indent(typst, level + 2);
                typst.push_str(&canonical_number(
                    measurement
                        .get(index)
                        .expect("derived list index is in bounds"),
                ));
                typst.push_str(",\n");
            }
            push_indent(typst, level + 1);
            typst.push_str("),\n");
        }
    } else {
        push_typst_string_field(typst, level + 1, "status", "unknown");
        push_typst_string_field(
            typst,
            level + 1,
            "reason",
            measurement
                .reason()
                .expect("unknown list measurement has a reason"),
        );
    }
    push_typst_record_end(typst, level);
}

fn push_typst_scalar_measurement(
    typst: &mut String,
    level: usize,
    name: &str,
    measurement: ScanLayoutScalarMeasurementView<'_>,
) {
    push_typst_record_start(typst, level, name);
    if let Some(value) = measurement.value() {
        push_typst_string_field(typst, level + 1, "status", "derived");
        push_typst_number_field(typst, level + 1, "value", value);
    } else {
        push_typst_string_field(typst, level + 1, "status", "unknown");
        push_typst_string_field(
            typst,
            level + 1,
            "reason",
            measurement
                .reason()
                .expect("unknown scalar measurement has a reason"),
        );
    }
    push_typst_record_end(typst, level);
}

fn push_typst_string_tuple_field(
    typst: &mut String,
    level: usize,
    name: &str,
    values: ScanLayoutStringListView<'_>,
) {
    push_indent(typst, level);
    typst.push_str(name);
    if values.is_empty() {
        typst.push_str(": (),\n");
        return;
    }
    typst.push_str(": (\n");
    for index in 0..values.len() {
        push_indent(typst, level + 1);
        push_typst_string(
            typst,
            values.get(index).expect("string list index is in bounds"),
        );
        typst.push_str(",\n");
    }
    push_indent(typst, level);
    typst.push_str("),\n");
}

fn push_typst_record_start(typst: &mut String, level: usize, name: &str) {
    push_indent(typst, level);
    typst.push_str(name);
    typst.push_str(": (\n");
}

fn push_typst_record_end(typst: &mut String, level: usize) {
    push_indent(typst, level);
    typst.push_str("),\n");
}

fn push_typst_string_field(typst: &mut String, level: usize, name: &str, value: &str) {
    push_indent(typst, level);
    typst.push_str(name);
    typst.push_str(": ");
    push_typst_string(typst, value);
    typst.push_str(",\n");
}

fn push_typst_integer_field(typst: &mut String, level: usize, name: &str, value: usize) {
    push_indent(typst, level);
    writeln!(typst, "{name}: {value},").unwrap();
}

fn push_typst_number_field(typst: &mut String, level: usize, name: &str, value: f64) {
    push_indent(typst, level);
    typst.push_str(name);
    typst.push_str(": ");
    typst.push_str(&canonical_number(value));
    typst.push_str(",\n");
}

fn push_typst_string(typst: &mut String, value: &str) {
    typst.push('"');
    for character in value.chars() {
        match character {
            '"' => typst.push_str("\\\""),
            '\\' => typst.push_str("\\\\"),
            '\n' => typst.push_str("\\n"),
            '\r' => typst.push_str("\\r"),
            '\t' => typst.push_str("\\t"),
            '\u{00}'..='\u{1f}' | '\u{7f}' => {
                write!(typst, "\\u{{{:x}}}", character as u32).unwrap()
            }
            _ => typst.push(character),
        }
    }
    typst.push('"');
}

fn push_indent(output: &mut String, level: usize) {
    for _ in 0..level {
        output.push_str("  ");
    }
}

fn canonical_number(value: f64) -> String {
    if value == 0.0 {
        return "0.0".to_string();
    }

    let display = value.to_string();
    if let Some(exponent_index) = display.find(['e', 'E']) {
        let mantissa = &display[..exponent_index];
        let exponent = &display[exponent_index + 1..];
        let (negative, magnitude) = match exponent.strip_prefix('-') {
            Some(magnitude) => (true, magnitude),
            None => (false, exponent.strip_prefix('+').unwrap_or(exponent)),
        };
        let magnitude = magnitude.trim_start_matches('0');
        let magnitude = if magnitude.is_empty() { "0" } else { magnitude };
        let mut canonical = String::with_capacity(display.len());
        canonical.push_str(mantissa);
        canonical.push('e');
        if negative && magnitude != "0" {
            canonical.push('-');
        }
        canonical.push_str(magnitude);
        canonical
    } else if display.contains('.') {
        display
    } else {
        format!("{display}.0")
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(hex, "{byte:02x}").unwrap();
    }
    hex
}
