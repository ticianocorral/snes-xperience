# Plano de projeto — emulador de SNES com moldura estática

Projeto pessoal, sem fins comerciais. Desktop para Windows, macOS e Linux,
executável standalone.

O jogo roda dentro de uma cena fixa: TV de tubo à esquerda, console com o
cartucho encaixado à direita, e um painel com logo, comandos, cheats e
anotações. Trocar de jogo é um ritual físico — desligar, ejetar, escolher
outro, encaixar, ligar — e pode ser disparado por cartão NFC.

Duas telas:

1. **Seletor** — estante com capas e ficha dos jogos.
2. **Jogo** — a cena fixa com o painel lateral.

---

## 1. Núcleo de emulação

**snes9x via libretro.**

A licença do snes9x permite uso, cópia, modificação e distribuição, em
binário e em código, para fins não comerciais, sem taxa, desde que o aviso
de licença e o copyright acompanhem todas as cópias. Projeto pessoal cai
exatamente nessa faixa.

Vantagens do caminho libretro sobre embutir um emulador direto:

- O core é biblioteca dinâmica com API C estável. Você escreve um
  frontend, não integra C++ alheio no seu build.
- Trocar de console depois é quase de graça: outro core, mesmo frontend.
- Os cheats do `libretro-database` já estão no formato que o core
  consome. Some a camada de conversão.
- O snes9x traz o filtro NTSC do Shay Green (LGPL) na árvore. O
  sangramento de cor composto vem junto.

Duas ressalvas, ambas leves:

- Se um dia virar produto pago, o core precisa trocar. O substituto
  natural é o ares (ISC, v121+).
- Publicando o binário, inclua o texto da licença do snes9x junto.

---

## 2. Arquitetura

Quatro camadas, com dependências só para baixo:

```
Apresentação   seletor, cena do jogo, painel, tela de pausa
Domínio        identificação da ROM, catálogo, cheats, anotações
Emulação       core libretro carregado dinamicamente
Plataforma     janela, áudio, entrada, NFC, sistema de arquivos
```

A camada de emulação nunca sabe que existe uma moldura, e a apresentação
nunca chama o core direto.

### Loop de execução na tela do jogo

O core produz um framebuffer por quadro. A apresentação desenha:

1. camada de fundo (cena pré-renderizada, PNG)
2. framebuffer do jogo, escalado e filtrado (ver 4.7)
3. camada de frente (borda do bezel, reflexo, sombra do móvel)
4. painel lateral, em widgets normais

A cena é imagem, não 3D em tempo real. Renderize em Blender uma vez,
exporte em camadas com a área da tela vazada, e componha em runtime.

### Latência

- O vídeo não pode passar por webview. Isso elimina Electron e Tauri como
  camada de renderização do jogo.
- Implemente *run-ahead* cedo. É o recurso que faz o app parecer melhor
  que o hardware original, e é barato quando o core tem save state
  rápido.

---

## 3. As duas telas

### 3.1 Seletor

É onde vive tudo que serve para **escolher** um jogo: capa, ano,
desenvolvedora, gênero, número de jogadores, região, sinopse. Nada disso
precisa estar visível durante a partida.

- **Navegação por gamepad em primeiro lugar.** O app inteiro é usado com
  controle na mão. Teclado e mouse são o caso secundário.
- **Estado persistente:** último jogado em primeiro, recém-adicionados
  logo abaixo.
- **Preenchimento progressivo.** Na primeira execução a estante não tem
  capa nenhuma. Mostre o item com placeholder imediatamente e preencha
  conforme os metadados chegam. Estante vazia carregando parece app
  quebrado.
- **Busca por digitação** com filtro incremental.

### 3.2 Tela do jogo

Todos os blocos ficam visíveis durante a partida, nesta ordem:

1. **Logo do jogo** — mídia `wheel` do ScreenScraper, PNG transparente.
   Fallback: título em tipografia.
2. **Cartucho encaixado no console**, com o rótulo. É a premissa do app,
   não um adereço — nunca é o primeiro item a ser cortado.
3. **Comandos** — nos botões do próprio console.
4. **Cheats** com interruptor.
5. **Anotações** — miniatura da mais recente e contador.
6. **Tempo de sessão.**

Cabe tudo sem apertar. A coluna tem 480 × 1080 px em Full HD; o orçamento
aproximado é logo 120, console com cartucho 380, comandos 80, cheats 300,
anotações 120, rodapé 60 — cerca de 1060 de 1080. Tipografia normal, de
14 a 16 px. Não reduza fonte para caber: se um dia não couber, a lista de
cheats é que rola, não o resto que encolhe.

Anotações longas, senhas e dicas abrem na pausa, em layout de página
dupla, que é quando existe atenção para ler e escrever.

O bezel e o móvel são **mais escuros que a tela**. A imagem do jogo tem
que ser a coisa mais clara do quadro, como numa TV num quarto de verdade.

### 3.3 Comandos e o ritual de troca

A trava física do Super Famicom define a sequência: no hardware real, a
alavanca de ejetar não se move com a chave ligada.

| Comando | Efeito |
|---|---|
| Desligar | Salva estado. TV vai para sinal off, console apaga, cartucho continua no slot, trava libera. O app continua aberto. |
| Ejetar | Só funciona com o console desligado. Cartucho sai, estante abre. |
| Reset | Reinicia o jogo, sem sair da tela. |
| Sair | Menu ou fechar a janela. Sem cerimônia. |

Ritual completo: desliga, ejeta, escolhe outro, encaixa, liga.

**Console desligado é um estado, não um beco.** Com a TV em sinal off e o
cartucho ainda dentro, o slot ganha um brilho discreto convidando ao
clique. Não abra a estante sozinho — mantenha o controle com o usuário.

**Ejetar com o console ligado.** Padrão: a alavanca resiste, com um clunk
seco. É o que a máquina fazia, e ensina a sequência sem aviso na tela.

Opção ligável para quem quer ver o estrago: congela o último quadro,
embaralha os tiles, estoura a paleta, áudio vira zumbido. Feito só com
pós-processamento do framebuffer, sem tocar no core — o mesmo pipeline
serve depois para simular cartucho mal encaixado. **Salve estado antes de
disparar**, senão o brinquedo vira armadilha.

**Sequência do sinal off:** chuvisco forte por meio segundo, assentando
num ruído baixo e escuro, quase parado. A estante entra por cima, com a
TV sem sinal ao fundo e o slot visivelmente vazio.

Decidir cedo, porque afeta o pipeline: o chuvisco é textura gerada em
shader ou vídeo curto em loop. Como já vai existir pipeline de shader
para o CRT, provavelmente nasce ali — não pesa nada e não engorda o
binário.

Três cuidados obrigatórios:

- **Sem flash de tela cheia**, nem no chuvisco nem no crash. Alto
  contraste piscando é gatilho de fotossensibilidade.
- **Zumbido de RF com corte.** Meio segundo, e silêncio. Ruído branco
  sustentado cansa em segundos.
- **Pular no primeiro botão.** Na décima troca ninguém quer a cerimônia.

**Caminho duplo.** Gamepad não alcança botão clicável. Os mesmos comandos
precisam ser focáveis no painel ou ter atalho de controle. Clique no
console é o caminho para mouse, não o único.

### 3.4 Anotações por jogo

Referência de comportamento: as notas por jogo do Steam Deck.

O uso real em jogo antigo é anotar senha, onde parou e ordem de itens —
ou seja, anotações e senhas são a mesma coisa vista de dois ângulos.

O recurso que resolve as duas de uma vez é **captura de tela colada
dentro da anotação**: o jogador chega na tela de senha, aperta um botão, a
imagem entra no caderno daquele jogo. É como se fazia no papel, e cobre
todos os títulos — inclusive os que você nunca vai documentar à mão.

- Indexe pelo **hash da ROM**, nunca pelo nome do arquivo.
- Guarde como markdown em arquivos soltos, legíveis fora do app, com as
  capturas numa pasta ao lado.
- Escrita acontece na pausa, com teclado. Não tente editor navegável por
  direcional.
- Sem anotação, o bloco some por completo. Nada de "nenhuma anotação"
  ocupando espaço.

---

## 4. Módulos

### 4.1 Identificação da ROM

CRC32, MD5 e SHA1 do arquivo, casados contra os DATs do No-Intro.
Devolve título canônico e região com certeza. Sem isso, rótulo, cheats e
ficha erram juntos. Fallback por nome de arquivo para ROMs com header ou
trimadas.

### 4.2 Catálogo e metadados

- ScreenScraper via `jeuInfos.php`, casando por hash, tamanho e nome.
- Cache local em SQLite, agressivo. A API tem cota horária e diária.
- Busca sob demanda, nunca varredura da biblioteca inteira de uma vez.
- Campo para o usuário cadastrar as próprias credenciais. Conta própria
  tem cota maior e tira o gargalo do app.

### 4.3 Rótulo e logo

Duas mídias do ScreenScraper: `texture` (rótulo do cartucho recortado,
feito para aplicar em modelo 3D) e `wheel` (logo em PNG transparente).

Três detalhes que quebram na prática:

- Rótulos americanos e europeus têm proporções diferentes. Template por
  região, não esticar.
- Nem todo jogo tem as duas mídias. O fallback do rótulo gera a arte a
  partir do nome interno no cabeçalho da ROM, em template neutro.
  Fallback feio faz o app parecer quebrado nos títulos obscuros.
- Se o logo vier em arte gráfica de verdade, tire o texto do rótulo —
  senão o nome aparece três vezes na mesma coluna.

Baixe sob demanda para a máquina do usuário; não empacote arte no
instalador.

### 4.4 Cheats

Pasta `cht` do `libretro-database`, licença MIT, embutida no app, sem
rede. Como o core é libretro, os códigos aplicam direto.

O diferencial é o interruptor, não a lista de strings. Reescreva as
descrições: a lista de endereços é fato bruto, o texto descritivo tem
autor.

### 4.5 Senhas

Não existe base pronta. Duas frentes: tabela manual onde a senha é fixa,
e gerador algorítmico onde ela é calculada. Comece por cinco a dez jogos.

### 4.6 Dicas

Conteúdo escrito por você. Sem scan de revista, sem detonado copiado de
fórum. Como o projeto é pessoal, pode viver em arquivos locais
versionados junto do app, sem backend nenhum.

### 4.7 Cena, escala e shaders

Cena renderizada offline em camadas. Para o CRT, use os presets slang já
existentes em vez de escrever do zero.

**Escala e filtragem — três modos, não dois.**

| Modo | Como funciona | Quando usar |
|---|---|---|
| Pixel perfect | Escala inteira + vizinho mais próximo | Nitidez máxima; sobram barras se a altura não for múltipla |
| Sharp bilinear | Amplia por múltiplo inteiro com vizinho mais próximo, depois bilinear para preencher a sobra | **Padrão.** Preenche a tela mantendo o pixel definido |
| CRT | O shader faz a própria amostragem | Modo autêntico; a escolha acima deixa de importar |

Bilinear puro sozinho não entra: borra sem ganho.

O SNES gera 256×224 com pixel não quadrado, e a proporção correta é 4:3.
Escala inteira exata e 4:3 exato não coexistem. O costume é escalar
inteiro na vertical e aceitar fração na horizontal.

### 4.8 Zaparoo (opcional)

Encostou o cartão NFC no leitor, o cartucho entra no slot na tela.

O Zaparoo Core é o serviço em segundo plano que lê os tokens, dispara as
mídias, gerencia os leitores e expõe a API usada pelos clientes. Roda em
Windows, Linux e SteamOS, cobrindo os três alvos. A API fica em loopback
na porta 7497, e o cliente precisa se parear com o Core antes de
conversar. Além de NFC, aceita QR code, código de barras, amiibo,
Skylanders, LEGO Dimensions e outros tokens.

**Não registre o app como lançador personalizado.** Esse caminho relança
o processo a cada troca de cartão — a moldura pisca, o console reinicia,
a ilusão quebra.

**Faça o inverso:** conecte na API do Core e assine as notificações de
token. Cartão encostado, você recebe o evento e executa a inserção dentro
da cena, sem relançar nada. Existem CLI e documentação de API mantidas
junto do Core para prototipar antes de integrar de verdade.

**Leitura e remoção são eventos distintos.** O cartão fica no leitor
enquanto se joga, e tirá-lo ejeta o cartucho. O objeto na mesa passa a
representar o cartucho na tela em tempo real.

**Janela de tolerância na remoção — obrigatória.** Nunca ejete no
primeiro evento de ausência. Espere dois a três segundos e só ejete se o
cartão continuar fora; se a leitura voltar antes, cancele. Ignore também
sequências de entra-e-sai rápidas demais para serem humanas. Isso vale
mesmo com leitor perfeito: cartão escorrega, mesa treme, alguém esbarra.

**Leitura intermitente no macOS é problema conhecido do ambiente**, não
do app. O caminho costuma ser o PC/SC do sistema, e leitores como o
ACR122U tendem a oscilar ali por disputa do dispositivo. Investigar
libnfc direto em vez de PC/SC, e reportar ao projeto Zaparoo com log — a
comunidade tem mais leitores testados. A janela de tolerância absorve o
sintoma enquanto isso.

**Gesto físico não é recusado pela trava.** A regra de desligar antes de
ejetar vale para os controles na tela. Se o cartão foi encostado com o
console ligado, o app executa a sequência inteira sozinho: desliga,
ejeta, insere, liga.

Depende do Core instalado e rodando, e o app tem que funcionar igual sem
nada disso. Recurso opcional, nunca requisito.

---

## 5. Stack e empacotamento

### Recomendado: C++ ou Rust + SDL3

SDL3 resolve janela, áudio, gamepad e clipboard nos três sistemas com uma
API só. Dear ImGui para as telas utilitárias; desenho próprio para o
seletor, a cena e o painel.

Custo: o visual bonito dá mais trabalho que numa stack web.

### Alternativa: Godot 4

Editor visual ajuda no seletor e na cena, exporta standalone para os três
sistemas e carrega o core via GDExtension. Custo: menos controle sobre
timing de áudio e vídeo, binário maior.

### Empacotamento

| Sistema | Formato | Observação |
|---|---|---|
| macOS | `.app` em `.dmg`, universal (arm64 + x86_64) | Assinatura ad-hoc basta; conviva com o aviso do Gatekeeper |
| Windows | `.exe` portátil | Sem certificado, o SmartScreen alerta na primeira execução |
| Linux | AppImage | Compile em distro antiga para o glibc não travar |

Sem venda, não há necessidade de conta Apple paga nem de certificado de
assinatura.

### CI

GitHub Actions com três runners desde o primeiro commit que compila.
Build cross-platform que só roda na sua máquina quebra em silêncio.

---

## 6. Fases

### Fase 0 — Provar as premissas (1 semana)

1. O core do snes9x carrega e roda pela API libretro nos três sistemas.
2. O ScreenScraper devolve `texture` e `wheel` para uma amostra de 20
   ROMs.

### Fase 1 — Emulador feio que funciona (4 a 6 semanas)

Carrega ROM, roda, som, gamepad, save state, tela cheia, os três modos de
escala. Sem moldura, sem seletor.

### Fase 2 — O seletor (3 a 4 semanas)

Varredura de pasta, identificação por hash, busca de metadados, estante
com capas, navegação por gamepad, preenchimento progressivo.

### Fase 3 — A moldura (4 semanas)

Cena estática composta, shader de CRT, cartucho com rótulo, botões do
console, sinal off, trava de ejeção, transição entre as telas.

### Fase 4 — O painel (4 a 5 semanas)

Logo, cheats com interruptor, tela de pausa, anotações com captura de
tela, e cinco jogos com senha e dica escritas à mão.

### Fase 5 — Empacotar (1 semana)

AppImage, `.exe` portátil, `.dmg`. Sem assinatura paga.

### Fase 6 — Zaparoo (opcional, 1 a 2 semanas)

Cliente da API do Core, pareamento, tratamento de token lido e removido,
janela de tolerância, sequência automática de troca. Só depois que a
transição entre estante e jogo estiver sólida.

Total realista para uma pessoa, em ritmo de projeto pessoal: quatro a
cinco meses até a Fase 5.

---

## 7. Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Estante vazia no primeiro uso | Alto — é a primeira impressão | Preenchimento progressivo com placeholder |
| Moldura cansa em sessão longa | Alto | Teste de duas horas antes de investir na modelagem |
| Projeto travar na Fase 4 | Alto | As fases 1 a 3 já entregam algo usável sozinho |
| Cota do ScreenScraper | Médio | Cache agressivo, credenciais do próprio usuário |
| Fallback de rótulo feio | Médio | Template neutro bem desenhado, não texto solto |
| Leitor NFC oscilando no macOS | Baixo — recurso opcional | Janela de tolerância; investigar libnfc; reportar ao Zaparoo |

Emulador em si é legal. O que atrai problema é distribuir ROM ou BIOS, ou
usar marca e trade dress da Nintendo. Mantenha o console "inspirado em",
não idêntico — vale mesmo em projeto pessoal, caso um dia você publique.

---

## 8. Fora de escopo

- Outros consoles além do SNES.
- Netplay e qualquer recurso online.
- Cerimônia longa de inserir cartucho.
- Backend próprio para dicas.
- Escrever leitor NFC próprio, sem o Zaparoo.

Cada um é um projeto. Nenhum é necessário para descobrir se a ideia
central funciona.

---

## 9. O teste que decide tudo

Duas horas de um RPG longo com a moldura na tela. Se você parar de
enxergá-la, está certa. Se começar a incomodar, é contraste ou saturação
demais competindo com o jogo.

Faça esse teste no fim da Fase 3, antes de investir na modelagem
definitiva e no painel. É a única forma de descobrir se a premissa do
projeto se sustenta.
