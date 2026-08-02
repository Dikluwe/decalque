# Prompt: `MeasurementResolution`

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/measurement_resolution.rs` + testes no mesmo ficheiro

## Contexto

Terceira pergunta do pipeline, e a que tem uma decisão de desenho genuinamente aberta — ver
`README.md`. Esta peça define quando uma diferença de posição conta como divergência a reportar,
versus ruído de precisão numérica aceitável.

## Decisão de desenho a confirmar antes de implementar (não presumir)

Duas formas candidatas, não mutuamente exclusivas:

1. **Tolerância absoluta**: um valor fixo em pontos (por exemplo, `0.5pt`), igual para o documento
   inteiro. Simples, mas cegamente insensível à escala do que está a ser medido — 0.5pt é grande
   para um índice de 6pt, pequeno para um título de 40pt (achado real do projeto irmão, que usou
   0.5pt fixo em P948 sem questionar se fazia sentido para todos os tamanhos de fonte presentes no
   mesmo documento).
2. **Tolerância relativa ao em**: um limiar expresso em fracção do tamanho de fonte do glifo
   medido (por exemplo, `2% do em`) — escala automaticamente com o contexto.

**Antes de implementar**: decidir se a versão inicial suporta só (1), só (2), ou as duas com (1)
como default e (2) como opção — registar a decisão explicitamente no próprio ficheiro `.rs`
gerado (doc-comment do tipo), não deixar a escolha implícita no código.

## Restrições estruturais

- L1: zero I/O, puro cálculo.

## Instrução (esqueleto mínimo, adaptar conforme a decisão acima)

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
| (preencher na execução) | Criação inicial — decisão de forma (absoluta vs relativa) ainda por confirmar com o dono antes da implementação | `measurement_resolution.rs` |
