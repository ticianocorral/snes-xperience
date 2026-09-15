# Fase 4 — O painel

Plano §6: logo, cheats com interruptor, tela de pausa, anotações com captura
de tela, e cinco jogos com senha e dica escritas à mão.

Progresso:

- [x] **Painel lateral (esqueleto)** — coluna de widgets reais ao lado do tubo
      durante o jogo, com logo (ou nome) no topo e o tempo de sessão embaixo
- [x] **Comandos** (item do §3.2) — botões clicáveis Power/Ejetar/Reset,
      não mais só legenda (ver Revisão abaixo)
- [x] **Cheats com interruptor** (`cht` do libretro-database, §4.4)
- [x] **Captura de tela pro caderno** (§3.4) — falta só a leitura/escrita
      longa, que é item da tela de pausa, abaixo
- [x] **Tela de pausa (leitura)** — página dupla, sem tubo, mostra o caderno
      do jogo; a escrita por teclado fica pro próximo incremento
- [ ] Tela de pausa (escrita) — subsistema de entrada de texto, ainda não
      existe no app
- [ ] Senhas e dicas — tabela manual, cinco jogos pra começar (§4.5/§4.6)

## Desvio deliberado do §3.2 (histórico, revertido — ver Revisão abaixo)

O plano listava "cartucho encaixado no console" como item 2 da coluna do
painel. Na Fase 3 eu já tinha posto o cartucho no queixo do próprio gabinete
(mobília da TV, não do painel) — decisão tomada antes do painel existir, e que
já passou pelo teste de duas horas. Mantive assim em vez de mover: refazer
teria custo alto pra ganho estético pequeno, e o cartucho já cumpre o papel de
"nunca é o primeiro item a ser cortado". O painel cobre os itens 1 e 6
(logo, tempo de sessão) e vai cobrir 3-5 (comandos, cheats, anotações) nos
próximos incrementos.

*Revertido na revisão de 2026-09-13, abaixo: o cartucho não aparece mais em
lugar nenhum, nem no queixo nem no painel — pedido do usuário, não uma volta
ao texto original do plano.*

## Painel lateral (`xperience-platform::cabinet`)

Uma coluna de **widgets normais**, não deformada pelo tubo (plano §2: "4.
painel lateral, em widgets normais") — desenhada direto na janela, como o
cartucho. Só existe durante o jogo (`present_frame` / `present_static`); a
estante continua ocupando a janela inteira, sem painel.

- **Geometria:** `panel_rect(out_w, out_h)` — coluna à direita, 25 % da
  largura da janela, limitada entre 260 e 520 px. O gabinete (tubo + friso)
  encolhe pra caber: `cab_w = out_w - panel.width()`, e todo o resto do layout
  do jogo (`screen_area`, malha do tubo, `cartridge_slot_rect`) passa a
  trabalhar dentro de `cab_w`, não da janela inteira — o cartucho continua
  ancorado no canto do **gabinete**, não desliza pra debaixo do painel.
- **Conteúdo (por ora):** logo (`Cabinet::set_panel`, mídia `wheel` do
  ScreenScraper) ou, sem ele, o nome da ROM — item 1 do §3.2. Tempo de sessão
  no rodapé (`MM:SS`, ou `H:MM:SS` passada 1 h) — item 6. Contagem por relógio
  de parede desde que `run_game` começou (inclui tempo desligado — mais simples
  que descontar pausas, e "tempo de sessão" não é claramente só tempo jogado).
- **Cores:** `PANEL_BG` = `(16, 15, 14)`, texto `PANEL_TEXT` /
  `PANEL_DIM`.

Reaproveita as funções livres já criadas pro cartucho na Fase 3
(`draw_image_absolute`, `draw_text_absolute`) — essa última ganhou um
`TextStyle { scale, color }` pra não estourar o limite de argumentos do
clippy, e um `draw_text_wrapped_absolute` novo pro título que não coube numa
linha. O painel também aparece no chuvisco de "console desligado"
(`present_static`) — o relógio e o logo continuam visíveis nesse estado.

`Pick::Play` ganhou `wheel: Option<PathBuf>` (a estante já tinha o
`wheel_path` da ficha em memória); `GameSpec::logo` carrega até
`runner::run_game`, que decodifica (≤640 px, alfa preservado) e chama
`Cabinet::set_panel`. `emu-run --logo img.png` testa sem catálogo.

## Comandos (item 3 do §3.2)

Legenda simples dos botões do próprio console — não os extras do emulador
(save state, slot, fast-forward): **Desligar** (`Esc`), **Ejetar** (`E`),
**Reset** (`Backspace`). São os três que existem numa SNES de verdade (liga,
ejeta, reseta); os outros ficam de fora da legenda de propósito, do jeito que
`docs/fase-3.md` já separava "comandos do console" de "extras do emulador".

`runner::run_game` monta a lista lendo `cfg.keymap` (a tecla real, não a
padrão — respeita o `config.toml` do usuário) e passa pra
`Cabinet::set_panel`, que agora recebe um terceiro parâmetro
`commands: &[(String, String)]`. `Esc` é fixo no código (não passa por
`KeyMap::describe()`, que só lista os binds configuráveis — ver o comentário
em `UiEvent::token`). `draw_panel` desenha o rótulo "comandos" (apagado) e
cada linha com o nome à esquerda, a tecla à direita, logo abaixo do
logo/título — cresce ou encolhe com a altura do título (uma ou duas linhas).

## Marca no gabinete

Fora da lista do §3.2 (que é sobre a tela do *jogo*): um selo "SNES
Xperience" discreto, impresso no queixo à esquerda do tubo — o pingente que
uma TV de verdade tem no gabinete. Por ser parte do móvel, não da partida,
aparece em todo lugar onde o queixo existe: estante, jogo, chuvisco de
console desligado. `draw_brand`, chamada logo depois da malha do friso em
cada um dos sete pontos que a desenham (`present_frame`, `capture_bmp`,
`capture_2d`, `present_static`, `capture_static_bmp`, `frame_2d_fade_in`,
`composite_screen`) — mesmo texto cor `BRAND_TEXT`, um tom mais claro que o
plástico do gabinete, como relevo gravado, não uma etiqueta acesa.

## Cheats com interruptor (§4.4)

Sem servidor de cheats nenhum: `crates/domain/src/cheats.rs` embute uma
tabela estática, `internal_name` (o título de 21 bytes do cabeçalho SNES,
não o nome do arquivo nem o hash — sobrevive a um re-dump ou renomeação)
apontando pra até três `CheatDef { desc, code }`. Os 19 jogos que já estão
no catálogo do usuário foram curados a partir da pasta `cht` do
`libretro-database` — ver `THIRD-PARTY-NOTICES.md` pela licença correta
(CC BY-SA 4.0; o plano tinha MIT de memória, já corrigido lá e aqui) e a
nota de atribuição. Só o código de cada cheat (endereço/valor, fato bruto)
veio de lá; toda descrição foi escrita pra este app — e sem acento: a fonte
bitmap do painel (`font8x8::legacy`) só cobre ASCII, então "não"/"é" viravam
"?" na tela até eu notar no primeiro screenshot e trocar por "nao"/"e".

`Core` (emulação) ganhou `cheat_reset`/`cheat_set`, fininhos sobre
`retro_cheat_reset`/`retro_cheat_set` do libretro — o código passa direto
pro core sem reformatar, então tanto o hex bruto (`7E034704`) quanto o Game
Genie/Pro Action Replay (`1B29-4DD9`) e os combinados com `+` (vários
endereços por cheat, comuns em jogos de luta) funcionam do jeito que a base
já os guarda.

`run_game` calcula a lista pro cabeçalho da ROM em mãos, carrega o estado
salvo (`<hash>.cheats` no save-dir, um `0`/`1` por linha — mesmo padrão de
`.srm`/`.state0..9`) e aplica tudo no core antes do primeiro frame. Três
teclas novas navegam e viram a chave (`UiEvent::CheatNext/Prev/Toggle`,
padrão `.`/`,`/`/`, rebindáveis como as outras): mover o cursor só redesenha
o painel; apertar o interruptor chama `core.cheat_set` de novo pro índice
tocado (o libretro não exige um `cheat_reset` global a cada mudança) e
grava o novo estado em disco. Só funciona com o console ligado, mesmo
padrão dos outros extras.

No painel, a lista fica entre comandos e o relógio de sessão: cabeçalho
"cheats" apagado, cada linha `[x]`/`[ ]` + descrição, a selecionada com
cursor `>` e brilho total, as outras apagadas — só ASCII, então o
interruptor é textual, não um ícone.

## Captura de tela pro caderno (§3.4)

"O jogador chega na tela de senha, aperta um botão, a imagem entra no
caderno daquele jogo" — a parte que cobre todo jogo, inclusive os que
nunca vão ganhar anotação escrita à mão. `N` (rebindável, `UiEvent::NoteCapture`,
só com o console ligado) salva o quadro **cru** do core — antes do NTSC e
antes do tubo — como PNG em `<notes-dir>/<hash>/<epoch>.png` e acrescenta
`![captura](hash/epoch.png)` em `<notes-dir>/<hash>.md`. Cru de propósito:
o visual do tubo é bonito, mas borra a tela de senha que você queria
guardar legível; "como se fazia no papel" pede nitidez, não atmosfera.

Indexado pelo **hash da ROM** (`rom_hash`, o mesmo SHA1 dos saves/estados,
plano §3.4) — nome de arquivo trocado ou re-dump não orfanam o caderno.
Markdown solto, legível fora do app, com as capturas numa pasta ao lado do
`.md` — dá pra abrir num editor de texto qualquer sem o SNES Xperience por
perto.

`xperience_emulation::Core` não precisou de nada novo — `runner::frame_to_rgb8`
decodifica RGB565/RGB1555/XRGB8888 na mão (o `Frame` bruto do core, os
mesmos formatos que a NTSC e o `Cabinet` já entendiam, só que sem passar
por nenhum dos dois). `GameSpec::notes_dir` é novo, ao lado de `save_dir`/
`system_dir` (`xperience`: `~/.local/share/snes-xperience/notes/`;
`emu-run`: `<save-dir>/notes` por padrão, `--notes-dir` sobrescreve).

No painel, item 5 (§3.2) fica entre cheats e o relógio: cabeçalho "notas"
apagado, a miniatura da captura mais recente (a mesma arte do cartucho/logo,
letterboxed) e "N captura(s)" — **some por completo** sem nenhuma capturada
ainda, nada de "0 anotações" ocupando espaço à toa.

## Tela de pausa — leitura (§3.2/§3.4)

Fatiada com o usuário: leitura primeiro, escrita (que precisa de um
subsistema de entrada de texto que o app ainda não tem — hoje só existem
teclas discretas, não digitação livre) fica pra um incremento à parte.

`P` (a mesma tecla de sempre) agora abre o caderno em vez de só congelar o
quadro do jogo: página dupla, **sem tubo** — cartucho, painel e o próprio
gabinete somem, é uma tela dedicada (a mesma lógica do seletor: sua própria
apresentação, não mobília por cima do jogo). Esquerda mostra o título e
quantas capturas o caderno tem ("Sem anotações ainda. Aperte N pra capturar
a tela." se for zero); direita mostra a captura mais recente, grande,
letterboxed. `P` de novo volta pro jogo congelado, do jeito que já era.

`Cabinet::set_pause_note` carrega o conteúdo uma vez, ao entrar na pausa —
não a cada quadro, os frames enquanto pausado só redesenham o que já foi
carregado (`present_pause`/`capture_pause_bmp`, mesmo par presente/capture
headless dos outros estados). Reaproveita `decode_art` (o mesmo decodificador
do cartucho/logo/miniatura do painel) num tamanho maior, e as funções de
texto absolutas de sempre — nada de biblioteca de UI nova.

## Menu de configurações

*Revisão (2026-09-14): a linha/tela "ScreenScraper" descrita abaixo foi
removida por completo — o ScreenScraper não existe mais no app (ver
`docs/fase-2.md`). No lugar dela, uma linha "Núcleo" baixa/atualiza o
snes9x pelo buildbot do libretro. O resto desta seção (estrutura de
`Mode`/`MAIN_ROWS`, `poll_menu`/`MenuMode`, o padrão de salvar na hora)
continua valendo — só o conteúdo daquela linha específica mudou.*

Fora da numeração do plano — pedido direto do usuário antes de seguir a
Fase 4, porque sem ele ligar o ScreenScraper e popular o catálogo com
capas/fichas dependia de exportar `SS_DEVID`/`SS_DEVPASSWORD` na mão toda
vez. `O` na estante abre `xperience_app::settings::run`, três listas
achatadas (sem menu dentro de menu): **Controles** (as 27 ações
rebindáveis, `describe()` do `KeyMap`), **ScreenScraper** (ativado, Dev
ID, Dev Password), e a raiz com Run-ahead/Tela cheia/atalhos pros dois
submenus. Cada mudança salva em `config.toml` na hora — não existe "aplicar
depois", então sair no meio não perde nada nem deixa nada pela metade.

Isso puxou uma peça que faltava na plataforma: `Platform::poll_menu` não
tinha como capturar uma tecla crua (pra rebind) nem digitar texto de
verdade (só letras minúsculas + espaço, pro campo de busca da estante).
Virou `poll_menu(mode: MenuMode)` com três modos — `Nav` (o de sempre, com
`F`/`O` como atalhos), `TextEntry` (edição de campo: tudo vira caractere,
sem atalho nenhum — senão digitar "f" ligava/desligava tela cheia no meio
da senha) e `CaptureKey` (rebind: a próxima tecla vem crua em
`captured_key`, Esc cancela em vez de virar o novo bind). `char_for_key`
lê Shift do próprio evento e devolve maiúscula/símbolo — os nomes de tecla
do SDL são a glifo *sem* Shift, então sem isso não dava pra digitar senha
com letra maiúscula ou `@`/`.`/`-`.

`config.toml` ganhou `[screenscraper]` (`enabled`, `dev_id`,
`dev_password`) e `Config` ganhou `to_toml`/`save`/`resolve_screenscraper`
— esse último prioriza o que está salvo no arquivo, caindo pra
`$SS_DEVID`/`$SS_DEVPASSWORD` só se a seção estiver desligada ou vazia
(o fluxo antigo continua funcionando, sem quebrar nada de quem já usava
variável de ambiente). Um arquivo `config.toml` de antes desta mudança
continua carregando normal (a seção é opcional na leitura).

## Verificado

- `emu-run --shot --cartridge-label` (sem `--logo`): painel com o título
  quebrado em duas linhas, comandos abaixo, tempo de sessão, cartucho no
  canto do gabinete encolhido
- `emu-run --shot --cartridge-label --logo`: logo no topo, comandos logo
  abaixo dele
- `emu-run --shot --shot-off --logo`: painel + comandos + cartucho continuam
  visíveis durante o chuvisco de console desligado
- Selo "SNES Xperience" no queixo, à esquerda: visível no jogo, no chuvisco
  de console desligado e na estante (não é sobre a tela do jogo, é sobre o
  gabinete)
- `selector --frames --shot`: estante segue funcionando — o painel do jogo
  continua sem espaço reservado lá, como esperado
- Aladdin (real, do catálogo do usuário) com um `.cheats` pré-gravado
  ligando "Invencibilidade total": o core aceita o cheat, roda 60 quadros
  sem travar, o painel mostra `[x]` no item certo — sem como confirmar o
  efeito num único frame de boot headless, mas a aplicação em si (FFI +
  persistência + UI) está provada de ponta a ponta
- fmt / clippy / 15 suítes, +4 testes novos (`cheats::tests`, round-trip do
  estado salvo em `runner::tests`) — verdes, na entrega de cheats
- `emu-run --shot --shot-frame 60 --debug-note-capture` no Aladdin real: a
  tela do Capcom/Disney vira um PNG 256×224 nítido em `<notes-dir>/<hash>/`,
  a entrada `![captura](...)` aparece no `.md`, e o painel mostra a mesma
  imagem em miniatura + "1 captura" — testado de novo com uma segunda
  captura no mesmo segundo pra confirmar que os nomes de arquivo não colidem
- fmt / clippy / 15 suítes, +2 testes novos (`frame_to_rgb8` decodifica o
  bit layout RGB565 certo; captura grava markdown + PNG e não sobrescreve
  numa colisão de timestamp) — verdes, na entrega da captura
- `emu-run --shot --debug-shot-pause` sem nenhuma captura ainda: página dupla,
  "Sem anotações ainda. Aperte N pra capturar a tela." à esquerda, "pagina em
  branco" à direita
- Mesmo teste depois de um `--debug-note-capture` real (Aladdin, frame 60):
  "1 captura salva." à esquerda, a tela do Capcom/Disney grande e legível à
  direita — sem tubo, sem NTSC, só a imagem
- fmt / clippy / 15 suítes — verdes, na entrega da tela de pausa
- `xperience --debug-settings main|controls|screenscraper --shot out.bmp`:
  as três telas renderizam pelo tubo, cabeçalho + linhas + cursor `>` +
  rodapé de dicas certos; `controles` lista as 27 ações e corta na 21ª
  (a rolagem existe, não dava pra fotografar rolado sem estender o preview);
  `screenscraper` mostra "(vazio)" nos dois campos e o texto explicativo
  quebrado em duas linhas
- Lançamento real (`xperience` de verdade, 3 s, sem `--debug-settings`):
  sobe, carrega o `config.toml` existente de antes desta mudança sem erro
  de parse (a seção `[screenscraper]` é opcional), fecha limpo
- fmt / clippy / 22 testes (+6 novos: `char_for_key`/modo de `poll_menu`
  via `config::tests::screenscraper_*`/`to_toml_round_trips`) — verdes

## A seguir

Escrita na pausa: precisa de um subsistema de entrada de texto que a
plataforma ainda não tem (`Platform::poll` só traduz teclas discretas via
`KeyMap` — nada de captura de texto livre/Unicode). Depois disso, o texto
digitado entra no `.md` ao lado das capturas. Por fim, senhas/dicas manuais
pra cinco a dez jogos (§4.5/§4.6) — conteúdo que preciso escrever com o
usuário, não vou inventar senha de jogo.

## Revisão (2026-09-13): tela inicial, cartucho fora de cena, botões clicáveis

Três pedidos do usuário, fora da numeração original do plano:

1. **Tela inicial (TV off + "Inserir cartucho").** `xperience` não pula mais
   direto pra estante. `crates/app/src/idle.rs` (novo módulo, mesmo padrão de
   `shelf.rs`/`settings.rs`) desenha `Cabinet::present_static` sem nenhum
   painel de jogo montado (`Cabinet::new` já nasce com `panel: None`) — nesse
   estado, `draw_panel` (cabinet.rs) desenha só um botão "Inserir cartucho"
   no lugar do bloco de logo/título (item 1 do §3.2). Essa tela é o **estado
   raiz** do app agora: aparece na abertura, depois de Ejetar
   (`GameExit::Ejected`, renomeado de `GameExit::ToShelf`) e no Esc da
   estante (`shelf::Pick::Back`, novo — distinto de `Pick::Quit`, que hoje só
   dispara ao fechar a janela). `xperience::main` ganhou um laço externo
   (`'app`) em volta do laço estante↔jogo que já existia, alternando com
   `idle::run`.
2. **Cartucho no console removido.** O desvio da Fase 3 (cartucho no queixo
   do gabinete) foi completamente desfeito — não só escondido, apagado:
   `CartridgeSlot`, `Cabinet::set_cartridge`/`clear_cartridge`,
   `cartridge_slot_rect`, `draw_cartridge_slot` e as cores
   `CART_SHELL`/`CART_RIM` saíram de `cabinet.rs`; `GameSpec.cartridge_label`,
   `Pick::Play.texture` e o `--cartridge-label` do `emu-run` saíram junto — a
   mídia `texture` do ScreenScraper continua sendo raspada e cacheada no
   catálogo (`xperience_domain`), só ficou sem consumidor visual.
3. **Comandos viram botões clicáveis.** A legenda de texto "Desligar Esc /
   Ejetar E / Reset Backspace" virou três caixas clicáveis (`draw_button`,
   cabinet.rs), acesas ou apagadas conforme fazem algo *agora* —
   `PanelInfo.powered` (atualizado por `Cabinet::set_powered`, chamado no
   `UiEvent::Quit` de `run_game`): Power aceso enquanto ligado, Ejetar aceso
   só desligado (senão só dá o clunk), Reset aceso só ligado. Clicar dispara
   exatamente o mesmo `UiEvent` que a tecla já disparava (`Quit`/`Eject`/
   `Reset`) — a máquina de estado de `run_game` não sabe que existe mouse.

   Isso pediu infraestrutura nova em toda a pilha, porque **o app não tinha
   nenhum evento de mouse antes**: `Platform::poll`/`poll_menu` (lib.rs)
   ganharam captura de `Event::MouseButtonDown` (botão esquerdo),
   `UiEvent::Click(i32, i32)` e `MenuInput.click` carregam a posição em
   coordenadas de *janela*; `Cabinet::window_to_output` converte pra
   coordenadas de canvas/output (necessário em telas HiDPI, onde as duas
   divergem); `Cabinet::hit_panel_button` varre `panel_buttons` — os rects
   dos botões desenhados no quadro anterior (`present_frame`/
   `present_static` guardam o retorno de `draw_panel`, que agora é
   `Vec<(PanelButton, Rect)>` em vez de `()`). `PanelButton` (`Insert`/
   `Power`/`Eject`/`Reset`) é o mesmo tipo pra tela inicial e pra tela de
   jogo — um hit-test só pros dois lugares.

Verificado com um exemplo headless descartável (`Cabinet` puro, sem core/ROM
— `cargo run -p xperience-platform --example panel_preview`, apagado depois
de conferir): tela inicial com o botão no lugar do logo, cartucho ausente em
todos os estados, e os três botões acendendo/apagando junto com
`set_powered(true/false)`, exatamente como descrito acima. `cargo build`/
`clippy`/`test --workspace` limpos nas 24 suítes existentes; não há teste
automatizado novo cobrindo o clique em si (pediria simular eventos SDL de
mouse, que os testes atuais de `cabinet` não fazem para teclado/gamepad
também) — a sequência ao vivo completa (abrir o app → Inserir cartucho →
escolher jogo → clicar Ejetar/Power/Reset no painel → Esc na estante) ainda
não foi jogada de verdade, vale conferir na próxima sessão.

## Revisão (2026-09-14): nenhum comando por teclado — só mouse/gamepad

Pedido do usuário: "remover navegação via teclado e ser tudo via mouse /
controle", sem exceção — inclusive os comandos do console durante a
partida (antes só Power/Ejetar/Reset eram clicáveis; Pausar, save/load
state, slot, turbo, screenshot, nota e cheats ainda dependiam de tecla).
Mudança grande, em várias camadas:

- **`xperience-platform::input`**: `KeyMap` perdeu o campo `ui`/`bind_ui`/
  `ui_for` inteiro — só sobrou `pad` (as 12 teclas de gameplay, D-pad/
  botões, que continuam por padrão porque nem todo mundo tem gamepad
  plugado pra *jogar*). `UiEvent` perdeu `ToggleFullscreen`/`CheatNext`/
  `CheatPrev`/`FastForward` (o held-key) e ganhou `CheatToggle(usize)`
  (clique endereça a linha direto, sem cursor) e `ToggleFastForward`
  (clique liga/desliga, não é mais segurar tecla). `UiEvent::token()`/
  `BINDABLE` sumiram — nada disso é mais rebindável.
- **`Platform::poll_menu`**: o `KeyDown` de `MenuMode::Nav` inteiro saiu —
  setas/Enter/Esc/PageUp/Home/`F`/`O`/digitação não navegam mais nada.
  `MenuMode::TextEntry` (nunca chegou a ser usado por ninguém) foi
  removido junto. Ganhou `Event::MouseWheel` → empurra `MenuNav::Up`/
  `Down`, pra rolar lista sem gamepad. `MenuMode::CaptureKey` é a
  **única** exceção de propósito — seu trabalho é literalmente gravar uma
  tecla de teclado pra rebind de gameplay, então continua lendo `KeyDown`.
- **`Platform::poll`** (dentro do jogo): o `Escape` fixo virando `Quit` e
  o `keymap.ui_for(k)` inteiro saíram — só resta `keymap.pad_for(k)`
  (D-pad/botões). Todo comando de console agora chega como `UiEvent::
  Click`, resolvido pelo chamador via `Cabinet::hit_panel_button`/
  `hit_pause_button`.
- **Painel lateral, em jogo**: a lista de comandos deixou de ser fixa
  (Power/Ejetar/Reset) — `runner::command_rows` monta `Vec<(PanelButton,
  String)>` **a cada quadro** (`Cabinet::set_commands`, novo — `set_panel`
  só define uma vez no início) porque os rótulos agora carregam estado
  vivo ("Slot: 3", "Turbo: ligado"). Ganhou: Pausar, Screenshot, Nota,
  Salvar/Carregar (slot atual), Slot (cicla), Turbo. As legendas "[Esc]"/
  "[E]" etc. saíram — não tem mais tecla pra mostrar.
- **Cheats**: cada linha virou seu próprio botão clicável
  (`PanelButton::CheatRow(usize)`) — clicar liga/desliga direto, sem
  cursor pra mover primeiro com `,`/`.` antes de apertar `/`.
  `Cabinet::set_cheats` perdeu o parâmetro `selected`.
- **Caderno de pausa** (`draw_pause_book`): deixou de ser só leitura —
  ganhou dois botões próprios, "Continuar" (retoma) e "Avancar quadro"
  (um frame só, pausado) — únicos ali porque o caderno troca a janela
  inteira, sem painel lateral (por isso `Cabinet` ganhou uma segunda
  lista de hit-test, `pause_buttons`/`hit_pause_button`, separada de
  `panel_buttons`).
- **Configurações** (`settings.rs`): ganhou clique de verdade — não tinha
  nenhum antes (só `MenuNav` por gamepad). `row_at`/`back_band_hit`
  convertem um clique em qual linha da lista foi tocada; a lista de
  Controles idem, mais uma faixa clicável embaixo ("Voltar", já que ela
  rola e não cabe como última linha fixa como a de Configurações cabe).
  Ajustar Run-ahead por clique cicla (incrementa e volta a 0); Tela cheia
  por clique alterna — o D-pad esquerda/direita do gamepad continua
  funcionando do jeito fino de antes, em paralelo.
- **Tela inicial**: ganhou o botão "Configuracoes" ao lado de "Inserir
  cartucho" (`PanelButton::Settings`, `IdleExit::OpenSettings`) — antes
  configurações só abria pela tecla oculta `O` na estante, sem nenhum
  caminho por mouse/gamepad a partir da tela inicial.
- **Estante**: a busca por digitação saiu inteira (não sobrou pra que
  serviria sem teclado) — o rótulo virou só "N games". Ganhou dois botões
  no rodapé da ficha, "Voltar" e "Configuracoes" (`Pick::Back`/
  `Pick::Settings` por clique, além do gamepad Back que já existia).
- **`config.rs`**: `[keyboard]` de um `xperience.cfg` antigo com uma ação
  removida (`eject`, `pause`, ...) não trava mais o app — vira
  `log::warn!` e segue (era `bail!`); só teclas de gameplay continuam
  bindáveis.

Build/clippy/test/fmt limpos no workspace inteiro. `emu-run --help` e a
tabela de teclas do `docs/fase-0.md` foram atualizados pra não anunciar
atalho nenhum que não existe mais.

## Revisão (2026-09-14, continuação): cartucho no painel, cheats no caderno, paginação e anotação por texto

Pedido do usuário, depois de testar a revisão acima: mover a lista de
cheats pra "outra janela" (o painel precisa do espaço pra logo + arte de
cartucho), e no caderno de pausa, paginação pros prints com botões, mais um
campo pra escrever texto (com limite de caracteres) do outro lado.

- **Arte de cartucho no painel** (`assets/cartridge/<rom>.*`, mesma
  convenção de nome de `assets/logo/`/`assets/cover/`): `Pick::Play`/
  `GameSpec` ganharam um segundo campo `cartridge: Option<PathBuf>`,
  independente do `wheel`/logo — nenhum dos dois precisa do outro.
  `Cabinet::set_panel` ganhou um parâmetro `cartridge`, `PanelInfo` um
  `has_cartridge`, e `draw_panel` desenha essa arte logo abaixo do
  logo/título, antes da seção "comandos".
- **Cheats saíram do painel, foram pro caderno de pausa**: no painel
  (visível o tempo todo, jogo rodando) viraram só texto informativo — só
  os que estão *ligados*, sem clique, sob o título "cheats ativos" (some
  inteiro se nenhum estiver ligado). O interruptor de verdade (clicar liga/
  desliga, `PanelButton::CheatRow`) mudou pro caderno de pausa, que tem
  espaço de sobra e já para o jogo de consumir input — sem essa disputa
  por espaço com a lista de comandos, que cresceu bastante nesta fase.
- **Painel ficou apertado**: com cartucho + 10 comandos, um título comprido
  sem logo (2 linhas em escala 2) já não cabe tudo. `draw_panel` calcula
  `limit` (o topo da faixa do relógio de sessão) e cada seção (comandos,
  cheats informativos, notas) para de desenhar assim que a *próxima* linha
  inteira não cabe mais — corte limpo, sem sobrepor o relógio, mas **sem
  rolagem ainda** (um comando pode simplesmente não aparecer se não houver
  espaço — registro do que falta, não aceito em silêncio).
- **Paginação dos prints** (`Cabinet::set_pause_page`, novo, substituindo o
  antigo terceiro parâmetro de `set_pause_note`): a página direita do
  caderno mostra qualquer captura, não só a mais recente — contador "N/
  total" e botões "< anterior"/"proxima >" (dim quando não há pra onde ir).
  `runner::list_note_images` lista todas as capturas em ordem cronológica;
  `note_page` (estado do laço) indexa nela, recalculado ao entrar na pausa
  (começa na última) e a cada clique.
- **Anotação por texto, com limite de caracteres** (`NOTE_CHAR_LIMIT` =
  240): a página esquerda ganhou um botão "Escrever anotacao" que troca o
  status por um editor ao vivo — contador "N/240", `Enter` ou o botão
  "Salvar" grava um novo parágrafo no mesmo `.md` do notebook
  (`runner::append_note_text`, intercalado cronologicamente com as
  capturas de imagem), `Esc` ou "Cancelar" descarta. **Única exceção
  deliberada ao "sem teclado"** desta revisão inteira — escrever texto
  exige teclado por definição. Implementado com a API de composição de
  texto de verdade da SDL (`VideoSubsystem::text_input()`/
  `Event::TextInput`), não um mapeamento manual de tecla pra caractere (o
  `char_for_key`/`shift_char` removidos na revisão anterior por estarem
  mortos) — trata layout de teclado/IME direito, coisa que o hack antigo
  não fazia. `Platform::poll_text_entry` (novo) só é chamado enquanto o
  editor está aberto; o `poll()` de gameplay normal fica intocado o resto
  do tempo. Continuar/Avancar quadro somem enquanto o editor está aberto —
  evita perder o rascunho clicando neles sem querer.

Verificado com exemplos headless descartáveis (arte de cartucho sintética,
capturas de teste geradas com `--debug-note-capture`, e um preview direto
do modo de escrita via `Cabinet::set_pause_draft`) — apagados depois de
conferir, junto com as anotações de teste que geraram (não eram do
usuário). Build/clippy/test/fmt limpos.

## Revisão (2026-09-14, continuação 2): dois relatos reais + Screenshot removido, notas viram slots fixos

Dois problemas reais encontrados testando a revisão acima (analisando o log
da própria sessão do usuário, não só código): **"Nota" clicado 7 vezes**
porque não dava feedback nenhum na tela (funcionava, silenciosamente), e
**nenhum "paused" no log** — o usuário nunca achou o caminho pros cheats,
porque o texto informativo no painel não apontava pra lá.

- **Feedback "(feito!)"**: `runner::flash` (`HashMap<PanelButton, Instant>`,
  `PanelButton` ganhou `Hash`) marca quando um botão silencioso (Nota,
  Salvar, Carregar) disparou; `command_rows` troca o rótulo por
  `"<label> (feito!)"` por `FLASH_DURATION` (900 ms). Sem estado por quadro
  novo além do já existente — só mais uma entrada no mapa por clique.
- **Dica nos cheats**: o texto informativo no painel virou "cheats ativos
  (pausar pra editar)" — aponta pro caderno de pausa, onde o interruptor
  mora de verdade.

Pedido seguinte do usuário, depois de testar: tirar o "Screenshot" (o
usuário achou que "ficava tirando constantemente" — na real eram os
cliques repetidos por falta de feedback, já corrigido acima — mas decidiu
que a funcionalidade em si não servia pra nada mesmo) e reestruturar as
notas pra um modelo de **slots fixos**, não mais uma lista cronológica
aberta:

- **`PanelButton::Screenshot`/`UiEvent::Screenshot` removidos por
  completo** — o botão, o evento, `shot_request` e as duas chamadas de
  `Cabinet::capture_bmp`/`capture_pause_bmp` que ele disparava ao vivo.
  `--shot`/`--debug-shot-pause` (headless, dev) continuam intactos — são
  outro mecanismo, não relacionado ao botão.
- **15 slots fixos de nota** (`NOTE_SLOTS`), não mais um arquivo por
  captura com timestamp: `PanelButton::NoteSlot`/`UiEvent::NoteSlotNext`
  cicla qual slot (1..=15) o botão "Nota" grava — mesmo modelo de
  "escolher slot, sobrescreve o que tinha" que save state já usava.
  `note_slot` (estado do laço) é compartilhado entre o painel (que slot
  "Nota" grava) e o caderno de pausa (Prev/Next navegam o mesmo valor,
  travados em 1/15 nas pontas em vez de dar a volta).
- **Pasta por nome do jogo, não por hash**: `notes/<título>/01.png` ..
  `15.png` (`runner::note_dir`/`note_slot_path`, título sanitizado contra
  caracteres inválidos de caminho) — pedido explícito do usuário, prioriza
  quem for abrir a pasta a mão sobre resistência a rename (motivo original
  do hash). Textos livres foram para `notes/<título>/notas.txt`, separado
  das imagens — `runner::append_note_text` só sabe desse arquivo agora,
  nada mais de markdown misturando imagem e texto.
- **`Cabinet::set_pause_note`/`PauseNote`** ganharam um campo `filled`
  (quantos dos 15 slots têm imagem) separado de `captures` (sempre 15
  agora, usado só pro limite da paginação) — o texto "N capturas salvas"
  virou "N de 15 slots usados", e "Sem anotacoes ainda" precisou trocar de
  gatilho (`captures == 0` nunca mais acontece) para `filled == 0`.

Verificado ao vivo com o ROM/core reais do usuário: painel sem Screenshot,
"Nota (slot N)"/"Nota slot: N" cabendo sem sobrepor o relógio de sessão,
captura gravando em `notes/Street Fighter II Turbo/01.png`, e o caderno de
pausa mostrando "1 de 15 slots usados" + paginação "1/15" corretamente
desabilitada/habilitada nas pontas. Build/clippy/test/fmt limpos — os
testes antigos de `append_note_image` foram substituídos por três novos
(`note_slots_save_independently_and_count_correctly`,
`note_text_appends_to_its_own_txt_file`,
`note_dir_sanitizes_path_hostile_titles`).

## Revisão (2026-09-14, continuação 3): fixar/nomear slots, capas e cartucho maiores, chaves gangorra, tela inicial = "cartucho ejetado"

Sequência de pedidos pontuais do usuário, cada um testado ao vivo (com o
ROM/core/arte reais dele, via `computer-use`) antes do próximo:

- **Fixar e nomear slots de nota** (`NotesMeta`/`SlotMeta`, novo sidecar
  `notes/<título>/slots.json` via `serde_json`): `PanelButton::PauseNotePin`/
  `PauseNoteName` e os `UiEvent` correspondentes. Fixar não bloqueia a
  captura — `UiEvent::NoteCapture` busca o próximo slot livre a partir do
  atual, dando a volta (`(0..NOTE_SLOTS).map(|i| ((note_slot-1+i) %
  NOTE_SLOTS)+1).find(|&s| !pinned)`); só quando **todos** os 15 estão
  fixados é que não há pra onde redirecionar — em vez de um flash "sem
  espaco" por 900ms (que somem e o problema volta a acontecer no próximo
  clique), o rótulo do botão "Nota" fica permanentemente
  "Nota: sem espaco (15 fixados)" enquanto essa condição for verdadeira
  (`command_rows` ganhou um parâmetro `all_slots_pinned`, computado ao
  vivo a cada quadro em vez de guardado como um `Instant` — mais simples e
  sempre correto, sem janela de tempo pra acertar). Nomear reaproveita o
  editor de texto livre da anotação (`NoteEdit::SlotName`, limite de 40
  caracteres) — mesmo `poll_text_entry`, heading diferente.
- **Capas da estante em paisagem** (`shelf.rs`, `TILE_W`/`TILE_H`
  invertidos de 150×200 pra 200×150): a arte que o usuário realmente usa é
  capa de frente horizontal, não retrato estilo lombada — `image_fit`
  (mantém proporção, sem distorcer) estava encaixando a imagem larga numa
  moldura alta, sobrando faixa vazia em cima/embaixo. Depois, a pedido,
  aumentadas ~30% (`200×150` → `260×195`, mesma proporção 4:3).
- **Arte de cartucho maior no painel** (`draw_panel`, 90px → 150px de
  altura — `CARTRIDGE_H` virou uma constante nomeada em vez de um número
  solto repetido em duas linhas).
- **Power/Reset como chaves gangorra roxas** (`draw_rocker`, nova função
  em `cabinet.rs`): um "track" recuado (retângulo escuro com borda) e um
  "thumb" roxo preenchendo metade dele, no topo ou embaixo — sem primitivo
  de retângulo arredondado na engine, então o efeito de chave física vem
  só de posição + uma tira mais clara no topo do thumb como bisel barato.
  Ejetar ficou no meio das duas, reaproveitando `draw_button` — mesmo
  espaço que o slot de cartucho ocupa no console real. Comportamento:
  Power é um alternador de verdade (posição = `panel.powered`, contínua
  até o próximo clique); Reset é **momentâneo** — sobe no clique e desce
  sozinho pouco depois (`runner::RESET_SPRING`, 220ms, mesmo mapa
  `flash: HashMap<PanelButton, Instant>` do "(feito!)" mas com uma janela
  mais curta e lida por `Cabinet::set_reset_pressed` em vez de mudar um
  rótulo de texto). `PanelInfo` ganhou o campo `reset_pressed`; Power/
  Ejetar/Reset saíram do laço genérico de `command_rows` (que ainda
  desenha Pausar/Nota/Salvar/etc. como texto) e das próprias entradas que
  `command_rows` gerava pra eles — o rótulo de texto que existia pra essas
  três virou morto assim que a chave gangorra passou a ignorá-lo, então
  foi removido de vez (`command_rows` perdeu o parâmetro `powered` que só
  servia pra isso).
- **Escolher um jogo só insere o cartucho, não liga sozinho**: `powered`
  em `run_game` passou a nascer `false` (era `true`) — a tela mostra o
  cartucho encaixado e estática de "desligado" até o jogador clicar Power,
  igual ao console de verdade (`Cabinet::set_panel` também passou a
  inicializar `powered: false`, consistente). Única exceção: uma captura
  `emu-run --shot` sem `--shot-off`/`--debug-shot-pause` (dev, testa
  gameplay ao vivo) nasce ligada — não existe "clicar Power" num teste
  sem tela, e sem essa exceção o `--shot` simplesmente travaria pra
  sempre no laço "desligado" (`let mut powered = spec.shot.is_some();`,
  mais um `cab.set_powered(powered)` logo em seguida pra sincronizar o
  painel com esse estado inicial).
- **Tela inicial = tela de "cartucho ejetado"**: as duas já eram a mesma
  tela por baixo (`idle.rs`, mostrada em startup/voltar da estante/ejetar
  — sempre foi um só `idle::run`), só que sem nenhum aproveitamento visual
  disso — agora o ramo `None` de `draw_panel` desenha a logo do console
  (`assets/console.png`, novo — `Cabinet::set_console_logo`, chave de
  textura reservada `PANEL_CONSOLE_LOGO_IMG`; sem o arquivo, cai pro texto
  "SNES Xperience", mesma regra de fallback do logo por jogo) no lugar do
  logo do jogo, "Inserir cartucho" (mesmo botão de sempre, só que agora do
  tamanho do slot de cartucho, 150px) no lugar da arte de cartucho, e
  "Configuracoes" foi realocado pro rodapé do painel (`rect.bottom() -
  pad - btn_h`, mesma posição do relógio de sessão durante o jogo) em vez
  de empilhado logo abaixo de "Inserir cartucho". Nenhum comando aparece
  (o painel idle nunca teve `PanelInfo`, então nunca desenhou
  Power/Ejetar/Reset/Pausar/etc. de qualquer forma — só ficou mais óbvio
  agora que o resto do layout ficou parecido com o do jogo).

Verificado ao vivo em `/Applications/SNES Xperience.app` (o binário `.app`
empacotado registra janela de verdade nas ferramentas de automação; o
binário cru de dev, rodado em background pelo shell, não — mesma limitação
já documentada nesta revisão): fluxo completo inserir → ligar → desligar →
ejetar, conferindo a chave Power subindo/descendo, Reset descendo sozinho,
Ejetar acendendo só quando desligado, e a tela final batendo com a tela
inicial original. Repetido com um `console.png` de teste (removido depois)
pra confirmar o caminho de imagem, não só o de texto. Build/clippy/test/
fmt limpos.
