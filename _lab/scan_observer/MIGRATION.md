# Migração do observador de scan

Em 2026-09-18, o conteúdo de `05_scan/` foi movido mecanicamente para
`_lab/scan_observer/`. A mudança aplica a ADR 0004: OCR e reconstrução Python são laboratório e
produtor externo; a arquitetura Tekt do Decalque termina em `04_wiring` e consome somente
`ScanObservation v1`.

Nenhum aprendizado foi descartado. Os 108 arquivos originais foram preservados no destino; os
ajustes posteriores se limitam a caminhos internos, referências de specs, documentação e à
remoção da classificação arquitetural falsa. As specs externas/experimentais foram movidas de
`00_nucleo/prompts/` para `specs/`. Os contratos Rust e os cinco contratos de `ScanObservation`
permanecem em `00_nucleo/prompts/`.

## Proveniência dos cabeçalhos removidos

Cada entrada abaixo possuía o marcador histórico de quinta camada. O marcador foi removido sem
alterar a obrigação indicada pela spec associada.

| Caminho original | Caminho atual | Spec de origem preservada |
|---|---|---|
| `05_scan/block_ocr_pipeline.py` | `block_ocr_pipeline.py` | `specs/block-ocr-pipeline.md` |
| `05_scan/book_spread_feedback.py` | `book_spread_feedback.py` | `specs/book-spread-feedback.md` |
| `05_scan/chapter_book_splitter.py` | `chapter_book_splitter.py` | `specs/chapter-book-segmentation.md` |
| `05_scan/editorial_book_composer.py` | `editorial_book_composer.py` | `specs/editorial-typst-composition.md` |
| `05_scan/editorial_region_classifier.py` | `editorial_region_classifier.py` | `specs/editorial-region-classification.md` |
| `05_scan/editorial_typst_composer.py` | `editorial_typst_composer.py` | `specs/editorial-typst-composition.md` |
| `05_scan/frozen_typography_profile.py` | `frozen_typography_profile.py` | `specs/frozen-document-typography.md` |
| `05_scan/hybrid_text_reconstructor.py` | `hybrid_text_reconstructor.py` | `specs/hybrid-text-reconstruction.md` |
| `05_scan/hybrid_typst_page.py` | `hybrid_typst_page.py` | `specs/hybrid-typst-page.md` |
| `05_scan/joint_glyph_reconstructor.py` | `joint_glyph_reconstructor.py` | `specs/joint-glyph-reconstruction.md` |
| `05_scan/line_diff_analyzer.py` | `line_diff_analyzer.py` | `specs/line-diff-analysis.md` |
| `05_scan/line_font_style_classifier.py` | `line_font_style_classifier.py` | `specs/line-font-style-classification.md` |
| `05_scan/local_font_discovery.py` | `local_font_discovery.py` | `specs/local-font-discovery.md` |
| `05_scan/page_format_normalizer.py` | `page_format_normalizer.py` | `specs/page-format-normalization.md` |
| `05_scan/page_geometry_normalizer.py` | `page_geometry_normalizer.py` | `specs/frozen-document-typography.md` |
| `05_scan/page_zone_marker.py` | `page_zone_marker.py` | `specs/page-zone-consensus.md` |
| `05_scan/scan_font_builder.py` | `scan_font_builder.py` | `specs/scan-derived-font.md` |
| `05_scan/scan_glyph_segmenter.py` | `scan_glyph_segmenter.py` | `specs/scan-glyph-segmentation.md` |
| `05_scan/zone_typst_reconstructor.py` | `zone_typst_reconstructor.py` | `specs/zone-typst-materialization.md` |
| `05_scan/tests/test_book_spread_feedback.py` | `tests/test_book_spread_feedback.py` | `specs/book-spread-feedback.md` |
| `05_scan/tests/test_font_guided_glyph_segmenter_execution.py` | `tests/test_font_guided_glyph_segmenter_execution.py` | `specs/scan-glyph-segmentation.md` |
| `05_scan/tests/test_hybrid_text_reconstructor_execution.py` | `tests/test_hybrid_text_reconstructor_execution.py` | `specs/hybrid-text-reconstruction.md` |
| `05_scan/tests/test_joint_glyph_reconstructor_execution.py` | `tests/test_joint_glyph_reconstructor_execution.py` | `specs/joint-glyph-reconstruction.md` |
| `05_scan/tests/test_line_diff_analyzer_execution.py` | `tests/test_line_diff_analyzer_execution.py` | `specs/line-diff-analysis.md` |
| `05_scan/tests/test_margin_word_anchors.py` | `tests/test_margin_word_anchors.py` | `specs/scan-margin-word-anchors.md` |
| `05_scan/tests/test_page_zone_marker_execution.py` | `tests/test_page_zone_marker_execution.py` | `specs/page-zone-consensus.md` |
| `05_scan/tests/test_scan_font_builder_execution.py` | `tests/test_scan_font_builder_execution.py` | `specs/scan-derived-font.md` |
| `05_scan/tests/test_scan_glyph_segmenter_execution.py` | `tests/test_scan_glyph_segmenter_execution.py` | `specs/scan-glyph-segmentation.md` |
| `05_scan/tests/test_scan_sdf_font_execution.py` | `tests/test_scan_sdf_font_execution.py` | `specs/scan-sdf-font.md` |

## Specs movidas

Foram movidas 29 specs ligadas exclusivamente ao laboratório Python: OCR em blocos, geometria
de linha/palavra, transformação de coordenadas, tipografia, fontes derivadas, reconstrução
híbrida/Typst, composição editorial, comparação raster auxiliar e execução multipágina.

Permaneceram no núcleo, por decisão explícita:

- `case2-scan-to-digital.md`;
- `scan-observation-model.md`;
- `scan-observation-json-adapter.md`;
- `scan-observation-compare.md`;
- `cli-scan-observation-compare.md`;
- todos os demais prompts que especificam Rust, inclusive `candidate-font-evidence.md`.

## Verificação reproduzível

A reorganização é válida quando o diretório antigo não existe, nenhum marcador de quinta camada
permanece, todas as specs listadas existem no laboratório e a suíte Python resolve os novos
caminhos. Os testes podem ser executados com:

```sh
python3 -m unittest discover -s _lab/scan_observer/tests -p 'test_*.py' -v
```
