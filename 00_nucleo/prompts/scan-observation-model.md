# Prompt: `ScanObservation v1` — contrato externo de observações OCR

**Camada**: L1 — modelo de domínio puro; serialização concreta na fronteira L3
**Arquivo gerado futuramente**: `01_core/src/entities/scan_observation.rs`
**ADR**: `00_nucleo/adr/0004-fronteira-observacao-scan.md`,
`00_nucleo/adr/0003-convencoes-transversais.md`
**Depende de**: `case2-scan-to-digital.md`

## Obrigação

Representar, validar e preservar uma página de observações OCR produzida fora do Decalque sem
adaptá-la ao PDF candidato. Texto, ordem, geometria, associação, transformação e confiança são
claims independentes. A granularidade é a que a evidência sustenta: região, linha, palavra ou
glifo.

`ScanObservation` não é `DocumentGeometry`. Nenhuma função deste componente cria
`GlyphInstance`, executa OCR, lê JSON ou acessa arquivos.

## Limite por camada

- L1 define tipos, invariantes e validação pura.
- L2 decide quais claims e níveis são obrigatórios para uma comparação.
- L3 materializa o JSON v1 nesses tipos (`scan-observation-json-adapter.md`).
- L4 combina a observação validada com o candidato digital.
- O produtor OCR é externo às quatro camadas.

## Forma de domínio

Forma conceitual obrigatória; nomes Rust podem ser refinados sem alterar os observáveis:

```rust
pub struct ScanObservation {
    pub source: ScanSource,
    pub producer: ProducerIdentity,
    pub raster_frame: RasterFrame,
    pub page_mapping: Claim<PageMapping>,
    pub provenance: Vec<ProvenanceRecord>,
    pub units: Vec<ObservationUnit>,
    pub diagnostics: Vec<ScanObservationDiagnostic>,
}

pub enum Claim<T> {
    Known {
        value: T,
        basis: EvidenceBasis,
        evidence: Vec<ProvenanceId>,
        confidence: Confidence,
    },
    Unknown {
        reason: UnknownReason,
        evidence: Vec<ProvenanceId>,
        detail: Option<String>,
    },
}

pub enum Confidence {
    Known { value: f64, semantics: String },
    Unknown { reason: UnknownReason },
}

pub enum EvidenceBasis { Observed, Inferred, Derived, Asserted }

pub enum UnknownReason {
    NotObserved,
    Ambiguous,
    Unsupported,
    Invalid,
    BudgetExhausted,
    Redacted,
    BelowPolicyThreshold,
}

pub enum ObservationKind { Region, Line, Word, Glyph }

pub struct ObservationUnit {
    pub id: String,
    pub kind: ObservationKind,
    pub parent_id: Option<String>,
    pub reading_order: Claim<u32>,
    pub text: Claim<String>,
    pub span_in_parent: Claim<UnicodeRange>,
    pub geometry: GeometryClaims,
}

pub struct GeometryClaims {
    pub bbox: Claim<FramedBbox>,
    pub polygon: Claim<FramedPolygon>,
    pub baseline: Claim<FramedPolyline>,
}
```

Os vetores `evidence` de claims `Known` são não vazios por invariante, ainda que o tipo Rust
concreto use `Vec`.

## Schema JSON v1 normativo

O objeto abaixo é um exemplo completo e válido. Nomes de campos, tags, convenções de coordenadas
e estrutura de `Known`/`Unknown` são normativos; IDs, hashes, textos e números são valores de
exemplo.

```json
{
  "schema": "decalque.scan-observation",
  "schema_version": 1,
  "source": {
    "page_index": 0,
    "raster": {
      "artifact_id": "raster-0",
      "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "media_type": "image/png",
      "width_px": 1000,
      "height_px": 2000
    }
  },
  "producer": {
    "name": "example-ocr-exporter",
    "version": "1.2.3",
    "run_id": "run-2026-09-18-001"
  },
  "raster_frame": {
    "id": "scan-px",
    "unit": "px",
    "origin": "top-left",
    "x_direction": "right",
    "y_direction": "down",
    "coordinate_basis": "pixel-edges",
    "extent": [1000, 2000]
  },
  "page_mapping": {
    "status": "known",
    "value": {
      "source_frame_id": "scan-px",
      "target_frame": {
        "id": "source-page-pt",
        "unit": "pt",
        "origin": "top-left",
        "x_direction": "right",
        "y_direction": "down",
        "extent": [500.0, 1000.0]
      },
      "kind": "homography-3x3",
      "matrix": [0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0],
      "max_error_pt": 0.0
    },
    "basis": "derived",
    "evidence": ["prov-map"],
    "confidence": {
      "status": "known",
      "value": 1.0,
      "semantics": "deterministic-transform"
    }
  },
  "provenance": [
    {
      "id": "prov-layout",
      "stage": "layout-detection",
      "tool_name": "example-layout-detector",
      "tool_version": "4.5.6",
      "model_identifier": "layout-model-v2",
      "method": "region-and-line-detection",
      "parameters_sha256": "1111111111111111111111111111111111111111111111111111111111111111",
      "input_artifact_ids": ["raster-0"],
      "parent_provenance_ids": []
    },
    {
      "id": "prov-text",
      "stage": "text-recognition",
      "tool_name": "example-text-recognizer",
      "tool_version": "7.8.9",
      "model_identifier": "text-model-v3",
      "method": "line-recognition",
      "parameters_sha256": "2222222222222222222222222222222222222222222222222222222222222222",
      "input_artifact_ids": ["raster-0"],
      "parent_provenance_ids": ["prov-layout"]
    },
    {
      "id": "prov-tokenize",
      "stage": "segmentation",
      "tool_name": "example-ocr-exporter",
      "tool_version": "1.2.3",
      "model_identifier": "none",
      "method": "unicode-span-tokenization",
      "parameters_sha256": "3333333333333333333333333333333333333333333333333333333333333333",
      "input_artifact_ids": [],
      "parent_provenance_ids": ["prov-text"]
    },
    {
      "id": "prov-map",
      "stage": "coordinate-transform",
      "tool_name": "example-ocr-exporter",
      "tool_version": "1.2.3",
      "model_identifier": "none",
      "method": "calibrated-page-homography",
      "parameters_sha256": "4444444444444444444444444444444444444444444444444444444444444444",
      "input_artifact_ids": ["raster-0"],
      "parent_provenance_ids": []
    }
  ],
  "units": [
    {
      "id": "region-1",
      "kind": "region",
      "reading_order": {
        "status": "known",
        "value": 0,
        "basis": "inferred",
        "evidence": ["prov-layout"],
        "confidence": {
          "status": "known",
          "value": 0.97,
          "semantics": "provider-native"
        }
      },
      "text": {
        "status": "known",
        "value": "Hello world",
        "basis": "inferred",
        "evidence": ["prov-text"],
        "confidence": {
          "status": "known",
          "value": 0.96,
          "semantics": "provider-native"
        }
      },
      "span_in_parent": {
        "status": "unknown",
        "reason": "not-observed",
        "evidence": [],
        "detail": "top-level region has no textual parent"
      },
      "geometry": {
        "bbox": {
          "status": "known",
          "value": {
            "frame_id": "scan-px",
            "x0": 100.0,
            "y0": 200.0,
            "x1": 900.0,
            "y1": 320.0
          },
          "basis": "observed",
          "evidence": ["prov-layout"],
          "confidence": {
            "status": "known",
            "value": 0.99,
            "semantics": "provider-native"
          }
        },
        "polygon": {
          "status": "unknown",
          "reason": "not-observed",
          "evidence": ["prov-layout"],
          "detail": "provider emitted only a bounding box"
        },
        "baseline": {
          "status": "unknown",
          "reason": "not-observed",
          "evidence": [],
          "detail": "region level has no baseline"
        }
      }
    },
    {
      "id": "line-1",
      "kind": "line",
      "parent_id": "region-1",
      "reading_order": {
        "status": "known",
        "value": 0,
        "basis": "inferred",
        "evidence": ["prov-layout"],
        "confidence": {
          "status": "known",
          "value": 0.95,
          "semantics": "provider-native"
        }
      },
      "text": {
        "status": "known",
        "value": "Hello world",
        "basis": "inferred",
        "evidence": ["prov-text"],
        "confidence": {
          "status": "known",
          "value": 0.96,
          "semantics": "provider-native"
        }
      },
      "span_in_parent": {
        "status": "known",
        "value": {"start": 0, "end": 11},
        "basis": "derived",
        "evidence": ["prov-tokenize"],
        "confidence": {
          "status": "known",
          "value": 1.0,
          "semantics": "deterministic"
        }
      },
      "geometry": {
        "bbox": {
          "status": "known",
          "value": {
            "frame_id": "scan-px",
            "x0": 120.0,
            "y0": 220.0,
            "x1": 880.0,
            "y1": 290.0
          },
          "basis": "observed",
          "evidence": ["prov-layout"],
          "confidence": {
            "status": "known",
            "value": 0.98,
            "semantics": "provider-native"
          }
        },
        "polygon": {
          "status": "unknown",
          "reason": "not-observed",
          "evidence": ["prov-layout"],
          "detail": "provider emitted only a bounding box"
        },
        "baseline": {
          "status": "known",
          "value": {
            "frame_id": "scan-px",
            "points": [[120.0, 282.0], [880.0, 282.0]]
          },
          "basis": "observed",
          "evidence": ["prov-layout"],
          "confidence": {
            "status": "known",
            "value": 0.91,
            "semantics": "provider-native"
          }
        }
      }
    },
    {
      "id": "word-1",
      "kind": "word",
      "parent_id": "line-1",
      "reading_order": {
        "status": "known",
        "value": 0,
        "basis": "derived",
        "evidence": ["prov-tokenize"],
        "confidence": {
          "status": "known",
          "value": 1.0,
          "semantics": "deterministic"
        }
      },
      "text": {
        "status": "known",
        "value": "Hello",
        "basis": "derived",
        "evidence": ["prov-tokenize"],
        "confidence": {
          "status": "known",
          "value": 0.96,
          "semantics": "inherited-from-parent-recognition"
        }
      },
      "span_in_parent": {
        "status": "known",
        "value": {"start": 0, "end": 5},
        "basis": "derived",
        "evidence": ["prov-tokenize"],
        "confidence": {
          "status": "known",
          "value": 1.0,
          "semantics": "deterministic"
        }
      },
      "geometry": {
        "bbox": {
          "status": "unknown",
          "reason": "not-observed",
          "evidence": [],
          "detail": "word was tokenized from line text; no word detector ran"
        },
        "polygon": {
          "status": "unknown",
          "reason": "not-observed",
          "evidence": [],
          "detail": "word was tokenized from line text; no word detector ran"
        },
        "baseline": {
          "status": "unknown",
          "reason": "not-observed",
          "evidence": [],
          "detail": "line baseline cannot be copied to a word"
        }
      }
    }
  ],
  "diagnostics": []
}
```

O digest do exemplo é estruturalmente válido, mas fixtures executáveis devem usar o SHA-256 real
do raster correspondente.

## Regras do envelope

1. `schema` é exatamente `decalque.scan-observation`.
2. `schema_version` é exatamente o inteiro `1`; versão ausente ou diferente é rejeitada.
3. Um artefato contém uma página e `source.page_index` é zero-based.
4. `artifact_id`, IDs de frame, unidade e proveniência são strings não vazias e únicas no seu
   namespace.
5. `sha256` usa exatamente 64 caracteres hexadecimais minúsculos.
6. `width_px` e `height_px` são inteiros positivos e iguais ao `extent` do `raster_frame`.
7. O frame raster v1 tem obrigatoriamente `px`, `top-left`, `right`, `down` e `pixel-edges`.
8. `producer` identifica o exportador do contrato; não substitui a proveniência por claim.
9. `diagnostics` contém apenas avisos/informações inspecionáveis. Falha fatal do produtor não
   publica um artefato de sucesso.

## Política normativa de claims

### `Known`

JSON:

```json
{
  "status": "known",
  "value": {},
  "basis": "observed",
  "evidence": ["prov-id"],
  "confidence": {"status": "unknown", "reason": "not-observed"}
}
```

Regras:

- `value`, `basis`, `evidence` e `confidence` são obrigatórios;
- `evidence` não é vazio e todos os IDs existem;
- `basis` é `observed`, `inferred`, `derived` ou `asserted`;
- confiança conhecida é finita, inclusiva em `[0,1]`, e declara semântica não vazia;
- confiança desconhecida não invalida o valor, mas uma política L2 pode exigir confiança
  conhecida e rebaixar a claim;
- valor vazio ou estruturalmente inválido é erro de contrato, não `Known` de baixa confiança.

### `Unknown`

JSON:

```json
{
  "status": "unknown",
  "reason": "ambiguous",
  "evidence": ["prov-id"],
  "detail": "two candidate associations remain"
}
```

Regras:

- `reason` e `evidence` são obrigatórios; `detail` é opcional;
- `reason` é `not-observed`, `ambiguous`, `unsupported`, `invalid`, `budget-exhausted`,
  `redacted` ou `below-policy-threshold`;
- `evidence` pode ser vazio quando nenhuma tentativa ocorreu;
- `value`, `basis` e `confidence` são proibidos;
- `Unknown` nunca é transformado em valor default nem em `preserved`.

`invalid` descreve uma tentativa do produtor que não sustentou uma claim válida. Inconsistência
do próprio artefato — por exemplo, bbox invertida marcada como `Known` — continua sendo erro de
contrato.

## Granularidade e hierarquia

1. `kind` é `region`, `line`, `word` ou `glyph`.
2. `parent_id`, quando presente, referencia outra unidade da mesma página e não pode formar
   ciclo.
3. Relações normais são região → linha → palavra → glifo. Unidade não associada pode não ter
   pai; essa ausência não cria associação implícita.
4. `span_in_parent` usa índices semiabertos em valores escalares Unicode, não bytes UTF-8.
5. Quando pai, span e textos são `Known`, `child.text` deve ser exatamente a substring do pai.
6. Texto conhecido de unidade textual é não vazio. Ausência de leitura é `Unknown`.
7. A ordem do array `units` é serialização, não ordem de leitura.
8. `reading_order` conhecido é único entre irmãos. Ordem desconhecida não é inferida pelo índice
   do array.
9. Um glifo pode conter vários codepoints; ligadura não é dividida artificialmente.
10. Caixa de pai não pode ser copiada ou subdividida para fabricar geometria de filho. Uma
    derivação geométrica legítima precisa de proveniência própria e predecessores explícitos.

## Proveniência

Cada `ProvenanceRecord` contém obrigatoriamente:

- `id` único;
- `stage`: `layout-detection`, `text-recognition`, `segmentation`,
  `coordinate-transform`, `manual-annotation` ou `other`;
- `tool_name`, `tool_version`, `model_identifier` e `method` não vazios;
- `parameters_sha256` válido;
- `input_artifact_ids` e `parent_provenance_ids` presentes, ainda que vazios.

Invariantes:

1. Todo `input_artifact_id` referencia `source.raster.artifact_id` na v1.
2. Todo pai de proveniência existe e o grafo é acíclico.
3. Toda raiz — registro sem pais — contém `source.raster.artifact_id` em
   `input_artifact_ids`, inclusive quando `stage == manual-annotation`.
4. Registro sem artefato de entrada deve ter ao menos um pai; `manual-annotation` não é
   exceção.
5. Toda cadeia de uma claim conhecida termina numa raiz que referencia diretamente o raster.
   Uma anotação humana declara o método de observação, não uma origem alternativa sem artefato.
6. Texto, geometria, associação, ordem e confiança não compartilham confiança por implicação.
7. `model_identifier: "none"` só é permitido para etapa determinística ou manual cujo `method`
   torna isso verificável.

## Coordenadas

### Frame raster

- Coordenadas são `f64` finitas sobre bordas de pixels.
- O domínio da página é `[0,width_px] × [0,height_px]`.
- bbox conhecida exige `0 <= x0 < x1 <= width_px` e
  `0 <= y0 < y1 <= height_px`.
- polígono conhecido tem ao menos três vértices; baseline conhecida, ao menos dois.
- todos os pontos ficam dentro do frame. Não aplicar clamp.
- se bbox e polígono forem conhecidos, usam o mesmo frame e a bbox é o envelope do polígono
  dentro de epsilon declarado pelo validador.

### Mapeamento para página física

`PageMapping` mapeia o raster observado para a página-fonte exibida, nunca para o candidato.

- target usa pontos, origem superior esquerda, X direita, Y baixo;
- `extent` físico é positivo e finito;
- `matrix` tem nove `f64` finitos, em ordem de linhas;
- convenção: `[x', y', w']ᵀ = M × [x, y, 1]ᵀ`, resultado `(x'/w', y'/w')`;
- matriz é não singular e `w'` não pode ser zero nos pontos transformados;
- `max_error_pt` é finito e não negativo;
- bbox é transformada pelos quatro cantos; o envelope resultante é geometria derivada;
- geometria raster original permanece inalterada;
- `page_mapping: Unknown` impede conclusão geométrica em pontos, mas não análise textual.

É proibido estimar `PageMapping` pela largura/altura do candidato. DPI, crop, rotação e
perspectiva precisam de evidência do lado da referência.

## Fora do contrato v1

- identidade, família, peso, estilo ou tamanho de fonte;
- bytes do raster;
- caminhos que o consumidor deva abrir;
- comandos, URLs ou credenciais;
- resultado da comparação com candidato;
- materialização Typst/PDF;
- métricas visuais raster;
- envelope multipágina.

## Critérios de verificação

1. Schema/número de versão ausente ou diferente é rejeitado.
2. Hash com tamanho, caixa ou caracteres inválidos é rejeitado.
3. `extent` divergente das dimensões do raster é rejeitado.
4. `Known` sem evidência ou com confiança fora de `[0,1]` é rejeitado.
5. `Unknown` contendo `value` é rejeitado.
6. `null`, campo omitido ou bbox zero não é aceito como `Unknown`.
7. Referência de pai ou proveniência inexistente é rejeitada.
8. Ciclo de unidades ou de proveniência é rejeitado.
9. Span fora do texto pai, em offset UTF-8 ou com substring divergente é rejeitado.
10. Irmãos com a mesma ordem conhecida são rejeitados.
11. Linha criada apenas da transcrição pode ser válida com geometria `Unknown`.
12. Palavra tokenizada de linha não herda bbox nem baseline da linha.
13. Bbox invertida, degenerada, não finita ou fora do frame é rejeitada.
14. Polígono com dois pontos e baseline com um ponto são rejeitados.
15. Bbox que não envolve o polígono conhecido é rejeitada.
16. Homografia identidade preserva todos os pontos.
17. A matriz do exemplo 1000×2000 px → 500×1000 pt leva `(200,400)` a `(100,200)`.
18. Homografia singular ou com denominador zero é rejeitada.
19. `page_mapping: Unknown` mantém a observação válida e nenhuma coordenada em pontos é
    inventada.
20. Duas serializações com os mesmos valores semânticos validam para o mesmo modelo de domínio,
    independentemente da ordem dos campos JSON.
21. Uma raiz `manual-annotation` com `input_artifact_ids: []` é rejeitada, mesmo sem pais.
22. Uma raiz `manual-annotation` que referencia diretamente `source.raster.artifact_id` é
    válida; um registro manual sem artefato só é válido quando possui pai e sua cadeia termina
    numa raiz ligada ao raster.

## Histórico de Revisões

| Data | Motivo |
|---|---|
| 2026-09-14 | Modelo intermédio inicial página → região → linha → token. |
| 2026-09-18 | ADR 0004: contrato externo v1 completo, claims marcadas, granularidade explícita, proveniência e coordenadas independentes do candidato. |
| 2026-09-18 | Refinamento pós-verificação: removida a exceção ambígua de anotação humana; toda raiz v1, inclusive manual, referencia o raster observado. |
