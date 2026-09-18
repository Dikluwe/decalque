# Segmentação de glifos a partir de uma linha escaneada

**Contexto:** construção experimental de fontes a partir da tinta observada em PDFs.

## Obrigação

Dada uma imagem de uma única linha e sua transcrição Unicode conhecida, separar a tinta em
amostras rotuladas de um caractere. O processo deve usar a imagem como evidência primária,
mas pode usar a transcrição para estimar a ordem, as larguras relativas e os espaços.

## Contrato observável

1. A execução por CLI recebe imagem, texto, linha de base e diretório de saída.
2. Um arquivo PNG é produzido para cada ocorrência não branca, sem sobrescrever caracteres
   repetidos. A ordem e o rótulo Unicode permanecem explícitos em um manifesto JSON.
3. Espaços não produzem glifos; produzem avanço mensurável no manifesto.
4. Cortes podem variar por linha da imagem para contornar antialiasing e letras encostadas.
5. O manifesto registra fronteiras, avanço, linha de base local e confiança de cada corte.
6. Cortes que atravessam proporção excessiva de tinta não são aceitos silenciosamente:
   devem aparecer como `uncertain` e a execução continua para permitir revisão humana.
7. Entradas inválidas, imagem sem tinta e texto sem caracteres visíveis falham sem produzir
   um manifesto aparentemente válido.
8. A saída deve ser diretamente conversível no manifesto aceito por
   `05_scan/scan_font_builder.py`.

## Segmentação guiada por fonte semelhante

Quando uma fonte local semelhante estiver disponível, ela pode ser usada somente como régua
geométrica. O segmentador deve renderizar a transcrição, ajustar tamanho, escala horizontal,
espaçamento e deslocamento à tinta observada e derivar desse ajuste os limites iniciais de cada
caractere. Os contornos da fonte-guia não podem ser copiados para a fonte reconstruída.

Observáveis adicionais:

1. A CLI aceita `--guide-font` e registra no manifesto o método, os parâmetros ajustados e uma
   medida de erro do encaixe.
2. A posição dos glifos deve vir dos avanços e do kerning reais da fonte-guia, incluindo o avanço
   de espaços; pesos genéricos por caractere deixam de ser a fonte primária dos limites.
3. Os limites geométricos podem ser refinados por vales de tinta próximos, mas nunca podem trocar
   a ordem Unicode nem produzir intervalos invertidos ou vazios.
4. A fonte-guia serve apenas para localizar tinta. Os PNGs produzidos devem conter exclusivamente
   pixels provenientes da imagem de entrada.
5. Um encaixe ruim ou um recorte contaminado deve ser marcado `uncertain`, não convertido
   silenciosamente em amostra confiável.
6. Sem `--guide-font`, o comportamento anterior continua disponível.

## Limites explícitos

- A transcrição já conhecida é obrigatória nesta etapa; reconhecer o texto é responsabilidade
  do OCR.
- Ligaduras semanticamente indivisíveis, escrita cursiva e sobreposição severa podem continuar
  incertas.
- Uma ocorrência isolada não basta para alegar reconstrução tipográfica geral.
