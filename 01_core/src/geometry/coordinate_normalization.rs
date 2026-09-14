//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/coordinate-normalization.md
//! @layer L1
//! @updated 2026-09-14

use crate::entities::PageGeometry;

/// Converte um ponto do espaço de usuário PDF para a convenção de saída:
/// origem no canto superior esquerdo e eixo Y crescendo para baixo.
///
/// A origem da caixa efetiva é subtraída e coordenadas fora da página não são
/// limitadas. A altura usada é anterior à rotação; tratamento adicional de
/// páginas rotacionadas depende de um caso real e está fora da versão atual.
pub fn normalize_to_top_left(point: (f64, f64), page_geometry: &PageGeometry) -> (f64, f64) {
    (
        point.0 - page_geometry.origin.0,
        page_geometry.height - (point.1 - page_geometry.origin.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{PageGeometry, PageRotation};

    fn page(origin: (f64, f64)) -> PageGeometry {
        PageGeometry {
            width: 600.0,
            height: 800.0,
            origin,
            rotation: PageRotation::Deg0,
            user_unit: 1.0,
        }
    }

    #[test]
    fn point_near_visual_top_becomes_small_y() {
        assert_eq!(
            normalize_to_top_left((100.0, 700.0), &page((0.0, 0.0))),
            (100.0, 100.0)
        );
    }

    #[test]
    fn bottom_left_becomes_bottom_in_output_coordinates() {
        assert_eq!(
            normalize_to_top_left((0.0, 0.0), &page((0.0, 0.0))),
            (0.0, 800.0)
        );
    }

    #[test]
    fn page_midpoint_is_invariant_with_zero_origin() {
        assert_eq!(
            normalize_to_top_left((50.0, 400.0), &page((0.0, 0.0))),
            (50.0, 400.0)
        );
    }

    #[test]
    fn points_outside_page_are_not_clamped() {
        assert_eq!(
            normalize_to_top_left((100.0, 900.0), &page((0.0, 0.0))),
            (100.0, -100.0)
        );
    }

    #[test]
    fn shifted_box_top_left_becomes_output_origin() {
        let mut geometry = page((10.0, 20.0));
        geometry.height = 792.0;
        assert_eq!(normalize_to_top_left((10.0, 812.0), &geometry), (0.0, 0.0));
    }

    #[test]
    fn shifted_box_bottom_left_preserves_effective_height() {
        let mut geometry = page((10.0, 20.0));
        geometry.height = 792.0;
        assert_eq!(normalize_to_top_left((10.0, 20.0), &geometry), (0.0, 792.0));
    }

    #[test]
    fn zero_origin_matches_the_reduced_formula() {
        let geometry = page((0.0, 0.0));
        for point in [(12.5, 34.0), (-10.0, 850.0), (600.0, 800.0)] {
            assert_eq!(
                normalize_to_top_left(point, &geometry),
                (point.0, geometry.height - point.1)
            );
        }
    }
}
