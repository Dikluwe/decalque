# Descoberta local de fontes

Estado: ativo
Data: 2026-09-15

## Objetivo

Classificar fontes disponíveis localmente contra evidências raster tipográficas antes de
qualquer consulta externa. A descoberta local é offline, reproduzível e conservadora: ausência
de candidato suficiente solicita fallback, mas nunca inicia rede implicitamente.

## Entrada observável

- manifesto de evidências compatível com `google_fonts_discovery.py`;
- zero ou mais diretórios de fontes explícitos;
- opcionalmente, fontes conhecidas pelo Fontconfig quando nenhum diretório é informado;
- limiar de aceitação no intervalo fechado de 0 a 1;
- limite positivo de resultados.

Arquivos suportados: `.ttf`, `.otf`, `.ttc` e `.otc`, recursivamente. Arquivos idênticos são
deduplicados pelo SHA-256 do conteúdo, mesmo quando aparecem por caminhos diferentes.

## Saída observável

JSON determinístico com:

- `schema_version`;
- `status`: `matched`, `fallback_required` ou `unknown`;
- consulta, limiar e diretórios efetivamente usados;
- candidatos ordenados por escore decrescente e desempate estável;
- família, estilo, caminho, SHA-256, origem, licença quando declarada e escores por evidência;
- rejeições inspecionáveis sem promover arquivo ilegível a candidato.

Quando a evidência contém tinta mensurável, cada comparação por amostra também informa:

- caixa e dimensões observadas da tinta;
- tamanho estimado do corpo em pixels para aquela fonte e variante;
- tamanho estimado em pontos quando `pt_per_px` for fornecido;
- tracking residual estimado em `em` após ajustar o corpo pela altura;
- razão entre largura observada e largura prevista sem tracking.

Cada candidato preserva `units_per_em` e o avanço do glifo `M` em unidades `em`, quando o
arquivo fornece essas métricas. A largura do `M` não é tratada como igual a um quadratim.
O tamanho é ajustado por candidato: uma dimensão absoluta não é inferida antes da família.

`matched` exige que o melhor escore seja maior ou igual ao limiar. Fontes válidas abaixo do
limiar produzem `fallback_required`. Ausência de qualquer fonte comparável produz `unknown`.
A saída nunca contém bytes da fonte e o processo não modifica nem instala arquivos descobertos.

## Segurança e fallback

- Nenhuma operação de rede é permitida neste componente.
- Symlinks para arquivos regulares podem ser lidos e são deduplicados pelo conteúdo.
- Metadados de licença ausentes permanecem `null`; não são inferidos.
- `pt_per_px`, quando informado, deve ser positivo. Sem ele, `estimated_size_pt` permanece
  `null`, sem assumir DPI.
- Evidência sem tinta ou texto com menos de dois caracteres não inventa tracking; o campo
  correspondente permanece `null`.
- `fallback_required` é uma decisão declarativa para um orquestrador externo. Não autoriza
  download, instalação ou consulta à internet.

## Verificação

Testes de execução devem cobrir: correspondência exata, recursão, deduplicação por hash,
ordenação estável, fallback por limiar, ausência de fontes, arquivo inválido e metadados sem
vazamento de conteúdo binário.

## Histórico

- 2026-09-15: contrato inicial para priorizar o banco local e tornar pesquisa externa fallback.
- 2026-09-15: estimativa por candidato de corpo, tracking e métricas do quadratim.
