# Descoberta tipográfica via Google Fonts

**Estado**: ativo
**Contexto**: `scan-typst-materialization.md`

## Obrigação

Dado um recorte contendo uma única linha e sua transcrição, consultar o catálogo oficial da
Google Fonts, baixar um conjunto limitado de variantes, renderizar localmente a transcrição e
classificar candidatos pela semelhança visual. A API produz candidatos; não prova identidade.

## Fronteira externa

- A chave vem de `GOOGLE_FONTS_API_KEY` ou `--api-key` e nunca aparece em stdout, erros ou JSON.
- A URL padrão é `https://www.googleapis.com/webfonts/v1/webfonts` e pode ser substituída em
  testes de processo.
- `category`, `sort`, variante solicitada e limite de downloads são parâmetros registrados.
- Ausência de chave na URL oficial encerra com código 2; não troca silenciosamente de provedor.
- Somente URLs `https` do catálogo são aceitas na execução oficial. Testes podem habilitar HTTP
  local explicitamente com `--allow-insecure-localhost`.

## Comparação

1. Normalizar o recorte para máscara de tinta, removendo margens brancas.
2. Renderizar o texto informado com cada arquivo em tamanho grande e fundo branco.
3. Normalizar sem alterar proporção e medir IoU da tinta, incluindo versão suavizada por
   antialiasing.
4. Publicar `family`, `variant`, `category`, URL de origem, caminho em cache e `similarity`.
5. Ordenar deterministicamente por similaridade decrescente e família crescente.

O resultado é `ranked` quando ao menos uma fonte válida foi comparada; respostas sem variantes,
downloads inválidos ou fontes ilegíveis são registradas em `rejected`, nunca omitidas. O cache
usa hash da URL e não baixa novamente um arquivo existente.

## Evidências discriminatórias múltiplas

Além do modo de uma amostra, `--evidence` aceita JSON com `samples`. Cada amostra declara `path`,
`text`, `weight > 0`, uma ou mais `dimensions` dentre `family`, `weight`, `style`, e `variants`
permitidas. Caminhos relativos resolvem contra o arquivo de evidências.

Para cada família, cada amostra escolhe a variante disponível com maior similaridade. O relatório
preserva todos os escores por amostra, a variante vencedora e calcula:

- `family_score`, `weight_score` e `style_score`: média ponderada apenas das evidências que
  declararam a dimensão;
- `overall_score`: média ponderada de todas as evidências comparáveis.

Dimensão sem evidência comparável permanece `null`. Uma família só entra no ranking se cobrir
todas as amostras obrigatórias; ausência de todas as variantes permitidas gera rejeição explícita.
Empates são resolvidos por nome da família. O `g` do título e o `y` do subtítulo podem, assim,
pesar mais que palavras com formas menos discriminatórias e testar variantes diferentes da mesma
família (`700`, `italic`, `700italic`, por exemplo).

## Critérios de verificação

1. Processo HTTP simulado recebe categoria, ordenação e chave sem que a chave apareça na saída.
2. Amostra gerada com uma fonte real classifica essa fonte acima de uma família visualmente distinta.
3. Resposta sem variante solicitada aparece em `rejected`.
4. Ausência de chave para a URL oficial falha antes da rede.
5. Limite impede downloads além do número configurado.
6. Evidências `g` e `y` selecionam variantes distintas da mesma família e produzem escores
   separados de família, peso e estilo.
7. Família que não cobre uma amostra obrigatória não aparece no ranking.
