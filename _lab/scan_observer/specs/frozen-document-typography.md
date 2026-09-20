# Prompt: descoberta e congelamento da tipografia documental

**Camada**: análise geométrica e contrato editorial do Caso 2
**Depende de**: `editorial-typst-composition.md`

## Obrigação

Separar estritamente: normalização geométrica, descoberta de métricas, congelamento do perfil,
aplicação e auditoria. O loop existe somente durante a descoberta. Depois de congelado, o perfil
não pode ser modificado por uma página, linha ou diferença isolada.

Antes das métricas, cada página deve ter inclinação estimada e corrigida. A transformação entre
scan e página normalizada deve ser registrada. Perspectiva só pode ser corrigida quando quatro
bordas confiáveis forem observadas; caso contrário permanece `unknown`.

## Hierarquia

1. documento/seção: famílias e mestres de página;
2. classe editorial: tamanho, peso, estilo, entrelinha e alinhamento;
3. bloco: coluna, recuo e tabulação;
4. linha: quebra, baseline e âncoras de primeira/última palavra;
5. glifo: proporção natural, sem escala horizontal.

## Auditoria

Após o congelamento, divergências produzem marcações de revisão com causa provável. A auditoria
não ajusta fonte, tamanho, tracking, entrelinha, coluna ou recuo. O hash do perfil antes e depois
deve permanecer idêntico.
