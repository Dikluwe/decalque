//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/pdf-scan-like-diagnostic.md
//! @layer L1
//! @updated 2026-09-14

use crate::content::{XObjectInfo, XObjectSubtype};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageDiagnostic {
    ImageOnlyPage,
    NoTextOperators,
}

/// Classifica uma página sem texto a partir dos hints brutos do adaptador.
///
/// A interface v1 não recebe os nomes de cada `Do`; assim, o cruzamento
/// disponível é entre a presença de ao menos um `Do` e ao menos um recurso
/// Image. Correlacionar invocações por nome exige revisão do contrato.
pub fn diagnose_page(
    has_text_show_operators: bool,
    has_do_operator: bool,
    xobjects: &[XObjectInfo],
) -> Vec<PageDiagnostic> {
    if has_text_show_operators {
        return Vec::new();
    }
    if has_do_operator
        && xobjects
            .iter()
            .any(|xobject| xobject.subtype == XObjectSubtype::Image)
    {
        vec![PageDiagnostic::ImageOnlyPage]
    } else {
        vec![PageDiagnostic::NoTextOperators]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xobject(name: &str, subtype: XObjectSubtype) -> XObjectInfo {
        XObjectInfo {
            name: name.to_string(),
            subtype,
        }
    }

    #[test]
    fn text_present_suppresses_page_diagnostics_even_with_image() {
        assert_eq!(
            diagnose_page(true, true, &[xobject("Im0", XObjectSubtype::Image)]),
            vec![]
        );
    }

    #[test]
    fn image_invocation_without_text_is_image_only() {
        assert_eq!(
            diagnose_page(false, true, &[xobject("Im0", XObjectSubtype::Image)]),
            vec![PageDiagnostic::ImageOnlyPage]
        );
    }

    #[test]
    fn blank_or_vector_page_has_no_text_operators() {
        assert_eq!(
            diagnose_page(false, false, &[]),
            vec![PageDiagnostic::NoTextOperators]
        );
    }

    #[test]
    fn form_invocation_is_not_classified_as_image() {
        assert_eq!(
            diagnose_page(false, true, &[xobject("Fm0", XObjectSubtype::Form)]),
            vec![PageDiagnostic::NoTextOperators]
        );
    }

    #[test]
    fn image_resource_without_do_is_not_classified_as_image_only() {
        assert_eq!(
            diagnose_page(false, false, &[xobject("Im0", XObjectSubtype::Image)]),
            vec![PageDiagnostic::NoTextOperators]
        );
    }
}
