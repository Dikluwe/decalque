# Prompt: `MeasurementResolution`

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/measurement_resolution.rs` + testes no mesmo ficheiro

## Contexto

Terceira pergunta do pipeline, e a que tem uma decisão de desenho genuinamente aberta — ver
`README.md`. Esta peça define quando uma diferença de posição conta como divergência a reportar,
versus ruído de precisão numérica aceitável.

## Decisão de desenho (fechada em 2026-08-11, ADR 0001)

Duas formas, **ambas suportadas desde a versão inicial**:

1. **Tolerância absoluta**: um valor fixo em pontos (por exemplo, `0.5pt`), igual para o documento
   inteiro. Simples, mas cegamente insensível à escala do que está a ser medido — 0.5pt é grande
   para um índice de 6pt, pequeno para um título de 40pt (achado real do projeto irmão, que usou
   0.5pt fixo em P948 sem questionar se fazia sentido para todos os tamanhos de fonte presentes no
   mesmo documento).
2. **Tolerância relativa ao em**: um limiar expresso em fracção do tamanho de fonte do glifo
   medido (por exemplo, `2% do em`) — escala automaticamente com o contexto.

A tolerância **não é uma constante do domínio**: é um parâmetro do caso de uso (ADR 0001).
**Quem constrói o `MeasurementResolution` é `02_shell`** (CLI), a partir do caso de uso:

- Caso 1 (digital↔digital): `Absolute { tolerance_pt: 0.5 }` como default (paridade com o
  comportamento validado em P948), `RelativeToEm` disponível como opção;
- Caso 2 (scan→digital): tolerância mais larga — candidatos iniciais
  `RelativeToEm { fraction: 0.02 }` ou `Absolute { tolerance_pt: 2.0 }`, a calibrar com
  documentos reais quando o caso for construído (ver `case2-scan-to-digital.md`).

O motor de comparação (`engine/compare.md`) recebe o `MeasurementResolution` como parâmetro;
esta peça define apenas as duas formas e o cálculo. Registar esta decisão no doc-comment do
tipo gerado, não deixar implícita.

## Restrições estruturais

- L1: zero I/O, puro cálculo.

## Instrução

```rust
enum MeasurementResolution {
    Absolute { tolerance_pt: f64 },
    RelativeToEm { fraction: f64 },
}

impl MeasurementResolution {
    fn tolerance_for(&self, font_size_pt: f64) -> f64 { ... }
    fn is_within(&self, delta: f64, font_size_pt: f64) -> bool { ... }
}
```

## Critérios de verificação

Dado `Absolute { tolerance_pt: 0.5 }`
Quando `is_within(0.3, qualquer_tamanho)` é chamado
Então devolve `true`; `is_within(0.6, qualquer_tamanho)` devolve `false`

Dado `RelativeToEm { fraction: 0.02 }` e `font_size_pt: 10.0`
Quando `tolerance_for(10.0)` é chamado
Então devolve `0.2` (2% de 10pt)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-11 | Criação inicial + primeira geração de `01_core` | `measurement_resolution.rs` |
| 2026-08-11 | ADR 0001: decisão fechada — ambas as formas suportadas; tolerância é parâmetro do caso de uso, não constante do domínio | — |
| 2026-08-12 | Revisão do dono: quem constrói o perfil é `02_shell`, com valores iniciais explícitos por caso de uso (Caso 1: `Absolute 0.5`; Caso 2: `RelativeToEm 0.02` ou `Absolute 2.0`, a calibrar) | — |
