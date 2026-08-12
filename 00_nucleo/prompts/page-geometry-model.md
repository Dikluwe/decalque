# Prompt: modelo de geometria de página — `PageBoxModel`, `PageGeometry`, `resolve_page_geometry`

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/page_geometry.rs` (revisão do existente) + testes no mesmo ficheiro
**Substitui**: `00_nucleo/prompts/_deprecated/page-geometry.md` (o `PageGeometry { width, height }` original é absorvido e estendido por este modelo; a spec antiga fica como histórico)
**ADR**: `00_nucleo/adr/0002-lopdf-backend-parsing.md` (secção "Geometria de página"), `00_nucleo/adr/0003-convencoes-transversais.md` (tipo numérico)

## Contexto

Primeira pergunta do pipeline (ver `README.md`). O PDF não tem uma única caixa de página: tem
`MediaBox` obrigatória, `CropBox` opcional (a área visível efectiva), `Rotate` (rotação de
exibição, múltiplos de 90°) e `UserUnit` (factor de escala, raro, default 1.0). Decisão do
dono (2026-08-12): dois tipos distintos — um **bruto**, produzido por `03_infra` tal como o
PDF declara, e um **resolvido**, calculado por função pura em `01_core`, que é o que o resto
do pipeline consome (`normalize_to_top_left`, `DocumentGeometry`, motor de comparação).

Tipo numérico: **`f64` em todos os campos** (ver ADR 0003).

**Âmbito deste prompt** (decisão do dono, 2026-08-12): revisa **apenas** `page_geometry.rs`.
A actualização dos consumidores existentes fica para prompts específicos. Consumidores
conhecidos que podem precisar de revisão: `normalize_to_top_left`, `DocumentGeometry`,
motor de comparação, testes existentes.

## Restrições estruturais

- L1: zero I/O.
- O arquivo gerado **não importa `lopdf`**.
- O arquivo gerado **não depende de tipos de `03_infra`** (`lopdf` é mencionado apenas como
  origem dos dados, em `03_infra`).
- `PageBoxModel` é construído por `03_infra` a partir do dicionário de página.
- `resolve_page_geometry` é função pura em `01_core`.
- Esta versão **assume que `Rect` contém valores finitos e coordenadas ordenadas**
  (`x0 <= x1`, `y0 <= y1`) — decisão do dono. Tratamento de caixas inválidas fica **fora de
  escopo**; tolerância a PDFs malformados, se um dia desejada, será prompt próprio.
- Rotação **não** troca `width`/`height` dentro de `PageGeometry` — ver regras abaixo.

## Instrução

Derives mínimos em todos os tipos abaixo:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
```

```rust
pub struct Rect { pub x0: f64, pub y0: f64, pub x1: f64, pub y1: f64 }

pub struct PageBoxModel {
    pub media_box: Rect,
    pub crop_box: Option<Rect>,
    pub rotate: Option<i32>,
    pub user_unit: Option<f64>,
}

pub enum PageRotation { Deg0, Deg90, Deg180, Deg270 }

pub struct PageGeometry {
    pub width: f64,
    pub height: f64,
    pub rotation: PageRotation,
    pub user_unit: f64,
}

pub enum PageGeometryDiagnostic {
    NonStandardRotation,   // Rotate não múltiplo de 90 após normalização — tratado como Deg0
    InvalidUserUnit,       // UserUnit zero, negativo, NaN, infinito ou inválido — tratado como 1.0
}

pub fn resolve_page_geometry(raw: &PageBoxModel) -> (PageGeometry, Vec<PageGeometryDiagnostic>);
pub fn display_size(geometry: &PageGeometry) -> (f64, f64);

impl PageGeometry {
    pub fn size_delta(&self, other: &PageGeometry) -> (f64, f64);
}
```

Regras de `resolve_page_geometry`:

1. **`CropBox`, se existir, substitui `MediaBox`** (precedência, não comparação de tamanho).
   Se não existir, `media_box` é a caixa efectiva.
2. `width` = largura da caixa efectiva (`x1 - x0`), `height` = altura (`y1 - y0`) — **antes da
   rotação**.
3. `Rotate`:
   - normalizar usando `rem_euclid(360)`;
   - aceitar 0, 90, 180 e 270;
   - se `None`, usar `Deg0`;
   - se o valor final não for múltiplo de 90, usar `Deg0` e emitir
     `PageGeometryDiagnostic::NonStandardRotation`.
4. `UserUnit`:
   - se ausente, usar `1.0`;
   - se presente, **finito e maior que zero**, armazenar;
   - se zero, negativo, `NaN`, infinito ou inválido, usar `1.0` e emitir
     `PageGeometryDiagnostic::InvalidUserUnit`.
   - O valor é **armazenado** mas não altera `width`/`height` na primeira versão — registar
     esta limitação no doc-comment do campo.

Regra de `display_size`: devolve o tamanho visual depois da rotação — para `Deg90`/`Deg270`,
`(height, width)`; para `Deg0`/`Deg180`, `(width, height)`.

Regra de `size_delta`:

- devolve `(other.width - self.width, other.height - self.height)`;
- **não aplica rotação** (compara o tamanho bruto da caixa efectiva — decisão do dono);
- consumidores que precisem de comparação de tamanho visual devem usar `display_size`;
- documentar esta convenção explicitamente no doc-comment do método.

## Resultado esperado

- `01_core/src/entities/page_geometry.rs`:
  - `Rect`
  - `PageBoxModel`
  - `PageRotation`
  - `PageGeometry`
  - `PageGeometryDiagnostic`
  - `resolve_page_geometry`
  - `display_size`
  - `PageGeometry::size_delta`
  - testes inline (`#[cfg(test)]`) cobrindo todos os critérios de verificação

## Critérios de verificação

Dado um `PageBoxModel` com só `media_box` (sem `crop_box`, sem `rotate`, sem `user_unit`)
Quando `resolve_page_geometry` é chamado
Então `width`/`height` vêm da `media_box`, `rotation == Deg0`, `user_unit == 1.0` e não há
diagnósticos

Dado um `PageBoxModel` com `crop_box` diferente de `media_box`
Quando `resolve_page_geometry` é chamado
Então `width`/`height` vêm da `crop_box` (a `media_box` é ignorada para o tamanho)

Dado `rotate: Some(-90)`
Quando `resolve_page_geometry` é chamado
Então `rotation == Deg270` e `width`/`height` **não** são trocados

Dado `rotate: Some(45)`
Quando `resolve_page_geometry` é chamado
Então `rotation == Deg0` e o diagnóstico `NonStandardRotation` é emitido

Dado `user_unit: Some(0.0)` (e, separadamente, `Some(f64::NAN)`)
Quando `resolve_page_geometry` é chamado
Então `user_unit == 1.0` e o diagnóstico `InvalidUserUnit` é emitido

Dado um `PageGeometry` com `width = 100.0`, `height = 200.0`, `rotation == Deg90`
Quando `display_size` é chamado
Então devolve `(200.0, 100.0)`

Dado `PageGeometry` self com `width = 100.0`, `height = 200.0`
E `PageGeometry` other com `width = 100.0`, `height = 200.0`
Quando `size_delta` é chamado
Então devolve `(0.0, 0.0)`

Dado `PageGeometry` self com `width = 100.0`, `height = 200.0`
E `PageGeometry` other com `width = 150.0`, `height = 180.0`
Quando `size_delta` é chamado
Então devolve `(50.0, -20.0)` (convenção `other - self`, com sinal)

Dado `PageGeometry` self com `width = 100.0`, `height = 200.0`, `rotation == Deg0`
E `PageGeometry` other com `width = 100.0`, `height = 200.0`, `rotation == Deg90`
Quando `size_delta` é chamado
Então devolve `(0.0, 0.0)` (documenta que `size_delta` não aplica rotação)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — decisão do dono: `PageBoxModel` bruto + `PageGeometry` resolvido; rotação não troca width/height; absorve `entities/page-geometry.md` | `page_geometry.rs` (revisão) |
| 2026-08-12 | Unificação do tipo numérico para `f64` (consistência com `01_core` existente e precisão em transformações); `CropBox` reformulado como precedência; comportamento explícito para `Rotate` inválido (`NonStandardRotation`) e `UserUnit` inválido (`InvalidUserUnit`); `resolve_page_geometry` passa a devolver diagnósticos | — |
| 2026-08-12 | Revisão do dono: `size_delta` declarado na instrução com regra explícita (tamanho bruto, sem rotação); derives mínimos; restrição explícita de não importar `lopdf`/tipos de `03_infra`; `UserUnit` exige finito e positivo; `Rotate` via `rem_euclid(360)`; `Rect` inválido declarado fora de escopo; âmbito limitado a `page_geometry.rs` (consumidores em prompts próprios); secção "Resultado esperado"; critérios de `size_delta` literais + caso de não-aplicação de rotação | `page-geometry-model.md` |
