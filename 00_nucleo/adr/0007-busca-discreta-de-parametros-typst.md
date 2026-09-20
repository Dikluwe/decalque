# ADR 0007 — Busca discreta e mensurável de parâmetros Typst

**Estado**: aceito
**Data**: 2026-09-19
**Complementa**: ADR 0005 e ADR 0006

## Contexto

O Decalque já materializa e mede uma hipótese tipográfica explícita, mas a iteração seguinte ainda
depende de uma pessoa escolher corpo e tracking. As medidas disponíveis não autorizam uma
conversão direta: altura da bbox não é corpo nominal, largura observada não revela sozinha o
tracking e baseline desconhecida não pode ser substituída pela borda da caixa.

O próprio Typst, as fontes instaladas e o extrator PDF formam a função de medida relevante. Assim,
o primeiro automatismo seguro não é uma fórmula inversa; é uma busca finita em que cada hipótese é
materializada, compilada e comparada pelo ciclo funcional existente.

## Decisão

O Decalque passa a oferecer uma busca discreta por linhas para uma família, peso e estilo fixos.
O espaço fechado varia somente:

- corpo em pontos;
- tracking em pontos.

A busca é exaustiva, sequencial e limitada. Todas as hipóteses existem antes de qualquer leitura de
domínio ou PDF candidato. Resultado algum pode acrescentar, remover ou recalibrar hipóteses durante
a execução.

```text
lista explícita de corpos × lista explícita de trackings
    -> grade canônica e limitada

ScanObservation vinculada + hipótese da grade
    -> ReconstructionPlan
    -> Typst
    -> PDF em memória
    -> DocumentGeometry
    -> ScanComparisonReport
    -> evidência de ajuste pura

evidências de todas as hipóteses
    -> selected | tied | inconclusive
    -> confirmação do selected
    -> publicação exclusiva do PDF confirmado
```

O comando inicial é `fit-scan-lines`. Ele não afirma identidade da fonte nem equivalência
editorial; encontra apenas a melhor hipótese geométrica mensurável dentro da grade declarada.

### Espaço de busca v1

1. família, peso e estilo são uma única hipótese fixa;
2. `--font-size-pt` pode repetir e aceita valores fechados em `[4, 96]`;
3. `--tracking-pt` pode repetir e aceita valores fechados em `[-2, 2]`; quando ausente, a lista é
   `[0]`;
4. `-0` é normalizado para `0` antes de deduplicar;
5. corpos e trackings são ordenados por `f64::total_cmp`; a grade é corpo-major e
   tracking-minor;
6. valores repetidos após normalização são erro de uso;
7. o produto cartesiano deve conter de 1 a 32 hipóteses; não existe flag para elevar esse teto.

Esses limites são política de produto, não inferência da observação. Busca de famílias fica fora
deste corte porque o relatório atual ainda não prova que a família solicitada foi a família
efetivamente resolvida pelo compilador.

### Evidência e elegibilidade

O avaliador puro recebe somente `ScanComparisonReport`. Uma tentativa é elegível quando:

- o escopo de linhas é não vazio;
- `content_status` é `Preserved`;
- cobertura de scan e candidato é integral;
- listas de não associados estão vazias;
- contadores de cobertura são coerentes com `matches` e com as listas de não associados;
- cada `scan_unit_id` horizontal é não vazio e aparece uma única vez;
- cada match possui `dx_start`, `dx_end` e `width_delta` finitos;
- cada match horizontal tem estado `Preserved` ou `Violated`, nunca `Unknown`.

Tentativas inelegíveis permanecem no relatório, com motivo explícito, mas não competem. Se nenhuma
tentativa for elegível, o resultado é `inconclusive` e nenhum PDF é publicado.

As tentativas elegíveis só são comparáveis se tiverem o mesmo conjunto de ids de linha com
evidência horizontal e o mesmo conjunto de ids com baseline conhecida. Suportes diferentes
produzem o motivo público `incomparable-support`; não se escolhe uma omissão como se fosse
resíduo zero.

`reflow` vazio no comparador por linhas não é usado como prova de ausência de recomposição. A
integridade de cada linha física continua protegida pelo emissor; detecção editorial
de reflow por palavras pertence ao estágio posterior.

### Chave de ajuste

Resíduos são quantizados para unidades inteiras de `1/1024 pt`:

```text
q(x) = round_ties_even(abs(x) * 1024)
```

Valor não finito ou cujo quantizado exceda `u64::MAX` é erro de evidência, nunca saturação.

Para cada linha com evidência horizontal:

```text
H_i = max(q(dx_start), q(dx_end), q(width_delta))
```

Para cada baseline conhecida:

```text
B_i = q(baseline_delta)
```

Os vetores `H` e `B` são ordenados do maior para o menor. A chave a minimizar é:

```text
(
  quantidade_de_violações_horizontais,
  H_descendente,
  quantidade_de_violações_de_baseline,
  B_descendente
)
```

Assim, tolerâncias continuam fixas, o pior resíduo prevalece e a soma não depende de ordem. O
`overall_status` não participa da escolha e é preservado literalmente no relatório. Baseline
globalmente desconhecida permite uma seleção `horizontal-only`; ela não se torna zero nem
`Preserved`.

Empate integral da chave quantizada produz `tied` com todos os índices co-vencedores. Ordem da
grade, família, corpo menor ou tracking menor não quebram empate sem evidência. Nenhum PDF é
publicado em `tied`.

### Execução, confirmação e publicação

O compilador é identificado uma vez e a mesma versão textual vincula todas as tentativas. Cada
hipótese é planejada, emitida, compilada e comparada sequencialmente em memória com os limites da
ADR 0006. Nenhum PDF intermediário é publicado.

Todas as tentativas precisam terminar. Falha de planejamento, emissão, compilação, PDF ou
comparação aborta a busca como erro de execução: stdout vazio e nenhum PDF novo. Resultado parcial
jamais escolhe vencedor.

Uma seleção única é compilada e comparada uma segunda vez. Fonte, PDF, relatório comparativo e
chave precisam ser idênticos à tentativa original. Divergência invalida a execução por não
determinismo e impede publicação. Somente o PDF confirmado é publicado, pela mesma operação
atômica sem sobrescrita da ADR 0006, depois da serialização completa do relatório.

`tied` e `inconclusive` são conclusões bem-sucedidas da busca: código zero, JSON em stdout e
nenhum PDF. `selected` retorna código zero, publica exatamente um PDF e não significa por si só
`Preserved`.

### Relatório

O schema `decalque.scan-typography-search`, versão 1, registra:

- fonte da observação e hash do raster;
- família, peso e estilo fixos;
- listas canônicas de corpo e tracking e quantidade de hipóteses;
- versão do compilador;
- para cada tentativa: índice, hipótese, hashes/tamanhos, elegibilidade, suporte, chave e estados
  comparativos;
- seleção `selected | tied | inconclusive`, índices relevantes e escopo da evidência;
- para `selected`, o relatório de avaliação completo da tentativa confirmada.

O relatório não contém timestamp, caminhos, diretório atual nem bytes intermediários.

Os nomes externos são fechados. Cada tentativa usa `eligibility.status` igual a `eligible` ou
`ineligible`; tentativas inelegíveis acrescentam um `reason` entre `empty-scan-scope`,
`content-not-preserved`, `partial-coverage`, `unmatched-units` e
`missing-horizontal-evidence`. Suporte e chave aparecem, respectivamente, como `support` e
`score`, e são `null` quando a tentativa é inelegível. A seleção usa exatamente:

- `selected_index` para `selected`;
- `indices` para `tied`;
- `reason` igual a `no-eligible-trial` ou `incomparable-support` para `inconclusive`.

`evidence_scope` é `horizontal-only` ou `horizontal-and-observed-baseline` quando há uma menor
chave comparável; é `null` em `inconclusive`. `artifact_published` só é `true` em `selected`
confirmado. `winner` contém o relatório de avaliação confirmado somente em `selected` e é `null`
nos demais resultados. Campos alternativos, duplicados ou fora dessa estrutura não pertencem ao
schema v1.

## Responsabilidades por camada

- **L1** calcula elegibilidade, suporte, quantização, chave e seleção sem I/O, Typst, relógio ou
  serialização.
- **L2** analisa a grade fechada, aplica limites, canonicaliza listas e serializa o relatório.
- **L3** identifica uma vez o compilador e reutiliza a fronteira limitada para compilar fontes;
  continua sem escolher hipóteses.
- **L4** vincula observação/raster, executa a grade na ordem canônica, confirma o vencedor e publica
  somente depois do relatório completo.

## Consequências

- Medidas tornam-se parâmetros Typst por experimento reproduzível, não por equivalência nominal.
- `Unknown`, cobertura parcial e suportes diferentes não podem vencer por ausência de resíduo.
- O resultado mostra quando há evidência apenas horizontal.
- A grade explícita é menos eficiente que otimização adaptativa, mas torna orçamento, empates e
  proveniência auditáveis.
- A família continua sendo hipótese fornecida. Busca e prova de identidade de fonte exigem um
  contrato posterior de resolução tipográfica.

## Alternativas rejeitadas

### Derivar corpo da altura da bbox

Rejeitada porque corpo nominal, ascendente, descendente e ink bounds não são equivalentes.

### Resolver tracking por número de caracteres

Rejeitada porque shaping, ligaduras, kerning e fallback tornam a relação dependente da fonte e do
compilador. O PDF real é medido em cada hipótese.

### Tratar `Unknown` como custo zero

Rejeitada porque faria ausência de evidência parecer ajuste perfeito.

### Desempatar pela ordem ou pelo menor parâmetro

Rejeitada porque promoveria uma preferência administrativa a evidência geométrica.

### Busca adaptativa já na v1

Rejeitada neste corte porque resultados candidatos passariam a definir o próprio espaço de busca,
ampliando os estados de parada e dificultando reprodução e ataque por mutação.

## Fora de escopo

- busca, download ou prova de resolução de família;
- peso, estilo, eixos variáveis e features OpenType;
- deslocamento X/Y, margens, tolerâncias, escala, stretch ou baseline artificial;
- leading, parágrafos, hifenização, colunas e mestres de página;
- parâmetros por linha, múltiplas páginas, cache, paralelismo ou busca contínua;
- alterar OCR, texto, ordem, geometria observada, `page_mapping` ou qualquer claim `Unknown`;
- usar pixels como autoridade normativa.
