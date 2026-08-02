# Prompt: `CartesianOrigin`

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/cartesian_origin.rs` + testes no mesmo ficheiro

## Contexto

Segunda pergunta do pipeline. PDFs não têm uma convenção única de eixo — a origem (0,0) do sistema
de coordenadas do content stream, e a direcção em que `y` cresce, dependem de como o produtor
escreveu o PDF (confirmado empiricamente no projeto irmão: cristalino e vanilla, do mesmo `.typ`,
precisaram de detecção heurística de orientação antes de qualquer comparação fazer sentido).

## Restrições estruturais

- L1: zero I/O. Detectar a orientação real de um PDF é trabalho de `03_infra` (inspeccionar a
  `MediaBox` e a matriz de transformação inicial do content stream); esta struct só representa o
  resultado já decidido.

## Instrução

Criar:

```rust
enum AxisDirection { YUp, YDown }

struct CartesianOrigin {
    x: f64,
    y: f64,
    y_axis: AxisDirection,
}
```

Método `fn normalize(&self, point: (f64, f64), page: &PageGeometry) -> (f64, f64)` — converte um
ponto do sistema de coordenadas nativo do PDF (qualquer que seja `y_axis`) para um sistema comum
**y-para-baixo, origem no canto superior esquerdo** (escolha explícita: mais intuitiva para quem
lê um relatório depois, "y maior = mais para baixo na página impressa" — documentar esta escolha
no doc-comment, não deixar implícita).

## Critérios de verificação

Dado uma origem com `y_axis: YDown` e um ponto qualquer
Quando `normalize` é chamado
Então o ponto devolvido é idêntico ao original (já está no sistema alvo)

Dado uma origem com `y_axis: YUp`, uma `PageGeometry` de altura conhecida, e um ponto (x, y)
Quando `normalize` é chamado
Então o `y` devolvido é `page.height - y` (mais a translação de `self.y`, se não-zero — especificar
a fórmula completa nos testes, não só o caso `self.y == 0.0`)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| (preencher na execução) | Criação inicial | `cartesian_origin.rs` |
