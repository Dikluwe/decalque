use crate::{
    validate_scan_observation, Claim, FramedBbox, ObservationKind, ObservationUnit, PageMapping,
    ScanObservation,
};
use std::fmt;

pub struct ScanLayoutProfile {
    source: ScanLayoutSource,
    state: ScanLayoutProfileState,
}

pub struct ScanLayoutSource {
    page_index: u32,
    raster_sha256: String,
}

#[derive(Debug)]
pub struct ScanLayoutProfileError {
    detail: String,
}

struct ScanLayoutProfileState {
    page: PageState,
    global: GlobalState,
    regions: Vec<RegionProfile>,
}

enum PageState {
    Unknown,
    Derived(DerivedPage),
}

struct DerivedPage {
    frame_id: String,
    width: f64,
    height: f64,
    max_error_pt: f64,
}

struct NonEmpty<T> {
    first: T,
    rest: Vec<T>,
}

enum GlobalState {
    R0,
    RI {
        support: NonEmpty<GlobalLineSupport>,
        reason: GlobalUnavailableReason,
    },
    R1 {
        line: ProjectedLine,
    },
    RN {
        first: ProjectedLine,
        second: ProjectedLine,
        rest: Vec<ProjectedLine>,
    },
}

enum RegionState {
    R0,
    RI {
        support: NonEmpty<RegionLineSupport>,
        reason: RegionUnavailableReason,
    },
    R1 {
        line: ProjectedRegionLine,
    },
    RN {
        first: ProjectedRegionLine,
        second: ProjectedRegionLine,
        rest: Vec<ProjectedRegionLine>,
    },
}

struct GlobalLineSupport {
    line_id: String,
    projected_bbox: Option<ProjectedBox>,
}

struct RegionLineSupport {
    line_id: String,
    projected_bbox: Option<ProjectedBox>,
}

struct ProjectedLine {
    _line_id: String,
    bbox: ProjectedBox,
}

struct ProjectedRegionLine {
    line_id: String,
    bbox: ProjectedBox,
}

struct RegionProfile {
    region_id: String,
    state: RegionState,
}

#[derive(Clone, Copy)]
struct ProjectedBox {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

#[derive(Clone, Copy)]
enum GlobalUnavailableReason {
    PageMappingUnknown,
    IncompleteLineBbox,
    ProjectionFailed,
}

#[derive(Clone, Copy)]
enum RegionUnavailableReason {
    PageMappingUnknown,
    IncompleteLineReadingOrder,
    IncompleteLineBbox,
    ProjectionFailed,
}

pub struct ScanLayoutCoverageView<'a> {
    page: &'a PageState,
    global: &'a GlobalState,
}

pub struct ScanLayoutPageView<'a> {
    page: &'a PageState,
}

pub struct ScanLayoutExtentView {
    width: f64,
    height: f64,
}

pub struct ScanLayoutBoxView {
    bbox: ProjectedBox,
}

pub struct ScanLayoutInsetsView {
    top: f64,
    left: f64,
    right: f64,
    bottom: f64,
}

pub struct ScanLayoutCountView<'a> {
    page: &'a PageState,
    global: &'a GlobalState,
}

enum BoxMeasurementSource<'a> {
    Global {
        page: &'a PageState,
        global: &'a GlobalState,
    },
    Region {
        page: &'a PageState,
        region: &'a RegionState,
    },
}

pub struct ScanLayoutBoxMeasurementView<'a> {
    source: BoxMeasurementSource<'a>,
}

pub struct ScanLayoutInsetsMeasurementView<'a> {
    page: &'a PageState,
    global: &'a GlobalState,
}

pub struct ScanLayoutScalarMeasurementView<'a> {
    page: &'a PageState,
    region: &'a RegionState,
}

#[derive(Clone, Copy)]
enum NumberListKind {
    Starts,
    Gaps,
}

pub struct ScanLayoutNumberListMeasurementView<'a> {
    page: &'a PageState,
    region: &'a RegionState,
    kind: NumberListKind,
}

pub struct ScanLayoutStringListView<'a> {
    region: &'a RegionState,
}

pub struct ScanLayoutRegionsView<'a> {
    page: &'a PageState,
    regions: &'a [RegionProfile],
}

pub struct ScanLayoutRegionView<'a> {
    page: &'a PageState,
    region: &'a RegionProfile,
}

pub fn derive_scan_layout(
    observation: &ScanObservation,
) -> Result<ScanLayoutProfile, ScanLayoutProfileError> {
    if let Err(error) = validate_scan_observation(observation) {
        return Err(ScanLayoutProfileError {
            detail: error.to_string(),
        });
    }

    let mapping = observation.page_mapping.known_value();
    let page = match mapping {
        Some(mapping) => PageState::Derived(DerivedPage {
            frame_id: mapping.target_frame.id.clone(),
            width: mapping.target_frame.extent.0,
            height: mapping.target_frame.extent.1,
            max_error_pt: mapping.max_error_pt,
        }),
        None => PageState::Unknown,
    };

    let lines: Vec<&ObservationUnit> = observation
        .units
        .iter()
        .filter(|unit| unit.kind == ObservationKind::Line)
        .collect();
    let global = derive_global_state(&lines, mapping);
    let regions = derive_regions(observation, mapping);

    Ok(ScanLayoutProfile {
        source: ScanLayoutSource {
            page_index: observation.source.page_index,
            raster_sha256: observation.source.raster.sha256.clone(),
        },
        state: ScanLayoutProfileState {
            page,
            global,
            regions,
        },
    })
}

fn derive_global_state(lines: &[&ObservationUnit], mapping: Option<&PageMapping>) -> GlobalState {
    if lines.is_empty() {
        return GlobalState::R0;
    }

    let Some(mapping) = mapping else {
        return GlobalState::RI {
            support: nonempty(
                lines
                    .iter()
                    .map(|line| GlobalLineSupport {
                        line_id: line.id.clone(),
                        projected_bbox: None,
                    })
                    .collect(),
            ),
            reason: GlobalUnavailableReason::PageMappingUnknown,
        };
    };

    let mut incomplete_bbox = false;
    let mut projection_failed = false;
    let mut support = Vec::with_capacity(lines.len());
    for line in lines {
        let projected_bbox = match &line.geometry.bbox {
            Claim::Known { value, .. } => {
                let projected = project_bbox(mapping, value);
                projection_failed |= projected.is_none();
                projected
            }
            Claim::Unknown { .. } => {
                incomplete_bbox = true;
                None
            }
        };
        support.push(GlobalLineSupport {
            line_id: line.id.clone(),
            projected_bbox,
        });
    }

    if incomplete_bbox || projection_failed {
        return GlobalState::RI {
            support: nonempty(support),
            reason: if incomplete_bbox {
                GlobalUnavailableReason::IncompleteLineBbox
            } else {
                GlobalUnavailableReason::ProjectionFailed
            },
        };
    }

    let projected: Vec<ProjectedLine> = support
        .iter()
        .map(|line| ProjectedLine {
            _line_id: line.line_id.clone(),
            bbox: line.projected_bbox.expect("complete global support"),
        })
        .collect();
    let envelope = envelope(projected.iter().map(|line| line.bbox));
    let (page_width, page_height) = mapping.target_frame.extent;
    let insets = [
        envelope.y0,
        envelope.x0,
        page_width - envelope.x1,
        page_height - envelope.y1,
    ];
    if insets.iter().any(|value| !value.is_finite()) {
        return GlobalState::RI {
            support: nonempty(support),
            reason: GlobalUnavailableReason::ProjectionFailed,
        };
    }

    complete_global_state(projected)
}

fn derive_regions(
    observation: &ScanObservation,
    mapping: Option<&PageMapping>,
) -> Vec<RegionProfile> {
    let mut regions: Vec<&ObservationUnit> = observation
        .units
        .iter()
        .filter(|unit| unit.kind == ObservationKind::Region)
        .collect();
    regions.sort_by(|left, right| {
        match (
            left.reading_order.known_value(),
            right.reading_order.known_value(),
        ) {
            (Some(left), Some(right)) => left.cmp(right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.id.as_bytes().cmp(right.id.as_bytes()),
        }
    });

    regions
        .into_iter()
        .map(|region| RegionProfile {
            region_id: region.id.clone(),
            state: derive_region_state(observation, region, mapping),
        })
        .collect()
}

fn derive_region_state(
    observation: &ScanObservation,
    region: &ObservationUnit,
    mapping: Option<&PageMapping>,
) -> RegionState {
    let mut lines: Vec<&ObservationUnit> = observation
        .units
        .iter()
        .filter(|unit| {
            unit.kind == ObservationKind::Line && unit.parent_id.as_deref() == Some(&region.id)
        })
        .collect();
    if lines.is_empty() {
        return RegionState::R0;
    }

    let incomplete_reading_order = lines
        .iter()
        .any(|line| line.reading_order.known_value().is_none());
    if incomplete_reading_order {
        lines.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    } else {
        lines.sort_by_key(|line| *line.reading_order.known_value().expect("known order"));
    }

    let Some(mapping) = mapping else {
        return RegionState::RI {
            support: nonempty(
                lines
                    .iter()
                    .map(|line| RegionLineSupport {
                        line_id: line.id.clone(),
                        projected_bbox: None,
                    })
                    .collect(),
            ),
            reason: RegionUnavailableReason::PageMappingUnknown,
        };
    };

    let mut incomplete_bbox = false;
    let mut projection_failed = false;
    let mut support = Vec::with_capacity(lines.len());
    for line in lines {
        let projected_bbox = match &line.geometry.bbox {
            Claim::Known { value, .. } => {
                let projected = project_bbox(mapping, value);
                projection_failed |= projected.is_none();
                projected
            }
            Claim::Unknown { .. } => {
                incomplete_bbox = true;
                None
            }
        };
        support.push(RegionLineSupport {
            line_id: line.id.clone(),
            projected_bbox,
        });
    }

    if incomplete_reading_order || incomplete_bbox || projection_failed {
        return RegionState::RI {
            support: nonempty(support),
            reason: if incomplete_reading_order {
                RegionUnavailableReason::IncompleteLineReadingOrder
            } else if incomplete_bbox {
                RegionUnavailableReason::IncompleteLineBbox
            } else {
                RegionUnavailableReason::ProjectionFailed
            },
        };
    }

    let projected: Vec<ProjectedRegionLine> = support
        .iter()
        .map(|line| ProjectedRegionLine {
            line_id: line.line_id.clone(),
            bbox: line.projected_bbox.expect("complete region support"),
        })
        .collect();
    if projected
        .windows(2)
        .any(|pair| !(pair[1].bbox.y0 - pair[0].bbox.y1).is_finite())
        || (projected.len() >= 2
            && !(projected[0].bbox.x0
                - projected[1..]
                    .iter()
                    .map(|line| line.bbox.x0)
                    .fold(f64::INFINITY, f64::min))
            .is_finite())
    {
        return RegionState::RI {
            support: nonempty(support),
            reason: RegionUnavailableReason::ProjectionFailed,
        };
    }

    complete_region_state(projected)
}

fn project_bbox(mapping: &PageMapping, bbox: &FramedBbox) -> Option<ProjectedBox> {
    let corners = [
        (bbox.x0, bbox.y0),
        (bbox.x1, bbox.y0),
        (bbox.x1, bbox.y1),
        (bbox.x0, bbox.y1),
    ];
    let mut x0 = f64::INFINITY;
    let mut y0 = f64::INFINITY;
    let mut x1 = f64::NEG_INFINITY;
    let mut y1 = f64::NEG_INFINITY;
    for (x, y) in corners {
        let matrix = &mapping.matrix;
        let projected_x = matrix[0] * x + matrix[1] * y + matrix[2];
        let projected_y = matrix[3] * x + matrix[4] * y + matrix[5];
        let denominator = matrix[6] * x + matrix[7] * y + matrix[8];
        let x = projected_x / denominator;
        let y = projected_y / denominator;
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    Some(ProjectedBox { x0, y0, x1, y1 })
}

fn complete_global_state(mut lines: Vec<ProjectedLine>) -> GlobalState {
    if lines.len() == 1 {
        GlobalState::R1 {
            line: lines.remove(0),
        }
    } else {
        let first = lines.remove(0);
        let second = lines.remove(0);
        GlobalState::RN {
            first,
            second,
            rest: lines,
        }
    }
}

fn complete_region_state(mut lines: Vec<ProjectedRegionLine>) -> RegionState {
    if lines.len() == 1 {
        RegionState::R1 {
            line: lines.remove(0),
        }
    } else {
        let first = lines.remove(0);
        let second = lines.remove(0);
        RegionState::RN {
            first,
            second,
            rest: lines,
        }
    }
}

fn nonempty<T>(values: Vec<T>) -> NonEmpty<T> {
    let mut values = values.into_iter();
    NonEmpty {
        first: values.next().expect("non-empty support"),
        rest: values.collect(),
    }
}

impl<T> NonEmpty<T> {
    fn len(&self) -> usize {
        1 + self.rest.len()
    }

    fn get(&self, index: usize) -> Option<&T> {
        if index == 0 {
            Some(&self.first)
        } else {
            self.rest.get(index - 1)
        }
    }
}

impl ScanLayoutProfile {
    pub fn source(&self) -> &ScanLayoutSource {
        &self.source
    }

    pub fn coverage(&self) -> ScanLayoutCoverageView<'_> {
        ScanLayoutCoverageView {
            page: &self.state.page,
            global: &self.state.global,
        }
    }

    pub fn page(&self) -> ScanLayoutPageView<'_> {
        ScanLayoutPageView {
            page: &self.state.page,
        }
    }

    pub fn observed_text_envelope(&self) -> ScanLayoutBoxMeasurementView<'_> {
        ScanLayoutBoxMeasurementView {
            source: BoxMeasurementSource::Global {
                page: &self.state.page,
                global: &self.state.global,
            },
        }
    }

    pub fn observed_text_insets(&self) -> ScanLayoutInsetsMeasurementView<'_> {
        ScanLayoutInsetsMeasurementView {
            page: &self.state.page,
            global: &self.state.global,
        }
    }

    pub fn regions(&self) -> ScanLayoutRegionsView<'_> {
        ScanLayoutRegionsView {
            page: &self.state.page,
            regions: &self.state.regions,
        }
    }
}

impl ScanLayoutSource {
    pub fn page_index(&self) -> u32 {
        self.page_index
    }

    pub fn raster_sha256(&self) -> &str {
        &self.raster_sha256
    }
}

impl<'a> ScanLayoutCoverageView<'a> {
    pub fn total_lines(&self) -> usize {
        global_len(self.global)
    }

    pub fn projected_lines(&self) -> ScanLayoutCountView<'a> {
        ScanLayoutCountView {
            page: self.page,
            global: self.global,
        }
    }
}

impl<'a> ScanLayoutPageView<'a> {
    pub fn status(&self) -> &'static str {
        match self.page {
            PageState::Unknown => "unknown",
            PageState::Derived(_) => "derived",
        }
    }

    pub fn reason(&self) -> Option<&'static str> {
        match self.page {
            PageState::Unknown => Some("page-mapping-unknown"),
            PageState::Derived(_) => None,
        }
    }

    pub fn frame_id(&self) -> Option<&'a str> {
        match self.page {
            PageState::Unknown => None,
            PageState::Derived(page) => Some(&page.frame_id),
        }
    }

    pub fn extent_pt(&self) -> Option<ScanLayoutExtentView> {
        match self.page {
            PageState::Unknown => None,
            PageState::Derived(page) => Some(ScanLayoutExtentView {
                width: page.width,
                height: page.height,
            }),
        }
    }

    pub fn max_error_pt(&self) -> Option<f64> {
        match self.page {
            PageState::Unknown => None,
            PageState::Derived(page) => Some(page.max_error_pt),
        }
    }
}

impl ScanLayoutExtentView {
    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn height(&self) -> f64 {
        self.height
    }
}

impl ScanLayoutBoxView {
    pub fn x0_pt(&self) -> f64 {
        self.bbox.x0
    }

    pub fn y0_pt(&self) -> f64 {
        self.bbox.y0
    }

    pub fn x1_pt(&self) -> f64 {
        self.bbox.x1
    }

    pub fn y1_pt(&self) -> f64 {
        self.bbox.y1
    }
}

impl ScanLayoutInsetsView {
    pub fn top_pt(&self) -> f64 {
        self.top
    }

    pub fn left_pt(&self) -> f64 {
        self.left
    }

    pub fn right_pt(&self) -> f64 {
        self.right
    }

    pub fn bottom_pt(&self) -> f64 {
        self.bottom
    }
}

impl<'a> ScanLayoutCountView<'a> {
    pub fn status(&self) -> &'static str {
        if matches!(self.page, PageState::Unknown) {
            "unknown"
        } else {
            "derived"
        }
    }

    pub fn reason(&self) -> Option<&'static str> {
        if matches!(self.page, PageState::Unknown) {
            Some("page-mapping-unknown")
        } else {
            None
        }
    }

    pub fn value(&self) -> Option<usize> {
        if matches!(self.page, PageState::Unknown) {
            None
        } else {
            Some(global_projected_len(self.global))
        }
    }
}

impl<'a> ScanLayoutBoxMeasurementView<'a> {
    pub fn status(&self) -> &'static str {
        if self.reason().is_some() {
            "unknown"
        } else {
            "derived"
        }
    }

    pub fn reason(&self) -> Option<&'static str> {
        match self.source {
            BoxMeasurementSource::Global { page, global } => {
                global_measurement_reason(page, global)
            }
            BoxMeasurementSource::Region { page, region } => {
                region_measurement_reason(page, region)
            }
        }
    }

    pub fn value(&self) -> Option<ScanLayoutBoxView> {
        if self.reason().is_some() {
            return None;
        }
        let bbox = match self.source {
            BoxMeasurementSource::Global { global, .. } => global_envelope(global),
            BoxMeasurementSource::Region { region, .. } => region_envelope(region),
        }?;
        Some(ScanLayoutBoxView { bbox })
    }
}

impl<'a> ScanLayoutInsetsMeasurementView<'a> {
    pub fn status(&self) -> &'static str {
        if self.reason().is_some() {
            "unknown"
        } else {
            "derived"
        }
    }

    pub fn reason(&self) -> Option<&'static str> {
        global_measurement_reason(self.page, self.global)
    }

    pub fn value(&self) -> Option<ScanLayoutInsetsView> {
        if self.reason().is_some() {
            return None;
        }
        let PageState::Derived(page) = self.page else {
            return None;
        };
        let bbox = global_envelope(self.global)?;
        Some(ScanLayoutInsetsView {
            top: bbox.y0,
            left: bbox.x0,
            right: page.width - bbox.x1,
            bottom: page.height - bbox.y1,
        })
    }
}

impl<'a> ScanLayoutScalarMeasurementView<'a> {
    pub fn status(&self) -> &'static str {
        if self.reason().is_some() {
            "unknown"
        } else {
            "derived"
        }
    }

    pub fn reason(&self) -> Option<&'static str> {
        if matches!(self.page, PageState::Unknown) {
            return Some("page-mapping-unknown");
        }
        match self.region {
            RegionState::R0 => Some("no-lines"),
            RegionState::RI { reason, .. } => Some(region_reason(*reason)),
            RegionState::R1 { .. } => Some("insufficient-support"),
            RegionState::RN { .. } => None,
        }
    }

    pub fn value(&self) -> Option<f64> {
        if self.reason().is_some() {
            return None;
        }
        let RegionState::RN {
            first,
            second,
            rest,
        } = self.region
        else {
            return None;
        };
        let continuation_min = std::iter::once(second)
            .chain(rest.iter())
            .map(|line| line.bbox.x0)
            .fold(f64::INFINITY, f64::min);
        Some(first.bbox.x0 - continuation_min)
    }
}

impl<'a> ScanLayoutNumberListMeasurementView<'a> {
    pub fn status(&self) -> &'static str {
        if self.reason().is_some() {
            "unknown"
        } else {
            "derived"
        }
    }

    pub fn reason(&self) -> Option<&'static str> {
        region_measurement_reason(self.page, self.region)
    }

    pub fn len(&self) -> usize {
        if self.reason().is_some() {
            return 0;
        }
        let line_count = region_complete_len(self.region);
        match self.kind {
            NumberListKind::Starts => line_count,
            NumberListKind::Gaps => line_count.saturating_sub(1),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<f64> {
        if index >= self.len() {
            return None;
        }
        match self.kind {
            NumberListKind::Starts => Some(region_line(self.region, index)?.bbox.x0),
            NumberListKind::Gaps => {
                let current = region_line(self.region, index)?;
                let next = region_line(self.region, index + 1)?;
                Some(next.bbox.y0 - current.bbox.y1)
            }
        }
    }
}

impl<'a> ScanLayoutStringListView<'a> {
    pub fn len(&self) -> usize {
        region_len(self.region)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<&'a str> {
        match self.region {
            RegionState::R0 => None,
            RegionState::RI { support, .. } => support.get(index).map(|line| line.line_id.as_str()),
            RegionState::R1 { line } => (index == 0).then_some(line.line_id.as_str()),
            RegionState::RN {
                first,
                second,
                rest,
            } => match index {
                0 => Some(first.line_id.as_str()),
                1 => Some(second.line_id.as_str()),
                index => rest.get(index - 2).map(|line| line.line_id.as_str()),
            },
        }
    }
}

impl<'a> ScanLayoutRegionsView<'a> {
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<ScanLayoutRegionView<'a>> {
        self.regions.get(index).map(|region| ScanLayoutRegionView {
            page: self.page,
            region,
        })
    }
}

impl<'a> ScanLayoutRegionView<'a> {
    pub fn region_id(&self) -> &'a str {
        &self.region.region_id
    }

    pub fn line_support_ids(&self) -> ScanLayoutStringListView<'a> {
        ScanLayoutStringListView {
            region: &self.region.state,
        }
    }

    pub fn observed_line_envelope(&self) -> ScanLayoutBoxMeasurementView<'a> {
        ScanLayoutBoxMeasurementView {
            source: BoxMeasurementSource::Region {
                page: self.page,
                region: &self.region.state,
            },
        }
    }

    pub fn line_starts_pt(&self) -> ScanLayoutNumberListMeasurementView<'a> {
        ScanLayoutNumberListMeasurementView {
            page: self.page,
            region: &self.region.state,
            kind: NumberListKind::Starts,
        }
    }

    pub fn signed_line_box_gaps_pt(&self) -> ScanLayoutNumberListMeasurementView<'a> {
        ScanLayoutNumberListMeasurementView {
            page: self.page,
            region: &self.region.state,
            kind: NumberListKind::Gaps,
        }
    }

    pub fn first_line_start_delta_pt(&self) -> ScanLayoutScalarMeasurementView<'a> {
        ScanLayoutScalarMeasurementView {
            page: self.page,
            region: &self.region.state,
        }
    }
}

fn global_len(state: &GlobalState) -> usize {
    match state {
        GlobalState::R0 => 0,
        GlobalState::RI { support, .. } => support.len(),
        GlobalState::R1 { .. } => 1,
        GlobalState::RN { rest, .. } => 2 + rest.len(),
    }
}

fn global_projected_len(state: &GlobalState) -> usize {
    match state {
        GlobalState::R0 => 0,
        GlobalState::RI { support, .. } => std::iter::once(&support.first)
            .chain(support.rest.iter())
            .filter(|line| line.projected_bbox.is_some())
            .count(),
        GlobalState::R1 { .. } => 1,
        GlobalState::RN { rest, .. } => 2 + rest.len(),
    }
}

fn region_len(state: &RegionState) -> usize {
    match state {
        RegionState::R0 => 0,
        RegionState::RI { support, .. } => support.len(),
        RegionState::R1 { .. } => 1,
        RegionState::RN { rest, .. } => 2 + rest.len(),
    }
}

fn region_complete_len(state: &RegionState) -> usize {
    match state {
        RegionState::R1 { .. } => 1,
        RegionState::RN { rest, .. } => 2 + rest.len(),
        RegionState::R0 | RegionState::RI { .. } => 0,
    }
}

fn global_measurement_reason(page: &PageState, state: &GlobalState) -> Option<&'static str> {
    if matches!(page, PageState::Unknown) {
        return Some("page-mapping-unknown");
    }
    match state {
        GlobalState::R0 => Some("no-lines"),
        GlobalState::RI { reason, .. } => Some(global_reason(*reason)),
        GlobalState::R1 { .. } | GlobalState::RN { .. } => None,
    }
}

fn region_measurement_reason(page: &PageState, state: &RegionState) -> Option<&'static str> {
    if matches!(page, PageState::Unknown) {
        return Some("page-mapping-unknown");
    }
    match state {
        RegionState::R0 => Some("no-lines"),
        RegionState::RI { reason, .. } => Some(region_reason(*reason)),
        RegionState::R1 { .. } | RegionState::RN { .. } => None,
    }
}

fn global_reason(reason: GlobalUnavailableReason) -> &'static str {
    match reason {
        GlobalUnavailableReason::PageMappingUnknown => "page-mapping-unknown",
        GlobalUnavailableReason::IncompleteLineBbox => "incomplete-line-bbox",
        GlobalUnavailableReason::ProjectionFailed => "projection-failed",
    }
}

fn region_reason(reason: RegionUnavailableReason) -> &'static str {
    match reason {
        RegionUnavailableReason::PageMappingUnknown => "page-mapping-unknown",
        RegionUnavailableReason::IncompleteLineReadingOrder => "incomplete-line-reading-order",
        RegionUnavailableReason::IncompleteLineBbox => "incomplete-line-bbox",
        RegionUnavailableReason::ProjectionFailed => "projection-failed",
    }
}

fn global_envelope(state: &GlobalState) -> Option<ProjectedBox> {
    match state {
        GlobalState::R1 { line } => Some(line.bbox),
        GlobalState::RN {
            first,
            second,
            rest,
        } => Some(envelope(
            std::iter::once(first.bbox)
                .chain(std::iter::once(second.bbox))
                .chain(rest.iter().map(|line| line.bbox)),
        )),
        GlobalState::R0 | GlobalState::RI { .. } => None,
    }
}

fn region_envelope(state: &RegionState) -> Option<ProjectedBox> {
    match state {
        RegionState::R1 { line } => Some(line.bbox),
        RegionState::RN {
            first,
            second,
            rest,
        } => Some(envelope(
            std::iter::once(first.bbox)
                .chain(std::iter::once(second.bbox))
                .chain(rest.iter().map(|line| line.bbox)),
        )),
        RegionState::R0 | RegionState::RI { .. } => None,
    }
}

fn envelope(boxes: impl IntoIterator<Item = ProjectedBox>) -> ProjectedBox {
    let mut result = ProjectedBox {
        x0: f64::INFINITY,
        y0: f64::INFINITY,
        x1: f64::NEG_INFINITY,
        y1: f64::NEG_INFINITY,
    };
    for bbox in boxes {
        result.x0 = result.x0.min(bbox.x0);
        result.y0 = result.y0.min(bbox.y0);
        result.x1 = result.x1.max(bbox.x1);
        result.y1 = result.y1.max(bbox.y1);
    }
    result
}

fn region_line(state: &RegionState, index: usize) -> Option<&ProjectedRegionLine> {
    match state {
        RegionState::R1 { line } => (index == 0).then_some(line),
        RegionState::RN {
            first,
            second,
            rest,
        } => match index {
            0 => Some(first),
            1 => Some(second),
            index => rest.get(index - 2),
        },
        RegionState::R0 | RegionState::RI { .. } => None,
    }
}

impl fmt::Display for ScanLayoutProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for ScanLayoutProfileError {}
