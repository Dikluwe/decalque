# Prompt: CLI de comparação `ScanObservation` → PDF digital

**Camadas**: L2 (`02_shell`) e L4 (`04_wiring`)
**Arquivos gerados/revistos futuramente**: módulo de argumentos/relatório em `02_shell` e
composição no binário `decalque` em `04_wiring`
**ADR**: `00_nucleo/adr/0004-fronteira-observacao-scan.md`
**Depende de**: `scan-observation-json-adapter.md`, `scan-observation-compare.md`,
`lopdf-backend-adapter.md`

## Obrigação

Expor o Caso 2 como consumidor de uma observação já produzida. A CLI recebe JSON v1, o raster
exato observado e o PDF candidato. Na mesma execução, valida o contrato, vincula a identidade
declarada aos bytes completamente decodificados do raster e só então abre o candidato. Não
executa OCR nem aceita configuração de modelo/provedor.

## Interface

```text
decalque scan-observation <observacao.json> <candidato.pdf> \
  --raster <raster> \
  --granularity <line|word> \
  --horizontal-tolerance-pt <numero> \
  --baseline-tolerance-pt <numero> \
  [--min-text-confidence <0..1>] \
  [--min-geometry-confidence <0..1>]
```

Regras:

- os dois caminhos posicionais e as quatro opções obrigatórias, incluindo `--raster`, são
  exigidos exatamente uma vez;
- caminhos são `PathBuf` e não exigem UTF-8;
- tolerâncias são `f64` finitos e não negativos;
- limiares opcionais são `f64` finitos em `[0,1]`;
- ausência do limiar significa `ConfidenceRequirement::Any`;
- `--granularity line|word` constrói a política L2 correspondente;
- `--raster` preserva o caminho sem UTF-8 e nunca é inferido de `artifact_id`, do JSON ou do PDF;
- normalização textual v1 é `Exact` e não possui flag de relaxamento;
- `--help` mostra uso e a semântica de `unknown`;
- argumento desconhecido, repetido ou valor inválido é erro de uso.

Não existem na v1:

- `--page`: o índice zero-based vem de `source.page_index`;
- opção de OCR, modelo, servidor, dispositivo, DPI ou rasterizador;
- opção para preencher `Unknown`;
- tolerância default escondida;
- modo de comparação tipográfica ou raster.

## Responsabilidades L2

L2:

- faz parsing dos argumentos sem I/O de domínio;
- constrói `ScanComparisonPolicy`;
- exige tolerâncias explícitas;
- renderiza o relatório completo sem separar métricas de cobertura;
- não conhece JSON de entrada, lopdf nem detalhes de OCR.

## Responsabilidades L4

Ordem obrigatória:

1. carregar `ScanObservation` por L3 com limites explícitos;
2. decodificar integralmente o raster explícito e vinculá-lo por SHA-256, media type e dimensões
   à observação, reutilizando exatamente a barreira do validador isolado;
3. obter `source.page_index` da observação vinculada;
4. carregar essa página do PDF candidato pelo adaptador PDF;
5. materializar `DocumentGeometry` pelo wiring existente;
6. chamar o comparador L1 com a política L2;
7. serializar um relatório completo usando a identidade efetivamente vinculada.

Erro em qualquer passo interrompe a execução. Raster inválido junto de PDF inexistente falha
como raster, provando que o candidato ainda não foi aberto. Nenhum JSON parcial de comparação é
publicado. Diagnósticos da observação, do PDF e da comparação permanecem em coleções separadas.

## Saída

`stdout` contém somente um objeto JSON:

```json
{
  "schema": "decalque.scan-comparison-report",
  "schema_version": 1,
  "source": {
    "page_index": 0,
    "raster_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  },
  "policy": {
    "granularity": "word",
    "text_normalization": "exact",
    "horizontal_tolerance_pt": 2.0,
    "baseline_tolerance_pt": 2.0,
    "min_text_confidence": {"status": "not-required"},
    "min_geometry_confidence": {"status": "known", "value": 0.8}
  },
  "content_status": "unknown",
  "geometry_status": "unknown",
  "overall_status": "unknown",
  "coverage": {
    "matched_scan": 1,
    "total_scan": 2,
    "matched_candidate": 1,
    "total_candidate": 2
  },
  "matches": [
    {
      "scan_unit_id": "word-1",
      "candidate_line_index": 0,
      "candidate_scalar_range": [0, 5],
      "dx_start": 0.25,
      "dx_end": 0.5,
      "width_delta": 0.25,
      "baseline_delta": -0.25,
      "horizontal_status": "preserved",
      "baseline_status": "preserved"
    }
  ],
  "unmatched_scan": ["word-2"],
  "unmatched_candidate": [
    {"candidate_line_index": 0, "candidate_scalar_range": [6, 11]}
  ],
  "reflow": [],
  "observation_diagnostics": [],
  "pdf_diagnostics": [],
  "comparison_diagnostics": [
    {"code": "unmatched-text", "scan_unit_id": "word-2"}
  ]
}
```

O exemplo mostra a regra central: métricas boas no par medido com cobertura 1/2 não produzem
`preserved`.

Regras de renderização:

- `preserved`, `violated` e `unknown` usam essas strings minúsculas;
- deltas não medidos são omitidos ou serializados por união marcada, nunca como `0.0` ou
  `null`; a escolha deve ser única em todo o relatório;
- listas seguem ordem determinística da observação e da vista candidata;
- números são emitidos com precisão suficiente para round-trip de `f64`, sem arredondamento de
  apresentação no JSON;
- nenhum caminho local absoluto, conteúdo de raster ou configuração secreta é incluído;
- logs e mensagens de progresso pertencem a `stderr`.

## Códigos de saída

- `0`: argumentos, leitura e comparação concluídos; o JSON informa se o resultado é
  `preserved`, `violated` ou `unknown`;
- `2`: erro de uso, I/O, schema/contrato, orçamento, identidade/decodificação do raster, PDF ou
  serialização; nenhuma saída JSON de sucesso.

Na v1, divergência é resultado de medição, não falha de processo. Não existe código distinto
para `violated` ou `unknown`.

## Política de `Unknown`

- A CLI nunca substitui `unknown` por mensagem de sucesso.
- Limiares de confiança rebaixam claims conforme o relatório, sem reescrever a observação.
- `overall_status: preserved` só é impresso quando L1 devolve cobertura completa e não vazia e
  todos os eixos preservados.
- cobertura `0/0` é serializada com conteúdo, geometria e resultado geral `unknown`, nunca
  `preserved`.
- `page_mapping: Unknown` é refletido como `geometry_status: unknown`, mesmo em escopo vazio.
- O texto de `--help` explica que código 0 significa execução concluída, não paridade.

## Limites de entrada

L4 constrói `ScanObservationLimits` explícitos e registra no erro qual orçamento foi excedido.
Os valores default do produto devem ser declarados em L2 no mesmo componente que os apresenta;
não ficam escondidos em L3. O limite de bytes e o limite de memória decodificada do raster são
os mesmos de `validate-scan-observation`. Flags para aumentar limites ficam fora da v1.

## Critérios de verificação

1. Dois caminhos posicionais, `--raster` e todas as opções obrigatórias são parseados.
2. Caminhos não UTF-8 são preservados.
3. Opção ausente, repetida, desconhecida, NaN, infinito, tolerância negativa ou confiança fora
   de `[0,1]` é rejeitada.
4. `source.page_index` seleciona a página candidata; não existe override silencioso.
5. `line` e `word` constroem políticas distintas e observáveis no relatório.
6. A CLI não aceita flags de OCR, modelo, rede, DPI, fonte ou Typst.
7. stdout de execução válida contém somente JSON parseável.
8. stderr pode conter progresso/erro sem contaminar stdout.
9. `preserved`, `violated` e `unknown` completos retornam código 0.
10. Erro de observação, raster ou PDF retorna código 2 e não publica relatório parcial.
11. Cobertura incompleta aparece junto dos status e impede `preserved`.
12. Delta desconhecido nunca é impresso como zero.
13. O relatório separa diagnósticos da observação, PDF e comparação.
14. `--help` informa sintaxe, índice vindo do artefato e significado do código 0.
15. A execução não inicia subprocesso, servidor, download ou OCR.
16. Escopo vazio publica cobertura `0/0`, diagnóstico de escopo vazio e os três status
    `unknown`, com código de saída 0 por execução concluída.
17. `page_mapping: Unknown` nunca é renderizado como geometria preservada.
18. Byte alterado, digest/tipo/dimensão divergente e PNG/JPEG/PGM truncado impedem a comparação.
19. Raster inválido é diagnosticado antes de qualquer tentativa de abrir o PDF candidato.
20. Remover, ignorar ou mover a vinculação para depois da abertura do candidato faz um teste
    falhar; o relatório recebe a identidade retornada pela vinculação.

## Histórico de Revisões

| Data | Motivo |
|---|---|
| 2026-09-19 | Exigir vinculação atômica do raster na mesma execução da comparação. |
| 2026-09-18 | Contrato inicial da CLI consumidora de `ScanObservation v1`. |
| 2026-09-18 | Refinamento pós-verificação: a saída torna explícitos `0/0` e mapeamento desconhecido como `unknown`, sem sucesso vacuoso. |
