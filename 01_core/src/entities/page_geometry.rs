//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/page-geometry-model.md
//! @layer L1
//! @updated 2026-09-14

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBoxModel {
    pub media_box: Rect,
    pub crop_box: Option<Rect>,
    pub rotate: Option<i32>,
    pub user_unit: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageRotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGeometry {
    pub width: f64,
    pub height: f64,
    /// Origem da caixa efetiva; não participa do tamanho nem do delta.
    pub origin: (f64, f64),
    pub rotation: PageRotation,
    /// Armazenado, mas não aplicado às dimensões nesta versão.
    pub user_unit: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageGeometryDiagnostic {
    NonStandardRotation,
    InvalidUserUnit,
}

pub fn resolve_page_geometry(raw: &PageBoxModel) -> (PageGeometry, Vec<PageGeometryDiagnostic>) {
    let mut diagnostics = Vec::new();
    let effective = raw.crop_box.unwrap_or(raw.media_box);
    let rotation = match raw.rotate.unwrap_or(0).rem_euclid(360) {
        0 => PageRotation::Deg0,
        90 => PageRotation::Deg90,
        180 => PageRotation::Deg180,
        270 => PageRotation::Deg270,
        _ => {
            diagnostics.push(PageGeometryDiagnostic::NonStandardRotation);
            PageRotation::Deg0
        }
    };
    let user_unit = match raw.user_unit {
        Some(value) if value.is_finite() && value > 0.0 => value,
        None => 1.0,
        Some(_) => {
            diagnostics.push(PageGeometryDiagnostic::InvalidUserUnit);
            1.0
        }
    };
    (
        PageGeometry {
            width: effective.x1 - effective.x0,
            height: effective.y1 - effective.y0,
            origin: (effective.x0, effective.y0),
            rotation,
            user_unit,
        },
        diagnostics,
    )
}

pub fn display_size(geometry: &PageGeometry) -> (f64, f64) {
    match geometry.rotation {
        PageRotation::Deg90 | PageRotation::Deg270 => (geometry.height, geometry.width),
        PageRotation::Deg0 | PageRotation::Deg180 => (geometry.width, geometry.height),
    }
}

impl PageGeometry {
    /// Diferença `other - self` das dimensões brutas; ignora rotação e origem.
    pub fn size_delta(&self, other: &PageGeometry) -> (f64, f64) {
        (other.width - self.width, other.height - self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> PageBoxModel {
        PageBoxModel {
            media_box: Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 612.0,
                y1: 792.0,
            },
            crop_box: None,
            rotate: None,
            user_unit: None,
        }
    }

    #[test]
    fn resolves_defaults_from_media_box() {
        let (g, d) = resolve_page_geometry(&raw());
        assert_eq!(
            g,
            PageGeometry {
                width: 612.0,
                height: 792.0,
                origin: (0.0, 0.0),
                rotation: PageRotation::Deg0,
                user_unit: 1.0
            }
        );
        assert!(d.is_empty());
    }

    #[test]
    fn crop_box_precedes_media_box_and_preserves_origin() {
        let mut model = raw();
        model.crop_box = Some(Rect {
            x0: 10.0,
            y0: 20.0,
            x1: 610.0,
            y1: 812.0,
        });
        let (g, d) = resolve_page_geometry(&model);
        assert_eq!((g.width, g.height, g.origin), (600.0, 792.0, (10.0, 20.0)));
        assert!(d.is_empty());
    }

    #[test]
    fn normalizes_rotation_without_swapping_raw_dimensions() {
        let mut model = raw();
        model.rotate = Some(-90);
        let (g, d) = resolve_page_geometry(&model);
        assert_eq!(g.rotation, PageRotation::Deg270);
        assert_eq!((g.width, g.height), (612.0, 792.0));
        assert_eq!(display_size(&g), (792.0, 612.0));
        assert!(d.is_empty());
    }

    #[test]
    fn resolves_all_standard_rotations_and_display_sizes() {
        for (raw_rotation, rotation, display) in [
            (0, PageRotation::Deg0, (612.0, 792.0)),
            (90, PageRotation::Deg90, (792.0, 612.0)),
            (180, PageRotation::Deg180, (612.0, 792.0)),
            (270, PageRotation::Deg270, (792.0, 612.0)),
            (450, PageRotation::Deg90, (792.0, 612.0)),
        ] {
            let mut model = raw();
            model.rotate = Some(raw_rotation);
            let (g, d) = resolve_page_geometry(&model);
            assert_eq!(g.rotation, rotation);
            assert_eq!(display_size(&g), display);
            assert!(d.is_empty());
        }
    }

    #[test]
    fn diagnoses_non_standard_rotation() {
        let mut model = raw();
        model.rotate = Some(45);
        let (g, d) = resolve_page_geometry(&model);
        assert_eq!(g.rotation, PageRotation::Deg0);
        assert_eq!(d, vec![PageGeometryDiagnostic::NonStandardRotation]);
    }

    #[test]
    fn diagnoses_invalid_user_units() {
        for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut model = raw();
            model.user_unit = Some(invalid);
            let (g, d) = resolve_page_geometry(&model);
            assert_eq!(g.user_unit, 1.0);
            assert_eq!(d, vec![PageGeometryDiagnostic::InvalidUserUnit]);
        }
    }

    #[test]
    fn preserves_positive_finite_user_unit() {
        let mut model = raw();
        model.user_unit = Some(2.5);
        let (g, d) = resolve_page_geometry(&model);
        assert_eq!(g.user_unit, 2.5);
        assert_eq!((g.width, g.height), (612.0, 792.0));
        assert!(d.is_empty());
    }

    #[test]
    fn size_delta_ignores_origin_rotation_and_user_unit() {
        let a = PageGeometry {
            width: 100.0,
            height: 200.0,
            origin: (0.0, 0.0),
            rotation: PageRotation::Deg0,
            user_unit: 1.0,
        };
        let b = PageGeometry {
            width: 150.0,
            height: 180.0,
            origin: (30.0, 40.0),
            rotation: PageRotation::Deg90,
            user_unit: 2.0,
        };
        assert_eq!(a.size_delta(&b), (50.0, -20.0));
    }
}
