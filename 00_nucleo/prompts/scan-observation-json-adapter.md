# Prompt: adaptador JSON de `ScanObservation v1`

**Camada**: L3 — `03_infra`
**Arquivo gerado futuramente**: `03_infra/src/scan_observation_json.rs`
**ADR**: `00_nucleo/adr/0004-fronteira-observacao-scan.md`
**Depende de**: `scan-observation-model.md`

## Obrigação

Ler um artefato JSON externo `decalque.scan-observation` versão 1, rejeitar qualquer violação
estrutural e construir o modelo puro de L1. O adaptador traduz serialização; não executa OCR,
não corrige claims e não decide política de comparação.

## Fronteira de camada

L3 pode depender da biblioteca de JSON e de `std::fs`. Os DTOs de serialização permanecem
privados em `03_infra`. `01_core` não importa serde, JSON nem tipos L3.

Direção permitida:

```text
03_infra JSON DTO -> validação/conversão -> 01_core::ScanObservation
```

Direções proibidas:

```text
01_core -> serde/serde_json
adaptador -> OCR/modelo/rede
adaptador -> correção ou preenchimento de Unknown
```

## Interface observável

```rust
pub struct ScanObservationLimits {
    pub max_input_bytes: usize,
    pub max_units: usize,
    pub max_provenance_records: usize,
    pub max_geometry_points: usize,
    pub max_total_text_bytes: usize,
}

pub enum ScanObservationJsonError {
    Io { message: String },
    InputTooLarge { limit: usize, actual: usize },
    JsonSyntax { path: Option<String>, message: String },
    UnsupportedSchema { schema: String, version: u64 },
    UnknownField { path: String, field: String },
    ContractViolation { path: String, message: String },
    BudgetExceeded { resource: String, limit: usize, actual: usize },
}

pub fn parse_scan_observation_json(
    bytes: &[u8],
    limits: &ScanObservationLimits,
) -> Result<ScanObservation, ScanObservationJsonError>;

pub fn load_scan_observation_json(
    path: &Path,
    limits: &ScanObservationLimits,
) -> Result<ScanObservation, ScanObservationJsonError>;
```

Os nomes concretos podem ser refinados, mas as variantes de erro e a distinção entre parsing,
schema, contrato e orçamento são observáveis obrigatórios.

## Parsing estrito

1. `schema` deve ser exatamente `decalque.scan-observation`.
2. `schema_version` deve ser o inteiro `1`.
3. Todos os campos exigidos por `scan-observation-model.md` são obrigatórios.
4. Campos desconhecidos são rejeitados no caminho exato em que aparecem. Extensão exige nova
   versão do schema; a v1 não possui saco genérico de extensões.
5. Tags, enums e caixa das strings são exatos.
6. Nenhum valor default é aplicado a claim, confiança, coordenada ou proveniência ausente.
7. Números convertidos para L1 são `f64` finitos. Overflow, número não representável ou inteiro
   negativo onde não permitido é `ContractViolation`.
8. Ordem de campos JSON não é significativa; ordem dos arrays é preservada.
9. Chaves duplicadas no mesmo objeto são rejeitadas, mesmo que os valores coincidam.
10. Depois da desserialização, o adaptador invoca a validação integral de L1; não publica objeto
    parcialmente validado.

## Claims e caminhos de erro

- `status: known` aceita somente `value`, `basis`, `evidence` e `confidence`, além da tag.
- `status: unknown` aceita somente `reason`, `evidence` e `detail`, além da tag.
- `null` é rejeitado em qualquer campo normativo.
- Erros identificam caminho estável, por exemplo:
  `$.units[2].geometry.bbox.value.x1`.
- O adaptador não reescreve `known` inválido como `unknown`; retorna
  `ContractViolation`.
- Referências quebradas, ciclos, spans divergentes, hash inválido e geometria fora do frame são
  violações do contrato, ainda que o JSON seja sintaticamente válido.
- Raiz de proveniência sem referência direta a `source.raster.artifact_id` é violação do
  contrato, inclusive quando `stage` é `manual-annotation`; o adaptador não cria pai nem
  artefato implícito.

## Orçamento e entrada não confiável

Nenhuma leitura é ilimitada. O chamador fornece `ScanObservationLimits`; todos os campos devem
ser positivos. O adaptador verifica, no mínimo:

- bytes do arquivo antes do parse;
- quantidade de unidades;
- quantidade de registros de proveniência;
- total de pontos em polígonos e baselines;
- soma de bytes UTF-8 de textos e detalhes.

Exceder limite é `BudgetExceeded`, não uma observação válida com
`Unknown::BudgetExhausted`. Esse motivo de `Unknown` pertence ao produtor quando uma etapa OCR
foi interrompida mas ainda conseguiu emitir artefato válido.

## Ausência de efeitos externos

- O único arquivo aberto por `load_scan_observation_json` é o caminho recebido pelo chamador.
- IDs, textos, diagnósticos e proveniência nunca são tratados como caminhos ou URLs.
- O adaptador não verifica o raster por conta própria e não dereferencia seu `artifact_id`.
  O SHA-256 é evidência registrada; verificação contra bytes reais exige entrada explícita em
  outro caso de uso L3.
- Nenhum subprocesso, rede, import dinâmico ou descoberta de modelo é permitido.
- Mensagens de erro não imprimem o documento JSON completo.

## Conversão para L1

- Strings e arrays são preservados sem normalização ou reordenação.
- Coordenadas entram como `f64` sem clamp.
- `Known`/`Unknown` são convertidos para variantes distintas do domínio.
- `parent_id`, IDs de proveniência e frame são resolvidos pela validação L1.
- O adaptador não cria bbox a partir de polígono nem pontos a partir de bbox.
- O adaptador não aplica `page_mapping`; transformação é cálculo puro posterior em L1.

## Critérios de verificação

1. O exemplo válido de `scan-observation-model.md`, com digest real, é aceito.
2. Espaços e ordem diferente de campos produzem o mesmo modelo L1.
3. Schema incorreto e versões 0, 2 ou string `"1"` produzem `UnsupportedSchema` ou
   `ContractViolation`, nunca fallback.
4. Campo desconhecido em envelope, claim ou unidade é rejeitado com caminho.
5. Campo obrigatório ausente e `null` são rejeitados.
6. Chave JSON duplicada é rejeitada.
7. `Known` com campos de `Unknown` e vice-versa é rejeitado.
8. `NaN`, infinito textual, overflow e coordenada não finita são rejeitados.
9. Pai/proveniência inexistente, ciclos e span divergente são rejeitados depois do parse.
10. Limites pequenos exercitam separadamente bytes, unidades, proveniência, pontos e texto.
11. `InputTooLarge` ocorre antes da construção do modelo completo.
12. Um JSON malformado nunca devolve `ScanObservation` parcial.
13. Nenhum teste do adaptador precisa de servidor, OCR, rasterizador ou rede.
14. O crate `01_core` continua sem dependência de serialização.
15. Uma raiz `manual-annotation` sem raster é rejeitada, e uma raiz manual que referencia o
    raster declarado é aceita quando as demais invariantes são satisfeitas.

## Histórico de Revisões

| Data | Motivo |
|---|---|
| 2026-09-18 | Contrato inicial do adaptador estrito para a fronteira externa `ScanObservation v1`. |
| 2026-09-18 | Refinamento pós-verificação: explicitada no adaptador a mesma regra L1 de que toda raiz de proveniência, inclusive manual, referencia o raster. |
