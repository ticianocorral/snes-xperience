# Changelog

Todas as mudanças relevantes deste projeto são registradas aqui.

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e o
versionamento segue [SemVer](https://semver.org/lang/pt-BR/). Enquanto a versão
for `0.x`, a API das crates e a interface de linha de comando podem mudar sem
aviso — só o incremento de _minor_ marca um conjunto de mudanças.

## [0.8.0] - 2026-09-16

### Adicionado

- **Anotações de texto viram 15 slots visíveis, iguais aos prints**: a
  página esquerda do caderno (antes só um botão "Escrever anotacao" que
  sempre acrescentava uma página nova a um arquivo só, sem jeito nenhum
  de ver o que já tinha sido escrito) agora mostra o conteúdo salvo,
  com `< anterior`/`proxima >` pra navegar entre as 15, "Fixar" (protege
  de sobrescrita e exclusão, igual já funcionava pros prints),
  "Escrever"/"Editar" (pré-preenchido com o texto já salvo) e um "Apagar"
  novo (desabilitado num slot fixado ou vazio). Anotações escritas antes
  dessa revisão (no antigo `notas.txt`) são migradas automaticamente pros
  slots numerados na primeira vez que o jogo abre depois da atualização.

- **Busca na modal de Cheats**: um campo "Buscar..." filtra a lista pelo
  texto digitado (case-insensitive, em qualquer parte da descrição) —
  necessário depois da base crescer pra milhares de códigos por jogo em
  alguns casos.

### Corrigido

- **Textos quebrados/sobrepostos nas modais** (Cheats, mas o bug valia
  pra qualquer botão): um rótulo maior que a caixa desenhava por cima da
  coluna vizinha em vez de cortar — inofensivo enquanto todo rótulo era
  curto ("Slot 3"), virou visível assim que a modal de Cheats passou a
  mostrar descrições da base do libretro-database. Rótulo longo demais
  agora corta com reticências (`...`); a coluna também ficou bem mais
  larga por padrão (calculada a partir do rótulo mais comprido da lista,
  não mais um valor fixo de 200px), então a maioria nem chega a cortar.
- Descrições de cheat que vinham com `&quot;`/`&amp;`/etc. escapado (HTML)
  na base agora aparecem com o caractere de verdade.

## [0.7.0] - 2026-09-16

### Adicionado

- **Cheats ganharam seu próprio menu**: um botão "Cheats" na lista de
  comandos (ausente quando a cartucho não tem nenhum código curado) abre
  uma modal com a lista de códigos — clicar num deles liga/desliga na
  hora, sem fechar a modal, igual ao antigo checklist do caderno.
- **Sistema de pin no painel**: o bloco "notas" do painel lateral só
  aparece quando pelo menos um dos 15 slots está fixado — antes mostrava
  a miniatura do slot mais recentemente visto/capturado mesmo sem
  ninguém ter pedido pra featurar aquilo ali.
- **Rolagem na modal de Cheats** (e, embutido de graça, em qualquer
  modal futura com muitas linhas): listas de até ~1200 linhas por coluna
  mostram só uma página por vez, com botões "Cima"/"Baixo" e um contador
  ("1-12/1209") — necessário depois da base de cheats crescer (ver
  Alterado).

### Alterado

- **Base de cheats cresceu de ~20 jogos curados pra base inteira do
  `libretro-database`** (~2400 jogos, ~67 mil códigos) — antes só um
  punhado de jogos escolhidos a dedo tinham cheat nenhum; agora qualquer
  ROM cujo nome de arquivo bate com a convenção usual
  (`Titulo (Regiao).sfc`) tem uma chance real de ter cheats prontos. Como
  a base não tem como saber o título interno do cabeçalho de 2000+
  cartuchos sem possuí-los, o casamento mudou de "título do cabeçalho
  SNES" pra "título do arquivo da ROM" (mesma string que `saves/`/`notes/`
  já usam) — ver `crates/domain/src/cheats.rs`.
- **Cheats saíram do caderno de anotações**: a lista de códigos (com
  checkbox) que vivia na página esquerda do caderno agora é a modal
  acima — o caderno voltou a ser só "anotacoes" (texto livre + álbum de
  prints).
- **Corrigido: desligar um cheat não desligava o efeito.** Um único
  `retro_cheat_set(indice, false, ...)` não é suficiente pra desfazer um
  patch de memória sustentado em todo core — a linha aparecia como
  desligada mas o jogo continuava com o cheat ativo. Agora todo toggle
  reseta e reaplica o estado inteiro de cheats (mesma sequência que o
  carregamento inicial já usava).
- **Corrigido: um código de cheat longo o bastante travava o app** —
  reproduzido de verdade durante o desenvolvimento (`SIGABRT`, estouro de
  buffer na pilha dentro do `retro_cheat_set` do core snes9x-libretro,
  com um código de 602 caracteres/67 endereços encadeados). A geração da
  base agora descarta qualquer código acima de 96 caracteres, e
  `cheats.rs` tem um teste de regressão fixando esse limite.

### Removido

- **"Avancar quadro" do caderno de anotações** — um botão de depuração
  (avançar um quadro com o jogo pausado) que não tinha uso real pra quem
  joga; o "Continuar" que sobrou ocupa a largura toda no lugar dos dois
  meio-a-meio.

## [0.6.0] - 2026-09-15

### Adicionado

- **Modais de Salvar/Carregar/Printscreen**: clicar em qualquer um dos três
  pausa o jogo e abre uma tela dedicada com os slots pra escolher — 10 pra
  save state, 15 pro print — em vez de um contador que precisava ser
  clicado várias vezes antes. O print ganhou uma etapa a mais: depois de
  escolher o slot, um campo de texto (opcional, até 40 caracteres) nomeia
  a captura.

### Alterado

- **Botão "Pausar" removido — "Nota" virou "Anotacoes"**: agora é o único
  caminho pro caderno (pausa o jogo e já abre a tela de anotações/cheats).
  A captura de tela que "Nota" fazia direto virou o botão "Printscreen"
  (ver Adicionado).
- **`saves/` organizado por jogo**, mesma convenção de `notes/`:
  `saves/<título>/0.state` .. `9.state`, `sram.srm`, `cheats.txt` — antes
  eram arquivos soltos direto em `saves/`, nomeados pelo hash SHA1 da ROM
  (`<hash>.state0`, `<hash>.srm`, `<hash>.cheats`), ilegíveis por quem
  fosse abrir a pasta a mão. Efeito colateral bem-vindo: uma ROM que falha
  a identificação (hash indisponível) agora salva normalmente — antes
  ficava sem save state nenhum ("no state slot (unidentified ROM?)"),
  já que não tinha hash pra nomear o arquivo.

### Removido

- **Botão "Turbo"** e o fast-forward que ele ligava — junto com o próprio
  conceito de rodar quadros extra sem áudio/run-ahead
  (`FF_SPEED`/`Input::fast_forward`).

## [0.5.0] — 2026-09-15

### Adicionado

- **Botão "Configuracoes" na tela inicial**, ao lado de "Inserir cartucho"
  — antes configurações só abria pela tecla `O` na estante, sem caminho
  nenhum por mouse/gamepad a partir da tela inicial.
- **Comandos novos no painel, em jogo**: Pausar, Nota, Nota slot,
  Salvar/Carregar (slot atual), Slot (cicla) e Turbo — todos clicáveis, ao
  lado dos já existentes Power/Ejetar/Reset.
- **Caderno de pausa interativo**: botões "Continuar" e "Avancar quadro" —
  antes era só leitura, exigia a tecla `P` pra sair.
- **Botões "Voltar"/"Configuracoes" na estante**, no rodapé da ficha.
- **Arte de cartucho no painel** (`assets/cartridge/<rom>.*`, mesma
  convenção de nome do logo/capa) — mostrada abaixo do logo, independente
  dele.
- **15 slots fixos de captura por jogo** (`notes/<nome do jogo>/01.png` ..
  `15.png`, botão "Nota slot" escolhe qual): o caderno de pausa pagina por
  eles ("< anterior"/"proxima >", contador "N/15", "vazio" pros que não
  têm nada ainda).
- **Anotação por texto no caderno de pausa**: botão "Escrever anotacao" na
  página esquerda abre um editor de texto livre (até 240 caracteres,
  contador ao vivo), salvo à parte em `notes/<nome do jogo>/notas.txt`.
- **Fixar e nomear slots de nota**: "Fixar" trava o slot mostrado contra
  sobrescrita — "Nota" pula pro próximo slot livre em vez de gravar em
  cima; com os 15 fixados, o próprio botão avisa "sem espaco" em vez de
  falhar em silêncio. "Nomear print" dá um título curto (até 40
  caracteres) ao slot, mostrado junto da imagem — mesmo editor de texto da
  anotação livre, reaproveitado.
- **Feedback "(feito!)"** nos botões Nota/Salvar/Carregar — clicar mostrava
  zero indicação na tela antes (um usuário clicou Nota sete vezes achando
  que não tinha funcionado — tinha, todas as vezes).
- **Dica "(pausar pra editar)"** no texto informativo de cheats do painel —
  sem ela não havia como saber que o interruptor de verdade mora no
  caderno de pausa.
- **Logo do console na tela inicial** (`assets/console.png`, opcional) —
  mostrado no lugar do logo do jogo; sem o arquivo, cai pro nome
  "SNES Xperience" em texto, mesmo comportamento do logo por jogo faltando.

### Alterado

- **Fonte maior e antisserrilhada** em todo o app (Noto Sans Mono, ~2.5×
  maior que o bitmap 8×8 original) — também ganhou suporte a acentos
  (á, ã, ç, é, õ...), que antes viravam `?`.
- **Nenhum comando usa mais teclado** — Power/Ejetar/Reset/Pausar/save-load
  state/slot/turbo/nota e toda navegação de menu (estante, configurações,
  tela inicial, caderno de pausa) são só mouse/gamepad agora. A única
  exceção deliberada é a anotação por texto (ver acima) — escrever texto
  exige teclado por definição. O único OUTRO teclado que sobra por padrão
  é o D-pad/botões do próprio SNES (nem todo mundo tem gamepad pra jogar).
  A tela de configurações ganhou clique de verdade (não tinha nenhum
  antes).
- **Cheats saíram do painel, foram pro caderno de pausa**: no painel (jogo
  rodando) agora são só texto informativo — os que estão ligados, sem
  clique; o interruptor de verdade (clique liga/desliga) mudou pro caderno
  de pausa, que tinha espaço de sobra e já parava o jogo de consumir
  input. Abriu espaço no painel pra arte de cartucho e os comandos novos.
- **Busca por digitação removida da estante** — não sobra pra que serviria
  sem teclado.
- **Capas da estante em paisagem, não retrato** (200×150, ~30% maiores) —
  a arte que os jogadores realmente têm é capa de frente horizontal, não
  retrato estilo lombada; o tile antigo (150×200) deixava a imagem
  espremida numa moldura alta com sobra de espaço vazio.
- **Arte de cartucho maior no painel** (90px → 150px de altura).
- **Power e Reset viraram chaves gangorra roxas**, estilizadas como as do
  console de verdade, no lugar de mais duas linhas de texto — Power é um
  alternador de verdade (fica onde foi deixado), Reset é momentâneo (sobe
  no clique, desce sozinho logo em seguida). Ejetar ficou entre as duas,
  no mesmo espaço que o slot do cartucho ocupa no hardware real.
- **Escolher um jogo só insere o cartucho, não liga o console sozinho** —
  igual ao hardware de verdade: a tela mostra o cartucho encaixado e o TV
  ainda apagado até o jogador clicar Power. (A captura headless de
  desenvolvimento, `emu-run --shot` sem `--shot-off`/`--debug-shot-pause`,
  seguiu ligando sozinha — não há como clicar Power num teste sem tela.)
- **Tela inicial = tela de "cartucho ejetado"**, a mesma tela agora nos
  três casos (abrir o app, voltar da estante, ejetar um jogo): logo do
  console no lugar do logo do jogo, "Inserir cartucho" no lugar da arte
  de cartucho, "Configuracoes" movido pro rodapé do painel (mesmo lugar
  do relógio de sessão durante o jogo) em vez de empilhado com "Inserir
  cartucho".

### Removido

- **Botão "Screenshot"** (BMP da janela inteira) — não sobrou uso real pra
  ele depois que "Nota" passou a servir de registro do jogo.

Ver `docs/fase-0.md`/`fase-2.md`/`fase-3.md`/`fase-4.md`, seções
"Revisão (2026-09-14)", pelo detalhe completo desta leva.

## [0.4.1] — 2026-09-14

### Corrigido

- **Pastas no macOS**: a raiz portátil (`roms/`, `core/`, `assets/`,
  `saves/`, `notes/`, `xperience.cfg`) ficava ao lado do `.app` — na
  prática, dentro de `/Aplicativos` depois de instalar pelo DMG, o que não
  é gravável/esperado nesse SO. No macOS a raiz agora é sempre
  `~/Documents/SNES Xperience` (criada no primeiro uso), independente de
  onde o `.app` foi parar; Windows/Linux continuam com a pasta ao lado do
  executável. Uma migração automática (mesmo espírito da migração de
  `saves`/`notes` do local antigo pré-portátil) copia o conteúdo da raiz
  antiga ao lado do `.app` pra dentro de `~/Documents/SNES Xperience`, uma
  vez, se esta ainda estiver vazia — quem já rodou o DMG 0.4.0 e colocou
  ROMs lá não perde nada.

## [0.4.0] — 2026-09-14

### Adicionado

- **Tela inicial** (TV off + botão "Inserir cartucho" no lugar do logo):
  agora é o estado raiz do app — aparece na abertura, depois de ejetar um
  jogo e ao dar Esc na estante (que antes fechava o app direto).
- **Botões clicáveis** de Power/Ejetar/Reset no painel lateral, no lugar da
  legenda de texto — primeiro suporte a clique de mouse no app.
- **Estante clicável**: clicar numa capa/linha seleciona, clicar de novo
  lança o jogo.
- **Lista estilo multicart** quando nenhum jogo da estante tem capa
  carregada ainda, em vez de uma grade de tiles vazios.
- A estante agora mostra o chuvisco de sinal off levemente por baixo o tempo
  todo (antes só durante a entrada vinda de um eject).

### Corrigido

- Depois de ejetar, o painel ficava com os botões do jogo anterior em vez do
  botão "Inserir cartucho" — não dava pra abrir a estante de novo por
  clique. `Cabinet::clear_panel` (novo) limpa o painel ao entrar na tela
  inicial.
- **Ligar de novo**: desligar o console (`Esc`/botão Power) era uma via de
  mão única — só dava pra ejetar depois. `Esc`/Power agora alternam os dois
  sentidos; ligar de novo retoma o jogo exatamente de onde parou (sem
  recarregar), com um chuvisco espelhado (`power_on_burst`) subindo até um
  pico breve. O botão do painel também troca a legenda pra "Ligar" e fica
  aceso nesse estado, em vez de ficar preso em "Desligar" apagado.

### Alterado

- O cartucho não aparece mais encaixado no console durante a partida.

### Removido

- **ScreenScraper**: sem raspagem online — capa e logo agora vêm de arte
  local em `assets/cover/`/`assets/logo/`, casada pelo nome do arquivo da
  ROM. A ficha da estante perdeu ano/desenvolvedora/gênero/sinopse; sobrou
  título + contagem de jogadas.
- **Catálogo SQLite**: sem banco — o app escaneia `roms/` a cada abertura e
  guarda só contagem de jogadas/datas num `library.json` ao lado do
  executável. Binários `library` e `scrape-test` removidos (sem banco pra
  popular/inspecionar, sem ScreenScraper pra testar).

### Adicionado (continuação)

- **App portátil**: `roms/`, `core/`, `assets/{cover,logo,cartridge}/`,
  `saves/`, `notes/`, `xperience.cfg` e `library.json` moram ao lado do
  executável (do `.app` no macOS, não dentro dele) — sem instalação, sem
  `~/.local/share`. Uma cópia única do `saves/`/`notes/` antigo migra
  sozinha na primeira execução, se existir.
- **Nomes via No-Intro**: um `nointro.dat` opcional ao lado do executável dá
  o título canônico de cada jogo (casado pelo CRC32 headerless), resolvido
  no scan — sem raspagem, sem espera.
- **Baixar/atualizar o núcleo snes9x** pelo menu de configurações, direto do
  buildbot do libretro — sem precisar colocar o arquivo à mão.
- **Moldura trava em 16:9**: num monitor ultrawide (ou janela redimensionada
  pra uma forma esquisita) sobra faixa preta nas laterais em vez de esticar
  o gabinete/tubo.

## [0.3.1] — 2026-09-11

### Corrigido

- **DMG "corrompido"**: era o Gatekeeper, não corrupção de verdade — sem
  assinatura nenhuma, o macOS recusa um `.app` baixado da internet com
  "está danificado", sem alternativa. `build-dmg.sh` agora assina ad-hoc
  (`codesign --force --deep --sign -`, sem precisar de conta de
  desenvolvedor Apple); o aviso vira o padrão "desenvolvedor não
  verificado", com "Abrir mesmo assim". Notarização de verdade (sem aviso
  nenhum) precisaria de conta paga da Apple — fica pra depois, se topar.

## [0.3.0] — 2026-09-11

### Adicionado

- **Menu de configurações**: `O` na estante abre controles (rebind das 27
  ações, tecla capturada na hora), ScreenScraper (ativar + Dev ID/Dev
  Password) e run-ahead/tela cheia — três listas achatadas, salva em
  `config.toml` a cada mudança. `config.toml` ganhou `[screenscraper]`;
  `Config::resolve_screenscraper` prioriza isso sobre `$SS_DEVID`/
  `$SS_DEVPASSWORD` (que continuam funcionando como fallback).
  `Platform::poll_menu` virou `poll_menu(MenuMode)` (`Nav`/`TextEntry`/
  `CaptureKey`) — a plataforma não tinha como capturar uma tecla crua pra
  rebind nem digitar texto além de minúsculas+espaço; `char_for_key` agora
  lê Shift pra maiúscula/símbolo.
- **Pacotes de verdade**: `.github/workflows/release.yml` gera, a cada tag
  `vX.Y.Z`, um DMG (macOS), um zip com os `.exe` autocontidos (Windows) e
  um AppImage (Linux) — as três com SDL3 vendorizado, sem depender de nada
  instalado na máquina de quem baixa. Scripts em `packaging/` (`build-dmg.sh`,
  `build-appimage.sh`) montam o pacote a partir do binário já compilado;
  `crates/app/build.rs` embute o ícone no `.exe` do Windows. O core do
  snes9x e as ROMs continuam de fora (ver `THIRD-PARTY-NOTICES.md`).
- **Diretório de dados multiplataforma**: `xperience_app::dirs` resolve
  `~/.local/share/snes-xperience` / `~/.config/snes-xperience` quando
  `$HOME` existe (todo o comportamento de antes, sem mudança) e cai pra
  `%APPDATA%\snes-xperience` quando não existe — o caso de um `.exe`
  aberto no Explorer sem terminal nenhum por perto, onde `$HOME` nunca
  esteve definido.
- **Núcleo padrão**: sem `--core`/`$XPERIENCE_CORE`, o `xperience` agora
  procura `snes9x_libretro.{dylib,so,dll}` em `<diretório de dados>/core/`
  antes de desistir — o caminho que sobra pra um pacote de verdade (DMG/exe/
  AppImage), que abre sem argumento nenhum.

## [0.2.0] — 2026-09-11

Fases 2, 3 e boa parte da 4 do [plano](docs/plano-emulador-moldura.md): o
catálogo e a estante, a moldura de verdade (gabinete, tubo, ritual de
ligar/desligar), e o painel lateral com comandos, cheats, caderno de
capturas e tela de pausa. Sem anotação escrita por teclado ainda (falta o
subsistema de entrada de texto) e sem a tabela manual de senhas/dicas.

### Adicionado

- **Catálogo** (`xperience-domain`): `library::scan` varre uma pasta
  recursivamente e hasheia os ROMs; `Catalog` é um cache SQLite (`rusqlite`
  bundled) com as tabelas `rom` e `meta`, ordenação de estante (§3.1: último
  jogado → recém-adicionado), `unscraped`, `prune_missing`.
- **ScreenScraper**: `GameInfo` (antes `GameMedia`) traz ano, desenvolvedora,
  editora, gênero, jogadores, sinopse e a capa `box-2D`; `Client::download`
  salva arte em disco; erro de cota agora é distinto.
- Binário **`library`** (`scan` / `scrape` / `list`) para montar e preencher o
  catálogo pela linha de comando.
- **Seletor** (`selector`): estante rolável de capas com painel de detalhes,
  navegação por gamepad/teclado, busca por digitação e preenchimento
  progressivo das capas numa thread. Imprime o caminho da ROM escolhida.
  Camada 2D nova em `xperience-platform` (`Ui`: rects, texto `font8x8`,
  imagens) e `Platform::poll_menu`.
- Binário **`xperience`**: estante → jogo → estante num processo só, sem shell.
  O laço do emulador virou `xperience_app::runner::run_game` e o da estante
  `xperience_app::shelf::run`; `emu-run` e `selector` agora são cascas finas em
  volta desses módulos. `UiEvent::CloseRequested` novo separa "voltar" (Esc) de
  "encerrar" (fechar janela / Cmd-Q). `scripts/play.sh` removido (obsoleto).
- **Scrape sob demanda** na estante: com `SS_DEVID` / `SS_DEVPASSWORD` no
  ambiente, o jogo em foco sem ficha é scrapeado numa thread (um pedido por
  jogo, ~700 ms entre chamadas, para ao esgotar a cota); a ficha e a capa
  entram na hora. `--no-scrape` desliga. O download de arte virou
  `xperience_domain::art::download_art`, compartilhado com o `library scrape`.
- **Ficha completa** no painel: logo `wheel` no topo quando existe (senão o
  título em texto) e sinopse longa numa região recortada (`Ui::clip`) que rola
  sozinha após ~1,3 s parada. `Ui::wrapped_height` (com teste) dimensiona o
  scroll; o decodificador de arte agora também trata os `wheel` (≤512 px, alfa).
- **Gabinete atrás do tubo** (Fase 3): a tela do jogo é recuada num gabinete
  escuro — limpa com a cor do recuo, desenha uma malha de anel
  (`build_bezel_mesh`) da borda da janela até a tela e então o tubo CRT dentro.
  `screen_area` / `fit_aspect_in` dão o vão 4:3 com queixo maior; a malha é
  cacheada por tamanho. A imagem do jogo fica sendo a coisa mais clara do quadro.
- **Janela única** (Fase 3): `Ui` + `Video` viraram um tipo só, `Cabinet`, que
  tem o caminho do jogo (`present_frame`) e o 2D da estante. `xperience` cria um
  `Cabinet` e passa `&mut` dele pra `shelf::run` e `runner::run_game` — a troca
  estante↔jogo não recria janela. Testes de `screen_area` / `fit_aspect_in`.
- **Estante pelo tubo** (Fase 3): o 2D da estante virou `Cabinet::frame_2d(bg,
  |Screen| …)` — o closure desenha num buffer do tamanho do vão, que é deformado
  pela mesma malha CRT do jogo. `capture_2d` salva isso headless (`selector
  --shot`).
- **Sinal off** (Fase 3): a queda de sinal toca ~0,65 s de
  `Cabinet::present_static` (chuvisco pelo tubo, teto abaixo do branco — sem
  flash) com um zumbido de RF decaindo que corta no fim.
- **"A estante entra por cima"** (Fase 3): a queda de sinal devolve o nível de
  chuvisco em que parou; os 18 primeiros quadros da estante seguinte usam
  `Cabinet::frame_2d_fade_in`, que compõe o chuvisco e a estante juntos (alfa
  da estante subindo por quadro) em vez de cortar direto pra tela limpa.
- **Cartucho no slot** (Fase 3): durante o jogo, `Cabinet::set_cartridge`
  mostra o rótulo (`texture` do ScreenScraper) ou, sem ele, o nome da ROM —
  nunca fica ausente. Procedural (retângulo + arte/texto), mobília do
  gabinete, não passa pelo tubo. A estante manda o `texture_path` escolhido em
  `Pick::Play { texture, .. }`. `emu-run --cartridge-label img.png` testa sem
  catálogo.
- **Comandos do console** (Fase 3, §3.3): `runner::run_game` ganhou um estado
  `powered`. `Esc` desliga (só ligado): descarrega a SRAM, roda a queda de
  sinal, o jogo trava e a tela vira chuvisco contínuo com o cartucho ainda no
  slot — console desligado é um estado, não um beco. `E` (`UiEvent::Eject`
  novo) só ejeta desligado (limpa o slot, sai pro seletor); ligado, a trava
  resiste com um "clunk" de áudio e mais nada. `Backspace` continua resetando,
  só ligado. Fechar a janela sempre funciona, sem cerimônia.
  `emu-run --shot-off` prevê a tela ociosa headless.
- **Painel lateral** (Fase 4, esqueleto): coluna de widgets reais ao lado do
  tubo durante o jogo (não deformada pelo tubo — plano §2), com logo (`wheel`
  do ScreenScraper) ou o nome da ROM no topo e o tempo de sessão no rodapé. O
  gabinete encolhe pra abrir espaço (`panel_rect`, 25 % da janela, 260–520 px);
  o cartucho continua ancorado no canto do gabinete, não do painel. A estante
  continua sem painel, janela inteira. `Pick::Play` ganhou `wheel`;
  `GameSpec::logo` chega em `Cabinet::set_panel` via `runner::run_game`.
  `emu-run --logo img.png` testa sem catálogo.
- **Comandos no painel** (Fase 4, §3.2 item 3): legenda dos três botões do
  console de verdade — Desligar (`Esc`), Ejetar, Reset — logo abaixo do
  logo/título, tecla lida do `KeyMap` atual (respeita `config.toml`, não é
  fixa). `Cabinet::set_panel` ganhou um terceiro parâmetro `commands`;
  extras do emulador (save state, slot, fast-forward) ficam de fora de
  propósito.
- **Marca no gabinete**: selo "SNES Xperience" impresso no queixo, à esquerda
  do tubo — mobília do gabinete, não da partida, então aparece em todo lugar
  (estante, jogo, console desligado), diferente do cartucho/painel que só
  existem durante o jogo.
- **Cheats com interruptor** (Fase 4, §4.4): `crates/domain/src/cheats.rs`
  embute códigos curados da pasta `cht` do `libretro-database` (CC BY-SA
  4.0 — ver `THIRD-PARTY-NOTICES.md`) para os 19 jogos do catálogo do
  usuário, casados pelo título do cabeçalho SNES, não pelo arquivo/hash.
  `xperience-emulation::Core` ganhou `cheat_reset`/`cheat_set` (FFI fina
  sobre o libretro). Três teclas novas (`.`/`,`/`/`, rebindáveis) navegam a
  lista e viram o interruptor no painel; o estado liga/desliga persiste em
  `<hash>.cheats` no save-dir. Descrições escritas para o app, sem acento
  (a fonte do painel só cobre ASCII).
- **Captura de tela pro caderno** (Fase 4, §3.4): `N` (rebindável) salva o
  quadro cru do core — sem NTSC, sem tubo, pra ficar legível — como PNG em
  `<notes-dir>/<hash>/<epoch>.png` e acrescenta a entrada em
  `<notes-dir>/<hash>.md`, indexado pelo hash da ROM (sobrevive a rename ou
  re-dump). `GameSpec::notes_dir` novo (`xperience`:
  `~/.local/share/snes-xperience/notes/`; `emu-run`: `<save-dir>/notes`,
  `--notes-dir` sobrescreve). O painel ganhou o item 5 do §3.2: miniatura da
  captura mais recente + contador, ausente por completo sem nada capturado.
- **Tela de pausa, leitura** (Fase 4, §3.2/§3.4): `P` abre o caderno do jogo
  em página dupla, sem tubo — não mobília do gabinete, tela própria como o
  seletor. Esquerda: título + contador de capturas (ou um convite a
  capturar, sem nenhuma ainda); direita: a captura mais recente, grande.
  `Cabinet::set_pause_note`/`present_pause`/`capture_pause_bmp` novos.
  Escrita por teclado (precisa de entrada de texto, que a plataforma ainda
  não tem) fica pro próximo incremento.

## [0.1.0] — 2026-09-10

Primeira versão marcada. Cobre as Fases 0 e 1 do
[plano](docs/plano-emulador-moldura.md): o esqueleto das quatro camadas e um
emulador utilitário completo, sem moldura nem seletor.

### Adicionado

- **Workspace de quatro camadas** (`emulation` → `platform` → `domain` → `app`),
  mais o crate folha `ntsc`. CI em Linux/macOS/Windows com SDL3 vendorizado.
- **Emulação** (`xperience-emulation`): carregador de core libretro em runtime
  com FFI de `libretro.h`, laço de frame (vídeo/áudio/entrada), save states
  (`retro_serialize`), SRAM de bateria (`retro_get_memory_data`).
- **Plataforma** (`xperience-platform`): janela SDL3, saída de vídeo por malha
  com distorção de barril (tubo CRT via `render_geometry`, vinheta, sem
  scanline), áudio push com guarda de latência, teclado remapeável (`KeyMap`) e
  até dois gamepads.
- **Domínio** (`xperience-domain`): identificação de ROM (CRC32/MD5/SHA1 sem
  header de copiadora) e cliente ScreenScraper `jeuInfos` que extrai as mídias
  `texture` e `wheel`.
- **NTSC** (`xperience-ntsc`): `snes_ntsc` 0.2.2 do blargg vendorizado
  (LGPL-2.1+), wrapper seguro e preset `Rf` (visual de antena).
- **`emu-run`**: roda uma ROM com o visual fixo NTSC RF + tubo CRT; 10 slots de
  save state indexados pelo SHA1 da ROM; persistência de SRAM; run-ahead
  (`--runahead`, padrão 1); fast-forward, frame-step, dois jogadores;
  screenshot (F12); `config.toml` para binds de teclado e padrões.
- **`scrape-test`**: verifica a cobertura de `texture`/`wheel` do ScreenScraper
  para uma amostra de ROMs (prova 2 da Fase 0).
- Exemplos `probe` e `state_check` no crate de emulação.

### Notas

- O core do snes9x, ROMs e BIOS **não** acompanham o repositório
  (ver [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md) e
  [`docs/fase-0.md`](docs/fase-0.md)).
- O "modo CRT" completo do plano §4.7 (scanline, máscara de fósforo) fica para
  a Fase 3; os três modos de escala originais foram substituídos por essa
  visualização única a pedido.

[Não lançado]: https://github.com/ticianocorral/snes-xperience/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/ticianocorral/snes-xperience/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/ticianocorral/snes-xperience/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/ticianocorral/snes-xperience/compare/v0.5.0...v0.6.0
[0.3.1]: https://github.com/ticianocorral/snes-xperience/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/ticianocorral/snes-xperience/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/ticianocorral/snes-xperience/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ticianocorral/snes-xperience/releases/tag/v0.1.0
