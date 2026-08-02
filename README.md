# Decalque <sub>_comparação geométrica de PDFs, sem OCR, sem pixel_</sub>

Ferramenta genérica para comparar a posição real de cada traço/glifo desenhado entre dois PDFs
quaisquer — não compara pixels renderizados (sensível a DPI, antialiasing, tipo de rasterizador) e
não compara texto extraído (cego a duplicação/posição, como já provou ser insuficiente no projeto
que motivou esta ferramenta). Lê os operadores de desenho do content stream diretamente e reporta
a diferença de posição real, em pontos.

Construída seguindo a Arquitetura Cristalina (Tekt) — `00_nucleo` é o ponto de partida para
qualquer trabalho neste projeto.

*Decalque*: papel de decalque reproduz a posição exata de cada traço de um desenho original,
por transferência directa — não por olhar e redesenhar. É essa a diferença entre esta ferramenta
e comparar pixels renderizados ou texto extraído: em vez de reinterpretar o que se vê, ela lê a
posição real de cada traço directamente da fonte (o content stream), como o papel encostado ao
desenho original.

---

## O pipeline central (o problema em três perguntas)

Dado um PDF, a ferramenta responde, em ordem, três perguntas — cada uma é uma peça de domínio
própria, e a resposta de uma condiciona a seguinte:

1. **Qual é o tamanho da folha?** (`PageGeometry`) — a `MediaBox` (ou equivalente) de cada página.
   Dois PDFs do "mesmo" documento podem legitimamente ter folhas de tamanhos diferentes (por
   exemplo, `width: auto` resolvendo para valores distintos em compiladores distintos) — isto não
   é por si só uma divergência a reportar, é o contexto que torna o resto da medição possível.
2. **Onde fica o ponto (0,0)?** (`CartesianOrigin`) — PDFs não têm uma convenção única de eixo
   (y pode crescer para cima ou para baixo, dependendo de como o produtor do PDF escreveu o
   content stream). Sem normalizar isto primeiro, qualquer comparação de posição está a comparar
   sistemas de coordenadas diferentes sem saber.
3. **Com que resolução/tolerância a posição é medida?** (`MeasurementResolution`) — dois traços
   "na mesma posição" na prática nunca têm exatamente o mesmo float; é preciso um limiar. E esse
   limiar provavelmente não deveria ser um valor absoluto fixo (0.5pt faz sentido para texto de
   corpo, mas é enorme relativo a um índice de 6pt, e minúsculo relativo a um título de 40pt) —
   candidato natural: resolução relativa ao tamanho de fonte/em do que está a ser medido, não um
   número absoluto único para o documento inteiro. **Isto é uma decisão de desenho ainda aberta,
   não uma resposta já dada** — ver `00_nucleo/prompts/entities/measurement-resolution.md`.

Só depois destas três respostas é que faz sentido comparar dois documentos: extrair a lista de
glifos desenhados de cada um (`GlyphInstance`), emparelhar os correspondentes entre os dois
(por conteúdo — codepoint via `ToUnicode` — e ordem de leitura, não por posição absoluta, que é
precisamente o que está a ser medido), e calcular o delta de posição de cada par, relativo à
origem apropriada (não necessariamente a origem da página — pode ser a origem do cluster/
construção a que o glifo pertence, para não confundir "página maior" com "glifo deslocado").

## Por que isto existe

Um projeto irmão (typst-crystalline, arquitetura Tekt) construiu uma primeira versão disto como
script interno (`tools/geometry/compare.py`, Python, `pikepdf`) para resolver um problema concreto:
comparações visuais e por texto extraído produziram conclusões contraditórias entre passos
consecutivos sobre se um delimitador matemático estava "duplicado" ou não — só resolvido descendo
ao content stream bruto com posições. Esta ferramenta generaliza essa solução para qualquer par de
PDFs, e reimplementa em Rust para poder ser embutida/reusada (não só um script standalone).

## Estrutura

```
decalque/
├── 00_nucleo/     # Prompts e ADRs (a Semente)
├── 01_core/       # Modelo de domínio puro: PageGeometry, CartesianOrigin,
│                  # MeasurementResolution, GlyphInstance, DocumentGeometry,
│                  # algoritmo de emparelhamento/diff. Zero I/O.
├── 02_shell/      # CLI
├── 03_infra/      # Leitura/parsing real de PDF (content stream, ToUnicode)
├── 04_wiring/     # main.rs, composição
└── _lab/          # Experimentos isolados
```

## Estado actual

Fase de nucleação — `00_nucleo/prompts/entities/` já tem as specs do domínio central (ver ficheiros
individuais). Nenhum código Rust ainda. Próximo passo: revisão das specs, depois primeira geração
de `01_core`.
