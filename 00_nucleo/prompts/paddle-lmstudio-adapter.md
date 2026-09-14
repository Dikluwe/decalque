# Prompt: adaptador PaddleOCR-VL + LM Studio

**Camada**: externa ao núcleo; fonte alternativa de geometria para o Caso 2
**Arquivo gerado**: `05_scan/paddle_lmstudio_adapter.py`
**Depende de**: `case2-scan-to-digital.md`

## Objetivo

Executar o pipeline completo PaddleOCR-VL: layout local pelo PaddleOCR e reconhecimento VLM por
um servidor LM Studio compatível com OpenAI. O componente VLM isolado não é fonte de geometria;
as coordenadas devem vir de `layout_det_res`/`parsing_res_list` do pipeline oficial.

## Entrada e saída

Receber uma imagem ou PDF, URL base e identificador do modelo. Emitir em stdout somente JSON
versionado com dimensões da imagem e regiões em ordem de leitura. Cada região contém rótulo,
texto, bbox, polígono, confiança opcional e ordem. Logs do fornecedor permanecem em stderr.

Coordenadas são pixels YDown no espaço da imagem processada. Não converter regiões em glifos e
não fabricar confiança ou caixas ausentes. Falhas de dependência, conexão ou formato retornam
código diferente de zero.

## Verificação

- normalização preserva bbox, polígono, texto e ordem;
- confiança é associada pelo `order` do detector e permanece ausente se não houver correspondência;
- regiões são ordenadas por ordem de leitura, com estabilidade para ordem ausente.
