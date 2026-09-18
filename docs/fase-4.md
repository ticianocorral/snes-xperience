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

## Revisão (2026-09-15): modais de Salvar/Carregar/Printscreen, "Pausar" vira "Anotacoes", saves organizados por jogo

Usuário achou a seção de comandos do painel confusa (Pausar, Nota, Nota
slot, Salvar, Carregar, Slot, Turbo — sete botões, vários deles só
existindo pra pré-selecionar o alvo de outro). Pedido: Salvar/Carregar
abrem uma modal pra escolher o slot; "Pausar" sai, "Nota" vira
"Anotacoes" e passa a só abrir o caderno (pausa + entra na tela já
existente); um botão novo, "Printscreen", assume a captura de tela,
também com modal (slot + nome opcional).

- **`Modal` (novo enum em `runner.rs`)**: `None`/`SaveSlot`/`LoadSlot`/
  `PrintSlot`, ortogonal a `paused` (o caderno continua sendo o dono
  exclusivo da tela de duas páginas) — trava o laço de gameplay do mesmo
  jeito (`(!paused && modal == Modal::None) || step_once`), mas desenha
  uma tela própria (`Cabinet::present_modal`), não o caderno.
- **`ModalInfo`/`draw_modal` (novos em `cabinet.rs`)**: um cartão único
  centralizado — título, uma grade de linhas clicáveis (uma ou duas
  colunas, conforme a contagem: 15 linhas de print em duas colunas de
  8/7 não vira uma coluna gigante) e um botão "Cancelar"; ou, na etapa de
  nomear um print, o mesmo cartão troca a grade por um campo de texto com
  contador e "Salvar"/"Cancelar" — mesma ideia do modo de rascunho do
  caderno (`PauseNote::draft`), implementação própria pra não arrastar a
  paginação/cheats do caderno pra uma tela que não precisa deles.
  `PanelButton` ganhou `ModalSlot(u8)`/`ModalConfirm`/`ModalCancel`.
- **Fluxo do print**: clicar "Printscreen" só marca `print_pending = true`
  — a captura de verdade acontece no mesmo quadro, quando `core.run()` já
  ia produzir um frame de qualquer jeito (mesma janela que
  `--debug-note-capture` já usava), clonado pra `print_capture: Option
  <EmuFrame>` e só gravado em disco quando o jogador confirma o nome
  (`NoteEdit::PrintName(slot)`, nova variante — reaproveita toda a
  máquina de digitação do caderno, incluindo o limite de 40 caracteres já
  usado por "Nomear print"). Cancelar em qualquer etapa descarta a
  captura sem gravar nada.
- **"Nota"/"Pausar" removidos, "Anotacoes" assume os dois papéis**:
  `PanelButton::Notebook` (renomeado de `Pause`) dispara o mesmo
  `UiEvent::TogglePause` de sempre — clicar pausa e já deixa o caderno
  aberto, sem precisar de um botão "Pausar" separado. `command_rows`
  perdeu os parâmetros `slot`/`note_slot` (os rótulos não precisam mais
  mostrar um número pré-selecionado — quem escolhe agora é a modal) e
  ficou com só cinco linhas: Anotacoes, Printscreen, Salvar, Carregar,
  Turbo.
- **`saves/` ganhou pasta por jogo** (`game_dir`, mesmo sanitizador de
  nome que `note_dir` já usava — extraído pra uma função só,
  `sanitize_dir_name`, pra não duplicar a lista de caracteres proibidos):
  `saves/<título>/0.state`..`9.state`, `sram.srm`, `cheats.txt`, no lugar
  de `<hash-sha1>.state0`, `<hash>.srm`, `<hash>.cheats` soltos direto em
  `saves/`. Pedido explícito do usuário ("os códigos ficam feios"), mesma
  motivação e mesmo trade-off já aceito pra `notes/` (legibilidade pra
  quem abre a pasta a mão, em troca de perder a resistência a rename que
  o hash dava). Efeito colateral: `rom_hash` não tinha mais nenhum uso
  fora desse propósito, então saiu de `run_game` inteiro — uma ROM que
  falha a identificação agora salva normalmente (a chave é o título, que
  sempre existe, nem que seja o nome do arquivo), onde antes ficava sem
  save state nenhum.
- **`--debug-shot-modal save|load|print|print-name`** (novo em
  `emu-run`): mesma ideia do `--debug-shot-pause` já existente, prova as
  quatro telas (três grades + a etapa de nome) sem precisar de clique
  nenhum — foi assim que as capturas abaixo saíram, depois que a sessão
  de `computer-use` ficou instável (janela sumindo, cliques não
  confiáveis) no meio da verificação ao vivo.

Verificado com `--debug-shot-modal` pras quatro telas (grades de 10/10/15
linhas mais a etapa de nome) e com testes novos
(`save_paths_are_grouped_by_game_with_plain_names`,
`game_dir_sanitizes_path_hostile_titles`) cobrindo os nomes de arquivo.
Antes disso, ao vivo: escolher o jogo → ligar → clicar "Anotacoes" abriu
o caderno de verdade (com um slot já fixado/nomeado de um teste anterior,
provando que o pin/rename da revisão passada sobreviveu intacto) — a
sessão de automação ficou instável logo depois (cliques íntermitentemente
não confiáveis, provavelmente do lado do host, não do app) antes de dar
pra clicar nos três botões novos ao vivo; o `--debug-shot-modal` supriu o
resto da verificação visual. Build/clippy (`-D warnings`)/test/fmt
limpos.

## Revisão (2026-09-15, continuação 2): botão "Turbo" removido

Pedido direto do usuário, sem contexto adicional — o fast-forward saiu por
completo, não só o botão: `PanelButton::Turbo`/`UiEvent::ToggleFastForward`
(o clique), `Input::fast_forward`/`set_fast_forward` (o estado, em
`crates/platform`), `FF_SPEED`/o `let ff = ...` em `runner.rs` (os quadros
extra sem áudio/run-ahead) e o ramo de paciente-de-quadro que rodava "sem
acumular dívida de tempo" enquanto ligado — sem esse ramo, o laço sempre
usa o único caminho de espera que sobrou (dormir até `frame_time`).
`command_rows` perdeu o parâmetro `turbo_on` (não sobrou nenhum rótulo que
precisasse dele). Nenhuma outra tela referenciava o conceito.

Verificado com `--shot` (painel sem a linha "Turbo", as quatro que sobraram
— Anotacoes/Printscreen/Salvar/Carregar — no lugar certo) e o ciclo
completo de build/test/clippy (`-D warnings`)/fmt.

## Revisão (2026-09-15, continuação 3): cheats num menu próprio, painel opt-in por pin, "avancar quadro" removido, bug de cheat corrigido

Quatro pedidos diretos do usuário depois de testar a leva anterior de
modais:

- **Cheats saíram do caderno de anotações**: viviam na página esquerda de
  "Anotacoes" (uma lista `[x]`/`[ ]` clicável, `PanelButton::CheatRow`)
  competindo por espaço com o status dos slots e o botão "Escrever
  anotacao". Agora é um botão próprio na lista de comandos — `Cheats`
  (`command_rows` só o inclui quando `!cheat_defs.is_empty()`, mesma
  regra de "esconder em vez de mostrar tela vazia" que o Printscreen já
  usava pros 15 slots fixados) — que abre uma modal dedicada
  (`Modal::Cheats`), reaproveitando o sistema de modal do Salvar/
  Carregar/Print em vez de inventar um terceiro tipo de tela. A
  diferença: uma modal normal fecha ao escolher uma linha (`ModalPick`
  seta `modal = Modal::None`); a de cheats não — o pick alterna o estado
  e o `cab.set_modal(...)` é chamado de novo com as linhas atualizadas
  (`[x]`/`[ ]` recalculado por `cheat_modal_rows`), então a modal continua
  aberta pra ligar/desligar vários de uma vez. `PanelButton::CheatRow` e
  `PauseStep` saíram do enum inteiro — o antigo clique de cheat agora usa
  `PanelButton::ModalSlot` como qualquer outra linha de modal.
- **"Avancar quadro" removido do caderno**: um botão de depuração (um
  frame por clique, pausado) sem uso real pra quem só quer ler/escrever
  anotações — nunca teve tecla nem foi mencionado em nenhum lugar do
  app fora do próprio caderno. Saiu por completo: `UiEvent::FrameStep`,
  `PanelButton::PauseStep`, a variável `step_once` e os dois lugares que
  a liam (o gate de step da gameplay, o `!step_once` no cálculo de
  run-ahead especulativo). O "Continuar" que sobrou ocupa a largura toda
  da faixa de botões, não mais metade.
- **Painel lateral: notas viram opt-in por pin.** `refresh_notes` contava
  quantos dos 15 slots tinham *algum* arquivo (`count_filled_slots`) e
  mostrava a miniatura do slot atualmente selecionado (`note_slot`),
  fixado ou não — na prática, qualquer captura recente aparecia ali sem
  o jogador ter pedido. Agora conta só os fixados
  (`meta.slot(s).pinned`) e a miniatura é a do slot atual *se* estiver
  fixado, senão a do primeiro fixado que achar, senão nenhuma — a seção
  "notas" some inteira quando não há nenhum pin. Precisou de um
  `refresh_notes` a mais: o toggle de pin (`NotePinToggle`) já salvava a
  meta e atualizava a página do caderno, mas não recomputava o bloco do
  painel — sem isso, fixar um slot só refletiria no painel na próxima
  captura, não assim que o jogador voltasse do caderno.
- **Bug real corrigido: desligar um cheat não desligava o efeito** — a
  linha virava `[ ]` mas o jogo continuava com o cheat ativo. Um único
  `core.cheat_set(indice, false, codigo)` não é garantia de que o core
  desfaça um patch de memória sustentado (nem todo core trata "enabled:
  false" como "reverte na hora"); a correção padrão (a mesma que o
  RetroArch usa) é resetar e reaplicar a lista inteira a cada mudança, não
  só o índice tocado — exatamente o que o carregamento inicial já fazia
  (`core.cheat_reset()` + laço reaplicando todo mundo), só que só na
  primeira vez. Agora o toggle dentro do `Modal::Cheats` faz a mesma
  sequência.

Verificado com `--debug-shot-modal cheats` (grade `[ ]`/`[x]` com os três
códigos curados de Street Fighter II Turbo, "Cancelar" embaixo) e
`--debug-shot-pause` (caderno sem cheats, sem "Avancar quadro", só
"Continuar" ocupando a largura toda). O sistema de pin foi verificado nos
dois sentidos com uma `slots.json` fabricada à mão: slot fixado -> painel
mostra "notas"/"N slot(s) fixado(s)" com a miniatura; mesmo slot com
`pinned: false` (mas ainda com imagem no disco) -> seção some por
completo. Essas duas capturas precisaram da janela 1280×800 de
`xperience.rs` (trocada de volta pra 1024×768 depois) — a pequena e
não-16:9 de `emu-run` já é conhecida por cortar as últimas linhas do
painel (ver revisão anterior), e o botão Cheats novo empurrou o bloco de
notas pra fora da área visível nela. Build/test (13 suítes)/clippy
(`-D warnings`)/fmt limpos.

## Revisão (2026-09-16): base de cheats vira a base inteira do libretro-database, rolagem na modal, um crash real corrigido

Pedido direto do usuário depois de duas perguntas sobre a feature de
cheats ("o DAT do nointro está embutido?" / "e os cheats está pegado da
onde?", respondidas nesta mesma sessão): a lista curada de ~20 jogos em
`cheats.rs` virou pouca coisa perto do que o `libretro-database` realmente
tem — "aumente o banco de dados, coloque tudo que tem lá no libretro" —
com o aviso já embutido de que a lista ia crescer bastante e ia precisar
de rolagem.

**De onde veio.** `git clone --filter=blob:none --sparse` do
`libretro-database`, `sparse-checkout` só da pasta
`cht/Nintendo - Super Nintendo Entertainment System/` (2773 arquivos,
15 MB) — mais rápido e mais leve que baixar o repositório inteiro (que
cobre todo console suportado pelo libretro) ou paginar pela API do GitHub
arquivo por arquivo. `scripts/gen_cheats_data.py` (novo, comitado —
referenciado pelo próprio doc comment de `cheats.rs`, pra a base poder
ser regerada quando o upstream atualizar) converte cada `.cht` (formato
ini do libretro: `cheatN_desc`/`cheatN_code`/`cheatN_enable`) num formato
bem mais compacto — `G\t<nome do jogo>` seguido de `C\t<descrição>\t<código>`
por linha — descartando cheats com código vazio ou com um placeholder de
valor ajustável (`X`/`?` literal no código, ex. `"7FC136XX"`: o
jogador escolheria o byte na UI do RetroArch, coisa que nosso checkbox
on/off simples não tem campo pra oferecer). Resultado: 2401 jogos, 67 mil
códigos, 2.7 MB de texto — `include_str!`'d direto em
`crates/domain/src/cheats_data.txt` (comitado; ver
`THIRD-PARTY-NOTICES.md`, atualizado — desta vez tanto código quanto
*descrição* são da base, não reescritos, então o arquivo em si carrega a
mesma licença CC BY-SA 4.0 por *share-alike*).

**Por que o casamento mudou de "título do cabeçalho SNES" pra "título do
arquivo".** A tabela curada anterior usava o título interno do cabeçalho
SNES (ex. `"ALADDIN"`, `"STREET FIGHTER2 TURBO"`) porque sobrevive a um
re-dump ou rename — mas isso exige *ler* o cabeçalho de cada jogo pra
saber a chave certa, e não há como fazer isso pros ~2400 jogos da base
sem possuir cada cartucho. A única chave que o app já tem de graça, pro
número que for de jogos, é o nome do arquivo da ROM — a mesma string que
`saves/`/`notes/` já usam (`title`, calculado uma vez em `run_game`).
`xperience_domain::cheats::for_title` (doc comment reescrito) casa em
duas etapas: exato (case-insensitive) primeiro — um arquivo nomeado do
jeito usual da cena, `Titulo (Regiao).sfc`, bate direto com o mesmo nome
que o `libretro-database` usa; se isso não achar nada, uma etapa mais
solta ignora todo grupo `(...)` no fim de ambos os lados (região, revisão,
"Action Replay"/"Game Genie") e casa pelo nome base. Efeito colateral:
`internal_name` (o título do cabeçalho) perdeu seu único uso em
`run_game` — só sobrou o log de identificação da ROM.

**Um bug real pego no meio do caminho, não hipotético.** A primeira
versão da etapa solta pegava o primeiro arquivo cujo nome base batesse —
e alfabeticamente isso às vezes é o arquivo errado: `"Final Fantasy III
(USA) (Action Replay).cht"` (29 cheats) ordena antes de `"Final Fantasy
III (USA).cht"` (2418 cheats) porque `' '` (espaço) vem antes de `'.'`
em ASCII. Um teste manual pegou isso na hora (uma ROM chamada só "Final
Fantasy III" abriu a modal com 29 linhas, óbvio demais pra passar batido).
Trocado pra pegar, entre todos os arquivos que batem pelo nome base, o
que tem *mais* cheats — sem precisar hardcodar nenhuma regra de
prioridade entre tags de região/formato, e resolve o caso certo de
qualquer forma (a lista mais completa é a lista mais completa,
independente de qual arquivo ela mora).

**O crash de verdade.** Testando a modal nova com uma lista grande (2418
cheats, forçado renomeando temporariamente uma ROM de teste pra "Final
Fantasy III.sfc" — o conteúdo real não importa pro teste, só o nome do
arquivo, já que o casamento é por título agora), o app abortou
(`SIGABRT`) no meio do carregamento — sem nenhum panic do Rust, só um
`abort trap: 6`. O crash report do macOS
(`~/Library/Logs/DiagnosticReports`) apontou o culpado exato:
`__stack_chk_fail` dentro de `retro_cheat_set`, chamado pelo `core.cheat_set`
de `run_game` — um estouro de buffer na pilha *dentro do próprio core*
snes9x-libretro, disparado por um código de cheat de 602 caracteres (67
endereços `7E......` encadeados com `+`, um cheat de "escreve a tabela
inteira" que a base tinha codificado como um combo gigante em vez de
vários cheats pequenos). Não dá (nem faz sentido tentar) consertar o
parser do core; a correção do lado de cá é defensiva: `gen_cheats_data.py`
descarta qualquer código acima de 96 caracteres na geração (folga grande
sobre os 602 que de fato travaram — a exata margem segura do buffer do
core nunca foi determinada por bisseção, não valia o risco de travar o
processo repetidas vezes só pra achar o número exato). Só 148 dos 67 mil
códigos (0.2%) passavam desse limite. `cheats.rs` ganhou um teste de
regressão (`no_cheat_code_is_long_enough_to_crash_the_core`, limite de
128 — folga extra sobre os 96 da geração) pra pegar isso automaticamente
se a base for regerada um dia sem essa cautela.

**Rolagem.** `ModalPick`/`PanelButton::ModalSlot` eram `u8` (a maior
lista antes disso, Printscreen, tinha 15) — não cabe mais um índice de
até 2418; virou `u16`. `draw_modal` calculava a altura do card pra caber
*todas* as linhas de uma vez e só recortava o card no fim (`card_h.min(...)`)
sem nunca deixar de desenhar as linhas que sobravam — ou seja, uma lista
grande simplesmente vazava pra fora do card, sem clique nem visual
corretos. Reescrito: calcula quantas linhas cabem sem nenhuma rolagem
(igual antes, pra Salvar/Carregar/Print de sempre — o caminho comum não
muda em nada); só se a lista *ainda* não couber é que entra o modo com
rolagem, reservando duas linhas de botão (`^ Cima`/`v Baixo`, dimmed nas
pontas) e recortando o card numa altura razoável. O desenho em si
continua iterando todas as linhas (o índice original de cada uma
`i` precisa sobreviver pro clique bater certo), só pula (`continue`) as
que caem fora da janela visível — nenhum desenho de texto de fato
acontece pra uma linha fora de tela, então o custo por quadro fica preso
em ~24 botões não importa se o jogo tem 3 cheats ou 2400. `scroll` mora
em `ModalInfo`, sobrevive a um `set_modal` que só está *atualizando* a
mesma modal aberta (Cheats chama de novo a cada toggle, pra redesenhar
os `[x]`/`[ ]`) e só zera numa abertura genuinamente nova — sem isso,
marcar um cheat na página 50 voltaria a modal pro topo a cada clique.

Verificado: `--debug-shot-modal cheats` pra um jogo pequeno (Street
Fighter II Turbo real, 2 cheats — layout idêntico a antes, sem UI de
rolagem) e pra um grande (2418 cheats, via o truque do nome de arquivo
trocado) — o segundo mostrando `^ Cima`/`v Baixo`/contador `"1-12/1209"`
corretamente, sem o crash (re-testado depois da correção, confirmando
que o mesmo cenário exato que travava antes agora sai limpo). `cargo
test --workspace` (12 suítes no domain agora, +1 sobre a revisão
anterior), clippy (`-D warnings`) e fmt limpos.

## Revisão (2026-09-16, continuação): texto quebrado na modal de Cheats corrigido, busca adicionada

Relato direto do usuário depois de usar a modal nova: "a tela de cheats
ta com os textos quebrados. melhorar visibilidade e adicionar busca para
achar truques".

**O bug de verdade.** `draw_button` centralizava o texto calculando sua
largura total e nunca cortava nada — pra um rótulo mais largo que a
caixa, `(largura_da_caixa - largura_do_texto)` dava negativo, o `.max(4)`
defensivo virava só "comece 2px pra dentro", e dali o texto desenhava
até o fim, sem clipping nenhum contra o retângulo da caixa (nem o SDL
faz isso sozinho — `canvas.copy` não recorta). Com rótulos curtos
("Slot 3", "Salvar") isso nunca aparecia; a modal de Cheats, com
descrições de dezenas de caracteres vindas direto da base do
libretro-database, tornou o bug visível pela primeira vez — o texto de
uma linha invadia visualmente a caixa vizinha (ou a coluna do lado, no
grid de 2 colunas). Corrigido com uma função nova, `clip_label`: corta
pra caber e acrescenta `"..."` (não o caractere único `…`, que fica fora
da faixa que a fonte bitmap cobre — só Basic Latin até Latin-1
Supplement, `GLYPH_FIRST`/`GLYPH_LAST`) quando precisa. `draw_button`
passou a usar isso sempre — corrige a mesma classe de bug em qualquer
botão do app, não só nos da modal, ainda que só a modal de Cheats
alcançasse o cenário na prática.

**Visibilidade além do corte.** Cortar era necessário mas não suficiente
pra "melhorar visibilidade" — cortar toda hora deixaria a lista ilegível
de qualquer jeito. A largura da coluna (`col_w`), fixa em 200px desde a
primeira versão da modal, virou calculada a partir do rótulo mais
comprido *da lista inteira* (não da página visível — pra não mudar de
tamanho a cada rolagem/filtro), com piso de 200 (mantém Save/Load/Print,
rótulos curtos, do jeito que já estavam) e teto no que a janela
realmente comporta (`out_w - 160`, dividido pelas colunas, com folga pro
`col_gap`) — o que sobrar de rótulo além disso ainda corta com `...`,
mas na prática a maioria das descrições de cheat (na faixa de 20-40
caracteres) já cabe inteira numa janela de tamanho normal (1280×800),
onde antes cabiam uns 20 caracteres só.

**Busca.** Só a modal de Cheats marca `searchable` (`Cabinet::
set_modal_searchable`, chamado uma vez ao abrir — `set_modal` carrega
esse flag adiante sozinho nos refreshes seguintes, do mesmo jeito que já
fazia com `scroll`) — Save/Load/Print continuam sem a caixa de busca,
suas listas nunca precisaram disso. Reaproveita a mesma máquina de
digitação por trás de "Escrever anotacao"/"Nomear print"
(`Platform::poll_text_entry`, o único desvio deliberado de teclado do
app): `NoteEdit` ganhou uma variante `CheatSearch`, tratada quase igual
a `PrintName` (mesmo `in_modal = true`, mesmo card cheio de campo de
texto) com uma diferença de propósito — ao confirmar, em vez de gravar
algo em disco e fechar a modal, só chama `cab.set_modal_search(...)` e
*volta* pro grid da própria modal de Cheats (o fechamento genérico de
"salvar ou cancelar" ganhou um `if matches!(note_edit, NoteEdit::
CheatSearch)` pra pular o `modal = Modal::None` que toda outra edição em
modal dispara). O filtro em si (`contains_ignore_ascii_case`, sem
alocação — compara bytes direto, adequado pras descrições em ASCII da
base) roda dentro de `draw_modal`: cada linha guarda seu índice
*original* (o mesmo que `ModalSlot(i)` usa pra achar o cheat certo em
`cheat_defs`/`cheat_state` no clique), só que agora um passo de filtro
decide quais índices participam da conta de colunas/rolagem antes de
desenhar — mesmo padrão "itera tudo, pula o que não se aplica" que a
rolagem já usava, com mais uma condição de pular. Card mostra "nenhum
cheat encontrado" no lugar do grid quando o filtro não bate com nada, e
o contador (`"1-7/7"`) passa a refletir o total *filtrado*, não o total
do jogo. Trocar de filtro reseta a rolagem pra 0 — a página em que
alguém estava sob a lista inteira não significa nada sob a filtrada.

**Achado de refinamento, não bug**: enquanto testava a busca, ~460
linhas da base tinham entidades HTML escapadas na descrição
(`&quot;USE ITEMS INFINITELY&quot;` em vez de aspas de verdade) —
sobrevivência do HTML original de onde o `libretro-database` tirou
algumas descrições. `scripts/gen_cheats_data.py` ganhou um
`html.unescape()` na função `clean()`; a base já commitada
(`cheats_data.txt`) recebeu a mesma limpeza direto (sem precisar re-clonar
os 15 MB do libretro-database de novo), preservando a contagem de linhas
e a estrutura `G`/`C` intacta (conferido com `awk` antes de aceitar o
resultado).

Verificado com `--debug-shot-modal cheats`/`cheats-search` (novo, kind
de debug fixo pra pré-aplicar um filtro sem precisar de clique — abre
"Cheats" já filtrado por `"infinit"`, dev/testing only) pro jogo grande
(renomeado pra "Final Fantasy III", 2418 cheats): sem filtro, larguras
maiores e sem sobreposição, um rótulo genuinamente longo cortando com
`...`; com filtro, `"1-7/7"` e as 7 linhas batendo todas com "infinit"
em algum lugar da descrição, aspas de verdade em vez de `&quot;`. Também
re-testado o caso pequeno (Street Fighter II Turbo de verdade — dessa
vez casou com uma entrada de 126 cheats, não os 2 de antes, já que o
fallback "mais cheats vence" da revisão anterior prefere a lista mais
rica quando existe mais de uma pro mesmo nome) — mesmo layout limpo,
caixa de busca presente mesmo numa lista que cabe numa página só. Build/
test (13 app + 12 domain)/clippy (`-D warnings`)/fmt limpos.

## Revisão (2026-09-16, continuação 2): anotações de texto viram 15 slots visíveis (pin, editar, apagar)

Relato direto do usuário: "as anotações em texto eu escrevo mas nao
consigo ver o que eu salvei. faça igual as screenshots. 15 anotações,
podendo deletar, fixar ou editar".

**O que existia até aqui.** A página esquerda do caderno era só um
botão "Escrever anotacao" que sempre abria um campo em branco e, ao
salvar, *acrescentava* o texto a `notas.txt` (`append_note_text`) —
um log sem limite, sem numeração, sem qualquer forma de reabrir e ver o
que já estava lá. O status acima do botão ("N de 15 slots usados")
também estava, na prática, **errado**: `count_filled_slots` contava os
slots de *print* (`.png`), não os de texto — então o texto do jogador
nunca influenciava aquele número. Isso confirma exatamente a queixa: dá
pra escrever, mas não pra ver de volta o que foi escrito.

**A virada.** A página esquerda passou a ser um visualizador de slot,
espelhando quase exatamente o que a página direita (álbum de prints) já
fazia — mesmo layout de baixo pra cima (linha Fixar/Apagar, depois
Escrever/Editar, depois `< anterior`/`proxima >`, depois a linha de
info "N/15 (fixado)"), mesmo conceito de "navega até o slot, depois age
nele". Mudanças concretas:

- **Armazenamento**: `notes_dir/<título>/01.txt`..`15.txt`, numerados
  independentemente dos slots de print (`01.png`..`15.png` continuam
  existindo lado a lado — extensão diferente, sem colisão possível
  mesmo usando o mesmo número). `save_text_slot`/`read_text_slot`/
  `delete_text_slot` substituem `append_note_text` por completo.
- **Cursor próprio**: `text_slot` (novo, começa em 1) é totalmente
  independente de `note_slot` (o cursor dos prints) — navegar pelos
  prints nunca move qual anotação de texto está sendo mostrada, e
  vice-versa.
- **Pin reaproveitado**: `NotesMeta` ganhou `text_slots` (mesmo
  `SlotMeta{pinned,label}` dos prints, mapa separado) — fixar uma
  anotação de texto usa exatamente o mesmo conceito de proteção que já
  existia pros prints, só que agora protegendo contra *duas* coisas:
  sobrescrita (editar) *e* exclusão (apagar). Um slot fixado mostra
  "Editar" e "Apagar" visualmente apagados (`draw_button`'s `lit`) — o
  clique em si é bloqueado pela guarda no handler do evento
  (`if !notes_meta.text_slot(text_slot).pinned`), mesmo padrão que os
  slots de print desabilitados na modal de Printscreen já usavam.
- **Apagar, novo**: não existia pra nada no app até aqui (primeira ação
  genuinamente destrutiva). Em vez de um diálogo de confirmação (que não
  existe em nenhum lugar do app ainda), a proteção é o próprio "Fixar"
  já estabelecido — pra apagar algo que importa, fixar primeiro é a
  cautela; um clique isolado em "Apagar" já vale pra qualquer slot não
  fixado, sem confirmação extra (chamada de julgamento: piorar a fricção
  de um caso comum — testar/limpar rascunhos — não parecia valer pela
  proteção extra, já que "Fixar" cobre o caso realmente importante).
- **Editar em vez de sempre-sobrescrever-em-branco**: "Escrever" (slot
  vazio) vira "Editar" (slot ocupado) — mesmo padrão que "Nomear
  print"/"Renomear" já usava pro nome de um print. O campo abre
  pré-preenchido com o texto salvo, e salvar vazio (backspace tudo,
  confirmar) apaga o slot — uma segunda forma de apagar, mais natural
  pra quem já está editando, ao lado do botão dedicado.
- **Migração automática**: `migrate_legacy_text_notes`, chamada uma vez
  no boot (perto de onde `notes_meta` já carrega) — se nenhum slot
  numerado existir ainda e `notas.txt` existir, separa o conteúdo pelas
  linhas em branco (o formato que `append_note_text` sempre escreveu,
  `"{texto}\n\n"`) e distribui nas primeiras até 15 páginas não-vazias
  encontradas, nessa ordem. `notas.txt` não é apagado — só passa a não
  ser mais lido depois da primeira migração (nenhum slot numerado =
  ainda não migrou; qualquer slot numerado = já migrou, não repete).
  Isso resolve a queixa também pra quem já tinha escrito algo antes
  desta revisão: a nota "perdida" aparece de volta, visível, na
  primeira vez que abrir o jogo depois de atualizar.
- **`PauseNote`/`Cabinet`**: novos campos `text_page`/`text_pinned`/
  `text_content` (espelham `page`/`pinned`/`slot_label` dos prints);
  novo `set_pause_text_page` (espelha `set_pause_page`); `filled`
  removido de `PauseNote`/`set_pause_note` — a página esquerda não
  precisa mais de um resumo "N usados" agora que dá pra simplesmente
  passear pelos slots e ver "slot vazio" ou o conteúdo direto, mesma
  razão pela qual a página direita nunca teve um resumo desses.
  `count_filled_slots` saiu inteiro (só existia pra alimentar aquele
  resumo, que aliás contava a coisa errada — ver acima).
- Quatro `PanelButton`/`UiEvent` novos: `PauseTextPrev`/`TextPrev`,
  `PauseTextNext`/`TextNext`, `PauseTextPin`/`TextPinToggle`,
  `PauseTextDelete`/`TextDelete`. `PauseWrite`/`NoteWriteStart` foram
  reaproveitados (mesmo nome, novo comportamento: agora mira o
  `text_slot` atual e pré-preenche, em vez de sempre abrir em branco).

Verificado com `--debug-shot-pause`: caderno com os dois lados vazios
("slot vazio" nas duas páginas, "Apagar" já nascendo desabilitado);
depois com uma anotação de texto de verdade escrita à mão num `01.txt`
de teste (apareceu inteira, com quebra de linha automática, botão
virou "Editar"); depois com esse mesmo slot marcado como fixado em
`slots.json` (info line virou "1/15 (fixado)", botão virou "Fixado",
"Editar" e "Apagar" visualmente apagados) — as três capturas batendo
exatamente com o layout pretendido, espelhando a página de prints ao
lado. Suite de testes do módulo reescrita: saiu o teste do antigo
append-log, entraram `text_slot_round_trips_and_deletes`,
`whitespace_only_text_slot_reads_back_as_empty`,
`legacy_notas_txt_migrates_into_numbered_slots`, e
`migration_is_a_noop_once_any_text_slot_exists` (16 testes no crate
`app` agora, antes 13). Build/test/clippy (`-D warnings`)/fmt limpos.

## Revisão (2026-09-16, continuação 3): painel com contagem de cheats, filtro ligado/desligado na modal, painel com nota de texto

Pedido direto do usuário: "no painel, mostrar apenas '1 cheat ativado'
'15 cheats ativados' - nao liste cada um dos cheats. na lista de cheats
possibilitar filtro ativado / desativado. possibilitar mostrar apenas
uma nota e/ou uma imagem das anotações no painel".

**Painel: só a contagem.** A seção "cheats ativos" do painel lateral
listava a descrição de cada cheat ligado, um por linha — útil quando a
base era um punhado de jogos curados com 2-3 cheats cada, mas depois da
expansão pra base inteira do libretro-database um jogo pode ter dezenas
ligados ao mesmo tempo, cada descrição competindo por espaço com as
outras seções do painel. Trocado por uma linha só: "N cheat(s)
ativado(s)" — a lista completa (com checkbox, buscável, filtrável) já
mora na modal de Cheats, então nada se perde por não repeti-la aqui.

**Filtro ligado/desligado na modal.** Três botões — "Todos"/"Ligados"/
"Desligados" — logo abaixo da caixa de busca, mesmo estilo de "segmented
control" (um sempre `lit`, os outros dois dimmed mas clicáveis). O
estado on/off de cada linha não tinha um campo próprio em `ModalRow` —
em vez de acrescentar um array paralelo só pra isso, o filtro lê o
mesmo prefixo `"[x] "`/`"[ ] "` que `cheat_modal_rows` já grava no
rótulo pra desenhar o checkbox (`row_checked`, novo helper) — reaproveita
a única fonte da verdade que já existia em vez de duplicá-la. O filtro e
a busca combinam com `&&`: dá pra buscar "infinit" *e* filtrar só os
ligados ao mesmo tempo. `cheat_filter: Option<bool>` mora em `ModalInfo`,
carregado adiante entre chamadas de `set_modal` do mesmo jeito que
`search_query` já era (senão cada refresh de checkbox depois de um
toggle resetaria o filtro escolhido).

**Painel: nota de texto fixada, independente da imagem.** O bloco
"notas" do painel já mostrava uma miniatura de print fixado; agora
também mostra o conteúdo de uma anotação de texto fixada, do mesmo jeito
("mostrar apenas uma nota e/ou uma imagem" — as duas são buscadas e
mostradas independentemente, cada uma podendo estar presente, ausente,
ou as duas ao mesmo tempo). `refresh_notes` ganhou um parâmetro
`text_slot` e passou a fazer a mesma busca "o slot atual se estiver
fixado, senão o de menor número fixado" duas vezes — uma pro lado das
imagens (já existia), outra pro lado do texto (novo) — e `Cabinet::
set_notes` ganhou um terceiro parâmetro `text: Option<&str>`. O texto
mostrado no painel é cortado em 120 caracteres
(`PANEL_NOTE_SNIPPET_CHARS`/`panel_text_snippet`, novo, com reticências
se passar disso) — o painel não tem como sobrar espaço pra um texto de
até 240 caracteres (`NOTE_CHAR_LIMIT`) inteiro ao lado de tudo mais que
já mora ali; mesma ideia de "versão pequena pro painel, inteira no
caderno" que a miniatura de foto (200px vs 900px) já usava.

**Um bug real, achado testando.** A verificação headless da contagem de
cheats ("3 cheats ativados") simplesmente não aparecia — nenhum erro,
só ausência silenciosa. Rastreado com `log::info!` temporário: `cab.
set_cheats(...)` já tinha sido chamado com os 3 cheats corretos, mas
`panel.cheats.len()` no momento de desenhar era 0. Causa: `set_cheats`
era chamado *antes* de `set_panel` em `run_game` — e `set_panel`
substitui a `PanelInfo` inteira por uma nova, `cheats: Vec::new()`
incluso, apagando o que acabara de ser gravado. Bug antigo, não desta
sessão: veio de uma revisão anterior que moveu o carregamento de cheats
pra *antes* do bloco do painel (pra `command_rows` saber se mostra o
botão "Cheats"), levando `set_cheats` junto por engano — o carregamento
em si (`cheat_defs`/`cheat_state`) precisa mesmo vir antes, só a
chamada de UI (`set_cheats`) não. Corrigido movendo só essa chamada pra
depois de `set_panel`; a seção de cheats do painel nunca tinha
funcionado antes disso, em nenhuma versão publicada.

Verificado com `--debug-shot-modal cheats-filtered` (novo kind de
debug, dev/testing only — força o primeiro cheat ligado e aplica o
filtro "Ligados" ou "Desligados" sem precisar de clique) mostrando
"Ligados" reduzindo pra só a 1 linha ligada e "Desligados" pros outros
125 de uma lista de 126; e com capturas do painel (janela 1280×800,
trocada de volta depois) numa pasta de save/notes fabricada à mão —
`cheats.txt` com 3 linhas "1" (→ "3 cheats ativados"), depois só 1 linha
(→ "1 cheat ativado", singular correto), com uma imagem e um texto
fixados ao mesmo tempo aparecendo juntos no bloco "notas". Build/test (16
app + 12 domain)/clippy (`-D warnings`)/fmt limpos.

## Revisão (2026-09-17): painel da estante sai de dentro do tubo, filtro/recentes/rolagem na estante, DAT alimenta o painel

Pedido do usuário, quatro partes sobre a estante: (1) o painel de
detalhes não pode ficar dentro da TV; (2) se o DAT No-Intro tiver
informação além do nome, mostrar no painel; (3) filtro pra achar jogos
na lista; (4) mostrar os últimos 5 jogados numa categoria separada no
início; (5, veio junto) indicador de rolagem na grade.

**O painel estava mesmo dentro do tubo.** `shelf::run` desenhava sua
própria coluna de detalhes (`PANEL_W` = 380px) *dentro* do fechamento
passado a `Screen` — o mesmo buffer que depois é distorcido pelo warp
CRT (`build_crt_mesh`) junto com a grade de jogos. O painel lateral
*durante o jogo* (`Cabinet::set_panel`/`draw_panel`) nunca teve esse
problema: `present_frame` reserva `panel_rect` primeiro, desenha o vídeo
só no que sobra (`cab_w = out_w - panel.width()`) e pinta o painel por
cima, plano, depois de tirar o viewport do tubo. A estante nunca fazia
essa divisão — só usava `screen_size()` (a área inteira do tubo) e
desenhava tudo, painel incluso, como conteúdo de jogo.

Correção: replicar a mesma divisão pro caminho da estante, sem tocar no
caminho 2D genérico (`frame_2d`/`capture_2d`/`frame_2d_fade_in`, que
`settings.rs` e a tela idle de `xperience.rs` continuam usando como
estavam — só a estante precisava mudar). `paint_2d` virou
`paint_2d_avail(bg, draw, avail_w)` recebendo a largura disponível;
`paint_2d` chama com a largura cheia (comportamento antigo, intacto),
`paint_shelf` (novo) chama com `out_w - panel_rect(...).width()`.
`composite_shelf`/`frame_shelf`/`capture_shelf`/`frame_shelf_fade_in`
(novos, espelhando os quatro métodos genéricos) desenham o painel plano
por cima do tubo já composto, usando um novo par
`ShelfPanelInfo`/`ShelfButton` — deliberadamente *não* o `PanelInfo`/
`PanelButton` do jogo, que carrega botões de console (Power/Eject/Reset,
comandos) sem sentido nenhum fora de uma partida; a estante só precisa
de logo/cartucho/contagem de jogadas/infos do DAT e dois botões
(Voltar/Configuracoes). `Cabinet::shelf_screen_size()` espelha
`screen_size()` descontando o painel, e é o que `shelf.rs` agora usa
pra dimensionar a grade — como o painel não rouba mais espaço da área
de jogo, a grade ganhou bem mais colunas/linhas de bônus.

**DAT alimentando o painel.** `NoIntroDat` só guardava nome por CRC32;
virou um `HashMap<String, NoIntroGameInfo>` com `name` obrigatório e
`description`/`year`/`publisher`/`category` opcionais (lidos de
elementos-filho de `<game>` quando existem — nenhum DAT No-Intro comum
tem esses campos, mas um TOSEC ou Logiqx mais rico pode ter; tudo
condicional, nada aparece se o DAT não tiver). `RomRow` ganhou
`nointro_extra: Vec<(String, String)>` (rótulo/valor, só os campos
presentes), montado em `Catalog::open` e clonado direto pro
`ShelfPanelInfo.info` do jogo focado.

**Filtro de título.** Reaproveita o mesmo padrão do caderno de notas —
um modo de digitação (`editing_filter`/`filter_draft`) que troca
`poll_menu` por `poll_text_entry` enquanto ativo, Enter aplica
(`filter_query`), Esc cancela o rascunho. Substring, sem diferenciar
maiúsculas, sobre `entry.title()` (já resolvido: DAT > nome interno >
nome do arquivo). Zero resultados mostra "nenhum jogo encontrado" no
painel em vez de deixar a grade e o painel vazios sem explicação.

**"Jogados recentemente".** Uma faixa própria (`RecentLayout`, até 5
itens, nunca rola) calculada toda visita a partir de
`last_played_at.is_some()` nas ROMs, ordenada por mais recente — some
enquanto o filtro está ativo (não faz sentido recapitular jogos
recentes no meio de uma busca por um jogo específico). Tem cursor
próprio (`in_recent`/`recent_idx`), com Up/Down trocando de uma faixa
pra outra na primeira/última linha da grade principal — mesmo game pode
aparecer nas duas listas (a estante inteira ainda lista todo mundo por
baixo; a faixa é só um atalho visual, não um filtro).

**Indicador de rolagem.** Uma barra fina (`draw_scrollbar`) na borda
direita da grade, só desenhada quando `total_rows > vis_rows` — a
rolagem em si (roda do mouse → `MenuNav::Up`/`Down`, d-pad, page
up/down) já funcionava desde sempre; só faltava qualquer pista visual
de que havia mais linhas abaixo.

Verificado com o binário `selector --shot`, usando a biblioteca real de
ROMs (`~/Documents/SNES Xperience`, sem tocar em `library.json` — só
lido — e com um `nointro.dat` temporário casando o CRC32 real de
`Aladdin.sfc`, removido depois do teste): painel flutuando fora do tubo
com texto nítido e capa/cartucho corretos; painel do Aladdin mostrando
ano/editora/categoria/descrição do DAT de teste; `--filter donkey`
reduzindo "19 games" pra "2 de 19 games" e escondendo a faixa de
recentes; `--filter zzz_no_match` mostrando "0 de 19 games" e "nenhum
jogo encontrado" sem crash; faixa de recentes com as 2 capas jogadas de
verdade, com miniaturas carregadas da pasta de assets real. Build/test
(16 app + 13 domain, incluindo um teste novo de `nointro.rs` e um ajuste
num teste existente que corria risco de condição de corrida entre
threads)/clippy (`-D warnings`)/fmt limpos.

## Revisão (2026-09-17, continuação): ícone/logo novos, versão na plaqueta, checagem de atualização

Pedido do usuário em duas partes: (1) recortar uma arte fornecida (o
controle roxo/cinza com o wordmark "SNES XPERIENCE") pra usar como ícone
do app e logo dentro dele; (2) mostrar a versão do app e do snes9x onde
o nome do app já aparece na TV, checar ao abrir se o snes9x está
desatualizado (avisando com uma modal se estiver) e se há uma versão
nova do app, com uma opção em Configurações pra ligar/desligar essa
checagem automática.

**Recorte da arte.** A imagem colada pelo usuário (2816×1536, fundo
branco) tinha duas partes empilhadas verticalmente com uma faixa em
branco entre elas — usei `numpy`/`PIL` pra achar a caixa delimitadora de
cada uma por linha (onde a linha inteira é branca = fim de um bloco):
o glifo do controle sozinho (quase quadrado, ~690×686) virou o ícone;
controle + wordmark juntos viraram o logo interno. Fundo branco chaveado
pra transparente (limiar suave por "branqueza" do pixel, não um corte
duro) com uma limpeza morfológica (`MinFilter`/`MaxFilter` — erosão
seguida de dilatação) pra tirar um ruído de compressão JPEG que sobrava
como pontinhos escuros isolados no fundo depois do chaveamento ingênuo.

*Ícone do app*: `packaging/icon_1024.png` (mestre, 1024×1024) regenerado
a partir do recorte; `packaging/macos/AppIcon.icns` via `iconutil`
(iconset com os 10 tamanhos padrão), `packaging/windows/AppIcon.ico`
via `Pillow` (multi-tamanho 16..256) e `packaging/linux/snes-xperience.png`
(256×256) — os três já eram referenciados por `build-dmg.sh`/`build.rs`
(Windows)/`snes-xperience.desktop`, então só o conteúdo mudou, nada de
código.

*Logo interno*: aqui a decisão foi diferente da convenção que a fase 4
original estabeleceu ("nenhuma arte embutida, tudo local em `assets/`").
`idle.rs` já carregava `assets/console.png` como logo opcional da tela
idle — mas isso significa que uma instalação nova, sem esse arquivo,
nunca mostra o logo do próprio app, só o fallback em texto. Como o
pedido era "usar essa imagem como logo do app" (não "deixar disponível
pra quem quiser"), embuti o recorte (`crates/app/assets/console_logo.png`,
~360KB, `include_bytes!` em `idle.rs`) como *fallback* — `assets/
console.png` continua tendo prioridade quando existe (mesma regra
"arquivo local vence" que capa/logo/cartucho de jogo já seguem), então
quem já tem seu próprio logo (o usuário mesmo tinha um "SUPER NINTENDO"
de verdade lá) não é afetado; só quem nunca colocou nada passa a ver
algo em vez de texto puro. Verificado nos dois sentidos: com o
`console.png` real do usuário no lugar (mostrou o dele) e com ele
temporariamente movido de lado (mostrou o novo padrão) — restaurado
logo em seguida, `diff` confirmando que nada mais no arquivo mudou.

**Versão na plaqueta.** `draw_brand` (o texto "SNES Xperience" impresso
no rodapé da tela em *todo* contexto — jogo, estante, idle) tinha o
texto fixo na constante `BRAND`. Virou um campo `Cabinet::nameplate`
(inicializado com `BRAND`, texto agora `pub` e reexportado de
`xperience_platform` pra o crate `app` não duplicar a string), com
`Cabinet::set_nameplate` pra sobrescrever — nove pontos de chamada de
`draw_brand` espalhados pelos caminhos `present_frame`/`present_static`/
`frame_2d`/`frame_shelf` (e suas variantes `capture_*`) precisaram do
parâmetro novo; a maioria só precisou de `&self.nameplate` (Rust separa
empréstimos por campo, então `&mut self.canvas` + `&mut self.font` +
`&self.nameplate` ao mesmo tempo compila sem drama), os que já rodavam
dentro de um closure de `with_texture_canvas` (onde `self` já está
emprestado) precisaram extrair `let nameplate = self.nameplate.as_str();`
antes do closure, mesmo padrão que `font`/`images` já usavam ali.

A versão do snes9x vem de `Core::system_version()` — já existia (lê
`retro_get_system_info`), só nunca era chamada fora de uma partida
carregada de verdade. `Core::load` só resolve símbolos e lê essa info
(não chama `retro_init`), então dá pra "espiar" a versão na hora de
montar o texto do nameplate e descartar o `Core` — sem custo real,
sem efeito colateral. Formato final: `"SNES Xperience v0.9.0"` sem
núcleo, `"SNES Xperience v0.9.0 - snes9x 1.63 890b5d4"` com um
instalado (o snes9x embute o hash do commit na própria string de
versão). Recalculado depois de qualquer volta da tela de configurações
(um download de núcleo pode ter acabado de acontecer).

**Checagem de atualização.** Dois checks independentes, ambos
best-effort (nunca travam a abertura do app, nunca viram erro):

- *App*: GET na API do GitHub (`/repos/.../releases/latest`), comparando
  `tag_name` (sem o `v`) contra `env!("CARGO_PKG_VERSION")` — comparação
  numérica por partes (`"0.10.0" > "0.9.0"`), não lexicográfica (que
  erraria "10" < "9"). Testado contra a API de verdade (teste
  `#[ignore]`, roda só sob demanda — `newer_release("0.0.0")` acha a
  release atual, `newer_release("999.0.0")` não acha nada).
- *Núcleo*: só sinalizado desatualizado se o núcleo foi baixado pelo
  próprio botão "Núcleo" das configurações — um núcleo colocado à mão
  não tem base de comparação e nunca é sinalizado (evita falso positivo
  chutando "desatualizado" sem saber a origem do arquivo). `core_update.
  rs` agora grava um sidecar (`core/.core_meta.json`: URL, ETag,
  Content-Length) no momento do download; a checagem faz um `HEAD` na
  mesma URL e compara — ETag primeiro, Content-Length como repescagem se
  o servidor não mandar ETag. Mais simples que comparar datas
  (`Last-Modified`) e não precisa de parser de data nenhum: o buildbot
  troca o ETag/tamanho toda vez que publica um build novo do "latest".

Os dois checks rodam numa thread separada (`std::thread::spawn`, sem
`Mutex`/estado compartilhado — só um `mpsc::Sender` no fim), gated por
`Config::check_updates_on_start` (novo campo, `true` por padrão, opção
"Verificar atualizacoes ao abrir: sim/nao" na tela principal de
configurações, empurrando "Voltar" de índice 4 pra 5). Só manda algo
pelo canal se achou alguma coisa — um app/núcleo em dia não manda nada,
o `Receiver` só fica quieto (mais simples que um enum "tudo certo" que
`idle::run` teria que ignorar do mesmo jeito).

**Modal → tela dedicada.** O pedido dizia "avisar com uma modal", mas a
modal existente (`ModalInfo`/`set_modal`) é pensada pra escolher um
slot dentro de uma partida (save/load/print/cheats) — encaixar um aviso
de texto puro ali seria forçar uma forma que não é a dela. Segui o
padrão que `no_core_screen`/`empty_roms_screen` já usam: uma tela cheia
temporária, plana (`cab.frame_2d`), aparecendo só na tela idle (onde
"ao abrir" naturalmente aterrissa) — `idle::run` ganhou um parâmetro
`notice_rx: &mut Option<Receiver<UpdateNotice>>`; a cada frame do loop
que já existia, dreno o canal com `try_recv()` (mesmo padrão de
`settings.rs` pro download de núcleo); assim que a checagem termina
(pode levar alguns segundos, a tela idle já está de pé há tempo), a
próxima vez que o jogador olhar pra tela ele vê o aviso, sem travar nada
antes disso. Clique ou Confirmar/Voltar dispensa; se dispensado antes de
terminar a checagem (jogador clicou rápido em "Inserir cartucho"), o
aviso simplesmente não aparece nessa sessão — não persegue o jogador
pra outras telas.

Verificado com `--debug-idle-shot`/`--debug-notice-shot app|core|both`
(kinds novos, dev/testing only): captura da tela idle mostrando a
plaqueta com "v0.9.0 - snes9x 1.63 890b5d4" (núcleo de verdade
instalado) e o logo padrão novo (com o `console.png` real do usuário
temporariamente fora do caminho); captura da tela de aviso com as duas
mensagens juntas. `--debug-settings main` confirmando a nova linha e a
contagem de `MAIN_ROWS` batendo. Build/test (18 app, incluindo o teste
`#[ignore]` de rede + 2 novos de config)/clippy (`-D warnings`)/fmt
limpos.

## Revisão (2026-09-17, continuação 2): cartucho maior, LED de power em cima do Ejetar

Pedido pequeno, dois ajustes visuais no painel: aumentar o cartucho e
acrescentar um LED vermelho de power em cima do botão Ejetar, "igual o
console original".

**Cartucho**: `CARTRIDGE_H` (a altura da caixa que `draw_image_absolute`
usa pra encaixar a arte, mantendo proporção) foi de 150 pra 210px nos
dois lugares que desenham cartucho — `draw_panel` (durante o jogo) e
`draw_shelf_panel` (estante, da sessão anterior). Sem mudar mais nada:
o corte (`limit`) que impede o painel de invadir o rodapé do relógio já
existia e continua valendo, só sobra menos espaço embaixo agora.

**LED de power**: a área acima do botão "EJETAR" já era espaço vazio —
os dois rockers (Power/Reset) ocupam a altura cheia do grupo de
switches, mas a caixa do Ejetar só ocupa a parte de baixo dessa altura
(pra alinhar seu rótulo com os dos rockers), sobrando uma faixa livre
em cima dele. É exatamente onde um LED faria sentido, então não precisou
mexer em nenhum layout existente, só desenhar ali.

Como todo o resto da UI é feito de retângulos lisos (botões, switches,
tiles — nenhuma forma redonda em lugar nenhum do código), um círculo de
verdade pediria sua própria malha (`render_geometry`) só por causa de um
detalhezinho desse tamanho. Optei por aproximar com retângulos
empilhados (`draw_led`, novo) — a primeira tentativa (3 fileiras
estreito/largo/estreito) ficou parecendo mais uma cruz do que uma bolinha
numa ampliação de conferência; a segunda (4 fileiras, indo de ~60% pra
~90% da largura e voltando) ficou visivelmente mais redonda, e em
tamanho real (16px, não ampliado) lê bem como "luzinha vermelha". Aceso
(`LED_ON`, vermelho vivo + um pontinho de brilho) segue `panel.powered`
— o mesmo booleano que já controla a posição do rocker de Power e o
"EJETAR"/dimmed; apagado (`LED_OFF`, vermelho bem escuro) é o estado de
repouso, igual o LED de praticamente qualquer console tem quando
desligado.

Verificado com `emu-run --core ... --rom ... --cartridge ... --shot`
(núcleo e ROM reais do usuário, arte de cartucho real de `assets/
cartridge/`) — cartucho visivelmente maior, LED aceso (o modo `--shot`
já liga o console sozinho pra dar pra ver a tela rodando) certo em cima
do Ejetar, com o pontinho de brilho. Estante conferida também
(`selector --filter mario --shot`) com a mesma arte de cartucho maior.
Build/clippy (`-D warnings`)/fmt/test (18 app + 13 domain + 3 platform +
2 ntsc) limpos — nenhum teste novo, mudança só visual.

## Revisão (2026-09-17, continuação 3): listagem geral alfabética com título, painel menos vazio

Pedido do usuário: a listagem geral da estante em ordem alfabética com
um título ("Todos os jogos"), e o painel de dados do jogo continuava
parecendo vazio demais (queixa recorrente mesmo depois do cartucho
maior e das infos do DAT da sessão anterior).

**Ordem alfabética.** `view` (a lista já filtrada pela busca) ganhou um
`sort_by_key(|e| e.title().to_lowercase())` logo depois do filtro,
sempre — não depende mais de `opts.order`/`--order`. Decisão: não
removi `ShelfOpts.order`/`Catalog::list(Order::Shelf)` (ainda usado pra
montar `all`, e a ordenação por recência que ele fazia é redundante
agora que a faixa "jogados recentemente" já cobre esse caso de uso —
mas mexer nisso seria trocar API pública sem necessidade real, já que
nada mais lê a ordem de `all` além da faixa de recentes, que já ordena
por conta própria por `last_played_at`). O efeito prático: a grade
principal agora é sempre A-Z, ponto.

**Título da seção.** Nova constante `ALL_GAMES_LABEL_H` (mesma altura
de `RECENT_LABEL_H`) reserva uma linha pro texto "todos os jogos"
(minúsculo, seguindo a convenção do resto da UI — "jogados
recentemente", "configuracoes" etc. — nunca há acento nem maiúscula
decorativa em texto de tela) desenhado bem onde a grade começava antes;
a grade em si desceu essa altura. Mesmo tratamento visual que "jogados
recentemente" já tinha, só que sempre presente (a faixa de recentes some
com um filtro ativo; este título não, já que a grade em si continua lá).

**Painel menos vazio.** Em vez de mexer só em detalhes do DAT (que a
maioria das bibliotecas não tem), a correção foi puxar dados que *todo*
jogo escaneado já tem, sem precisar de DAT nem de arte local nenhuma:
tamanho do arquivo, há quanto tempo foi adicionado à estante, há quanto
tempo foi jogado (ou "nunca"), e o nome interno do cabeçalho da ROM
quando ele difere do título mostrado (caso comum: título vem do DAT ou
já tem uma tag de região que o nome interno não tem). Novo
`game_info_lines` em `shelf.rs` monta esses quatro (no máximo) pares
label/valor e são concatenados *depois* dos extras do DAT (`nointro_extra`)
no mesmo `ShelfPanelInfo.info` — o DAT continua tendo prioridade visual
(informação "editorial" antes da mecânica), mas agora sempre tem algo
depois dele mesmo sem DAT nenhum carregado.

Datas relativas, não absolutas — reaproveitei o mesmo estilo "hoje"/"ha
1 dia"/"ha N dias" que `settings::core_installed_label` já usa pro
núcleo instalado (`days_ago`, novo, mesma lógica: segundos desde
`UNIX_EPOCH`, divisão inteira por 86400) em vez de trazer uma
dependência de calendário (`chrono`/`time`) só pra formatar uma data —
o projeto não tinha nenhuma delas e a informação relativa ("há 24
dias") é mais legível numa UI pequena do que uma data absoluta de
qualquer forma. Tamanho em MB/KB (`human_size`, novo) também sem
dependência — só uma divisão e um `format!`.

De brinde, a linha "plays" no painel (que só aparecia depois da
primeira partida) agora sempre aparece, "plays 0" incluso — menos um
motivo pro painel parecer incompleto num jogo nunca jogado.

Verificado com `selector --filter mario --shot` (Super Mario World,
com histórico de partidas real) e `selector --shot` sem filtro
(Aladdin, nunca jogado, sem capa) — a grade principal começando em
"ALADDIN" (A-Z) com "todos os jogos" escrito acima dela, "jogados
recentemente" preservada e ordenada por recência como antes, e o
painel do Aladdin mostrando "plays 0"/"tamanho 1.2 MB"/"adicionado ha
24 dias"/"jogado nunca" — bem mais preenchido que só o título e dois
botões. Build/clippy (`-D warnings`)/fmt/test (mesma contagem da
revisão anterior, nenhum teste novo — só lógica de exibição) limpos.

## Revisão (2026-09-17, continuação 4): tempo total de jogo, relógio da sessão só conta com o power ligado

Pedido do usuário: mostrar quanto tempo cada jogo já foi jogado no
total, e o relógio da sessão (mostrado no rodapé do painel durante a
partida) passar a contar só o tempo com o console ligado, não desde a
inserção do cartucho.

**O bug de fato.** `session_start = Instant::now()` era marcado bem no
início de `run_game` e todo `cab.set_session_time(session_start.
elapsed())` media contra ele direto — ou seja, o relógio corria mesmo
com o console desligado (tela de estática), contando um tempo que não
é "sessão jogada" nenhuma. Isso já estava errado antes deste pedido; o
usuário só notou ao pensar em como computar o tempo total certo.

**Relógio pausável.** Duas variáveis novas substituem `session_start`
(removida — nenhum uso restante depois da troca): `powered_elapsed`
(quanto já foi banked de trechos anteriores ligado, nesta sessão) e
`powered_since: Option<Instant>` (quando o trecho atual começou;
`None` enquanto desligado). `live_session_time(elapsed, since)` (nova,
pequena) soma os dois: `elapsed + since.map(|t| t.elapsed())
.unwrap_or_default()`. Nos dois handlers de Power (`UiEvent::Quit`
faz dupla função de desligar/ligar, plan §3.3): desligar faz `powered_
elapsed += powered_since.take().unwrap().elapsed()` (fecha o trecho
atual); ligar faz `powered_since = Some(Instant::now())` (abre um
novo). Os três lugares que chamavam `cab.set_session_time(session_
start.elapsed())` (dois durante o jogo, um na tela de estática enquanto
desligado) viraram `cab.set_session_time(live_session_time(powered_
elapsed, powered_since))` — o terceiro (tela de estática) agora só
redesenha o mesmo valor congelado, em vez de continuar contando.

**Tempo total, persistido por jogo.** Sidecar novo, `save_dir/<titulo>/
playtime.txt` — só o número de segundos, mesmo espírito raso de `cheats.
txt` (texto puro, não JSON, por um valor só). `total_playtime_secs`
(pub) lê (`0` se o arquivo não existe ou está corrompido — nunca vale a
pena falhar o painel por causa de um número); `add_playtime` (privada)
soma e regrava, só chamada uma vez, no fim de `run_game`, depois do
flush final de SRAM — fecha qualquer trecho ligado ainda aberto
(`powered_since.take()`) antes de somar, cobrindo até a saída "suja"
(janela fechada com o console ligado). Um `secs == 0` (sessão inteira
sem ligar o console nem uma vez) pula a escrita — não cria pasta nem
arquivo à toa.

**Nome de pasta, não título da estante.** Achado importante ao ligar
os dois lados (`run_game` escreve, a estante lê): a pasta por jogo é
nomeada pelo *stem do arquivo da ROM* (`title = spec.rom.file_stem()`),
não pelo `CatalogEntry::title()` que a estante já usa (que prefere
nome do DAT > nome interno > nome do arquivo). Um jogo com DAT
carregado teria nomes diferentes dos dois lados — a estante procuraria
`saves/Nome Canonico Do DAT/playtime.txt` enquanto `run_game` escreveu
em `saves/Nome Do Arquivo/playtime.txt`. Corrigido extraindo esse
cálculo pra uma função `pub fn rom_title(rom_path: &Path) -> String`
(só o que já existia inline, sem mudar o comportamento de `run_game`)
que a estante agora chama com o caminho do arquivo (`entry.rom.path`)
antes de ler o sidecar — garantindo que os dois lados olham pra
exatamente a mesma pasta.

**No painel.** `game_info_lines` (da revisão anterior) ganhou um
parâmetro `playtime_secs` e uma linha nova, "tempo total"
(`format_playtime`, novo: "nunca" pra zero, "Xmin" abaixo de uma hora,
"Xh Ymin" acima — sem trazer dependência de data/duração só pra isso,
mesma escolha da revisão anterior com `days_ago`/`human_size`). O
ponto de montagem do painel na estante (`shelf.rs`) agora calcula
`rom_title(&entry.rom.path)` e lê `total_playtime_secs(&dirs::saves_
dir(), &esse_titulo)` a cada quadro pro jogo focado — uma leitura de
arquivo pequena, 60x/s, no mesmo espírito "sem thread, é barato o
suficiente" que a busca de capa/logo já segue.

Verificado com `emu-run --core ... --rom ... --save-dir <pasta de
teste> --shot ... --shot-frame 90` (Super Mario World, núcleo real) —
duas rodadas seguidas confirmando acumulação real (`playtime.txt`
"1" depois "2" segundos, ~1.5s reais por rodada a 60fps); e com um
`playtime.txt` semeado manualmente (5430s) numa pasta de save que não
existia ainda de um jogo real (Killer Instinct, sem save nenhum antes),
`selector --filter killer --shot` mostrando "tempo total: 1h 30min" no
painel — removido depois do teste (`rm -rf` só daquela pasta, as três
pastas de save reais do usuário confirmadas intactas antes e depois).
Build/test (22 app, 3 novos: acumulação, corte em zero segundos, stem
correto)/clippy (`-D warnings`)/fmt limpos.

## Revisão (2026-09-17, continuação 5): remover plays/adicionado, data de lançamento, botão de histórico

Pedido do usuário, três partes: remover "plays" (contagem de partidas)
e "adicionado" do painel; colocar a data de lançamento no lugar de
"plays"; e um botão de histórico com os jogos mais jogados de todo o
período.

**Remoção.** `game_info_lines` perdeu a linha "adicionado"
(`days_ago(entry.rom.added_at)`) — `days_ago` continua existindo,
ainda usada por "jogado" (`last_played_at`), então não sobrou código
morto. O bloco "plays" do painel (`draw_shelf_panel`, desenho dedicado
de uma linha, não fazia parte do loop genérico de `info`) foi
inteiramente reaproveitado pra um bloco "lancamento" — mesma posição,
mesmo estilo (label cinza + valor branco na mesma linha), só trocando a
fonte do dado.

**Data de lançamento.** Não existe fonte nova nenhuma pra isso — o
No-Intro DAT já carregava um `<year>` por jogo desde a sessão passada
(virava a linha "ano" na lista genérica de extras do DAT). A mudança
foi só de *onde* esse dado aparece: `shelf.rs` agora procura a entrada
`"ano"` dentro de `nointro_extra` (`Vec<(String,String)>`), remove ela
dali com `Vec::remove` (pra não aparecer duplicada na lista genérica) e
usa o valor como `ShelfPanelInfo.release: Option<String>` — campo novo,
substituindo `play_count: u32` que a struct tinha antes (removido de
verdade, não só parou de ser lido — nada mais no código precisava
dele). Sem DAT carregado, ou um DAT sem `<year>` pra aquele jogo,
`release` é `None` e a linha simplesmente não aparece — mesma regra
condicional que já valia pros outros campos do DAT.

**Botão "Historico".** Decisão de posicionamento: em vez de acrescentar
um terceiro botão fixo ao painel plano (que já tem Voltar/Configuracoes
via `ShelfButton`/`Cabinet::hit_shelf_button`), o botão foi desenhado
*dentro do tubo*, no cabeçalho da estante, à esquerda da caixa de busca
— mesmo tratamento visual dela (`draw_history_button`, novo, mesmo
"outline + fill + texto" que `draw_filter_box` já usa), testado por
clique com `cab.hit_screen_point` + `in_rect`, do mesmo jeito que a
caixa de busca e as faixas de jogos já são. Motivo: adicionar um botão
permanente ao painel plano (`draw_shelf_panel`, em `cabinet.rs`)
significaria ele aparecer em *toda* tela que usa esse painel — inclusive
dentro da própria tela de histórico, sem fazer sentido nenhum lá dentro
(clicar "Historico" estando já no histórico). Manter o botão como
conteúdo da estante em si evita esse problema de vez, sem precisar de
lógica condicional nova em `cabinet.rs`.

**A tela em si** (`run_history`, `capture_history_preview`, novas em
`shelf.rs`) é uma lista simples e rolável — sem grade, sem capas, só
texto: `ranked_by_playtime` escaneia o catálogo inteiro (`Order::Name`,
a ordem de entrada não importa, é reordenado na sequência), calcula
`total_playtime_secs` pra cada jogo (mesma função da revisão anterior)
e descarta quem nunca foi ligado (`filter(|(_, secs)| *secs > 0)`) —
"mais jogados" não faz sentido incluir quem tem zero. Ordenado por
`Reverse(secs)`. Cada linha mostra o título à esquerda e o tempo
formatado à direita (`format_playtime`, reaproveitado sem mudança);
clicar numa linha já selecionada lança o jogo direto (`pick_play`,
a mesma função que a estante principal usa) — mesmo padrão de
"clique pra selecionar, clique de novo pra confirmar" que a grade
principal já ensina.

O ponto mais delicado foi o que "Voltar" significa em cada tela: o
enum `Pick` já é usado pela estante principal, onde `Pick::Back` quer
dizer "sai da estante, volta pra tela idle" (`xperience.rs`'s `'app:
loop`). Reaproveitar o mesmo enum pro histórico faria um "Voltar" de lá
significar a mesma coisa por engano — pularia a estante inteira,
voltando direto pra idle. Resolvido em `xperience.rs`: o novo braço
`Pick::History` (dentro do `match` que já trata `Pick::Settings` do
jeito parecido) chama `shelf::run_history` e trata o `Pick` *que ele
devolve* com seu próprio sub-`match` local — `Pick::Back` (do
histórico) vira só `continue` do laço `'shelf`, ou seja, "reabrir a
estante", nunca "sair pra idle"; `Pick::Play` dali flui pro carregamento
do jogo normalmente; `Pick::Settings` dali abre configurações e volta
pra estante depois. Nenhum tipo novo foi necessário, só esse
roteamento local — o mesmo truque que já existia pra `Pick::Settings`
(que também faz uma ação colateral e depois `continue`s o laço da
estante) — só que agora aninhado mais um nível.

**Nome de pasta, de novo.** `run_history`/`capture_history_preview`
usam `runner::rom_title`/`runner::total_playtime_secs` exatamente do
jeito que o painel principal já usa (revisão anterior) — sem isso, o
histórico ranquearia por um tempo que não bate com o que o painel
mostra pro mesmo jogo.

Verificado com `selector --filter mario --shot` (painel sem "plays"/
"adicionado", com "tempo total" e sem "lancamento" — sem DAT carregado
nesse teste); um `nointro.dat` temporário pro Aladdin (mesmo esquema de
testes da sessão passada, removido depois) confirmando "lancamento
1993" no topo do painel e "ano" *não* duplicado na lista de extras do
DAT; e `selector --debug-history-shot --shot` (kind novo, dev/testing
only) com dois `playtime.txt` semeados manualmente em jogos sem save
nenhum antes (Killer Instinct, Top Gear) — a lista mostrando os dois
mais um terceiro jogo com tempo real de verdade que o usuário já tinha
acumulado testando o relógio da sessão na revisão anterior (Street
Fighter II Turbo, 5 segundos — dado real do usuário, não tocado),
ordenados corretamente do maior tempo pro menor. Pastas de teste
removidas depois (`rm -rf` só nas duas criadas pelo teste), as três
pastas de save reais do usuário confirmadas intactas antes e depois.
Build/clippy (`-D warnings`)/fmt/test (22 app, mesma contagem — nenhum
teste novo, mudança de exibição/navegação) limpos.

## Revisão (2026-09-17, continuação 6): acentuação e ortografia em toda a UI

Pedido do usuário: corrigir falta de acentuação e erros de ortografia
nos textos do app. Toda a UI (desde a fase 3) vinha sendo escrita sem
nenhum acento — convenção que se instalou cedo no projeto e nunca foi
questionada até agora.

**Por que dava pra fazer isso de verdade.** Antes de sair trocando
strings, confirmei que a fonte suporta os caracteres necessários:
`GLYPH_FIRST`/`GLYPH_LAST` cobrem `0x20..=0xFF` (Latin-1 Supplement),
que inclui todas as vogais acentuadas e cedilhas do português (á à â ã
é ê í ó ô õ ú ç, maiúsculas inclusas). Não era uma limitação técnica —
só nunca tinha sido usado.

**O bug real que apareceu no caminho.** As três implementações de
quebra de linha (`Screen::text_wrapped`, `draw_text_wrapped_absolute`,
a `wrapped_height` livre) mediam largura com `word.len()` — bytes, não
caracteres. Um acento ocupa 2 bytes em UTF-8 mas é 1 caractere visível
na fonte (largura fixa por glifo); a conta de "cabe nessa coluna?"
subestimaria o espaço restante sempre que uma palavra acentuada
entrasse. Pior: duas das três (`text_wrapped`/`draw_text_wrapped_
absolute`) usavam `line.split_at(cols)` pra cortar uma linha longa
demais no meio — `split_at` em `String` é indexado por *byte*, então um
corte que caísse no meio dos 2 bytes de um acento causaria panic em
tempo de execução ("byte index is not a char boundary"), derrubando o
processo inteiro. Isso não tinha como se manifestar antes: com zero
acentos em qualquer string da UI, todo texto era ASCII puro, onde
byte e caractere sempre coincidem. Adicionar o primeiro acento a
qualquer texto que passa por quebra de linha (uma descrição de DAT
longa, por exemplo) teria achado esse bug de um jeito bem pior — um
crash em produção. Corrigido nas três funções: contagem por
`.chars().count()` em vez de `.len()`, corte por `.chars().take(n)/
.skip(n).collect()` em vez de `split_at` — testado de propósito com uma
`<description>` de DAT longa e acentuada (`selector --shot`, sem
crash, quebra de linha certa, nenhuma palavra cortada ao meio).

**O texto em si.** Passei arquivo por arquivo (`cabinet.rs`, `shelf.rs`,
`settings.rs`, `idle.rs`, `runner.rs`, `xperience.rs`, e o gerador de
infos do DAT em `catalog.rs`), separando strings realmente desenhadas
na tela (via `d.text`/`draw_button`/`draw_text_wrapped_absolute`/etc.)
de comentários de documentação (que às vezes citam o pedido original do
usuário entre aspas — esses eu deixei como estavam, são um registro
histórico do que foi pedido, não texto do app). Alguns exemplos do que
mudou: "Configuracoes" → "Configurações", "Nucleo" → "Núcleo", "sessao"
→ "sessão" (rótulo do relógio da sessão no painel — esse precisou do
ajuste de `.chars().count()` acima também, já que o cálculo de onde
desenhar o valor ao lado do rótulo usava o comprimento do próprio
rótulo), "lancamento" → "lançamento", "anotacoes"/"anotacao" →
"anotações"/"anotação", "proxima" → "próxima", "historico" →
"histórico", "executavel" → "executável", "opcao"/"acao" →
"opção"/"ação", "atualizacao(oes)" → "atualização(ões)", "disponivel"
→ "disponível", "desricao" (label do DAT) → "descrição", entre outras.
Deixados de propósito sem tradução/acento: termos técnicos/nomes de
botão em inglês já estabelecidos (POWER, RESET, Cheats, Printscreen,
"print" como sinônimo de captura de tela) — trocar esses seria mudança
de vocabulário, não correção ortográfica.

Verificado com capturas de tela de cada área afetada: `--debug-settings
main` (título "configurações", linha "Núcleo: ... instalado há N dias",
opção "Verificar atualizações ao abrir", texto de ajuda no rodapé — tudo
renderizando com acento, sem glifo quebrado nem desalinhamento);
`emu-run --shot` durante uma partida real (rótulo "sessão 00:00"
alinhado certinho com o valor ao lado, "Anotações" no botão do
caderno); e o `nointro.dat` de teste com descrição longa e acentuada já
citado acima, confirmando tanto a exibição quanto a ausência de crash.
Build/clippy (`-D warnings`)/fmt/test (mesma contagem — mudança de
conteúdo de texto e de uma função de wrap, sem lógica nova) limpos.

## Revisão (2026-09-17, continuação 7): renomear ROMs para o padrão No-Intro

Pedido do usuário (depois de perguntar sobre fontes de DAT mais
completas — respondido em chat, sem mudança de código: No-Intro em si
não carrega metadados além de nome+hash por design; `libretro-database`
tem uma pasta `metadat/` com atributos separados por arquivo, formato
diferente do que já parseio; TOSEC usa o mesmo formato Logiqx que já
suporto, `<year>`/`<publisher>`/`<description>` incluídos): um jeito de
renomear as ROMs pro nome canônico do No-Intro automaticamente.

Duas perguntas de design que só o usuário podia responder, feitas antes
de escrever qualquer código: (1) renomear sozinho a cada escaneamento,
ou só por um botão manual em Configurações? (2) o que fazer com
save/nota/cheat/tempo já gravados pra uma ROM que vai ser renomeada?
Escolhido: botão manual, e mover a pasta correspondente junto (não
perder nada).

**O problema real por trás da pergunta 2.** `saves/<título>/`,
`notes/<título>/` e `assets/{cover,logo,cartridge}/<título>.*` são
nomeados pelo *nome do arquivo da ROM*, não pelo hash — puro acaso de
como o app foi projetado desde a fase 4 original (legibilidade >
imunidade a rename, decisão já tomada antes). Trocar só o nome do
arquivo `.sfc` sem mexer nessas pastas orfanaria qualquer progresso já
gravado. `library.json` (play count, `last_played_at`, data de
adição) não tem esse problema — é chaveado por SHA1, que não muda com
o rename.

**Módulo novo, `rom_rename.rs`.** `rename_to_nointro(roms_dir, dat_path,
saves_dir, notes_dir, assets_dir)`: usa `xperience_domain::library::scan`
(a mesma função que o catálogo já usa) pra listar as ROMs já com
CRC32 calculado, casa cada uma contra o DAT carregado
(`NoIntroDat::lookup`), e pula quem já está com o nome certo (`old_stem
== new_stem`, comparando o stem do arquivo contra o nome canônico já
sanitizado). Pra cada uma que precisa de rename: `fs::rename` no
próprio arquivo, depois `move_entry` (nova, pequena) pra cada pasta/
arquivo dependente — `saves/`/`notes/` (pasta inteira, mesmo nome) e
`assets/{cover,logo,cartridge}/` (um arquivo por extensão testada:
png/jpg/jpeg). Tudo best-effort: uma falha de permissão ou um arquivo
de destino já existente (evita sobrescrever por engano) só pula aquele
item e loga, não aborta o lote inteiro.

**Reaproveitado, não duplicado.** `sanitize_dir_name` (a função que já
existia em `runner.rs`, tirando caracteres hostis a caminho de arquivo
de um título) virou `pub(crate)` — é a mesma sanitização que a pasta
`saves/<título>/` já recebe (`game_dir` chama ela por dentro), então
usar ela aqui garante que o nome novo do arquivo bate exatamente com o
nome de pasta que `saves_dir`/`notes_dir` vão procurar depois. Isso
importa porque tem uma sutileza de nomenclatura: a pasta antiga de save
é `sanitize_dir_name(nome_do_arquivo_antigo)` (não o nome do arquivo
cru), enquanto a arte local (`assets/`) usa o *stem cru*, sem
sanitizar (mesma convenção que `shelf.rs::find_local_art` já segue) —
então o código busca a pasta antiga de save/nota por uma chave e o
arquivo antigo de arte por outra, cada uma espelhando exatamente como o
resto do app já monta esses caminhos.

**Na tela de configurações.** Nova linha, "Renomear ROMs para o padrão
No-Intro", entre "Verificar atualizações..." e "Voltar" (`MAIN_ROWS`
6 → 7). Roda síncrono no clique — são só renomeações de arquivo local,
rápido mesmo pra uma biblioteca de algumas centenas de ROMs, sem
justificar o padrão thread+canal que o download do núcleo usa (que
existe por causa de latência de rede, não presente aqui). O resultado
vira o próprio rótulo da linha até o próximo clique ("N roms
renomeadas" / "nenhuma precisava de nome novo" / "nointro.dat não
encontrado") — mais simples que inventar uma barra de progresso pra
algo que termina em milissegundos.

Verificado com três testes novos em `rom_rename.rs` (ROM sintética
gerada em memória, ida completa: renomeia o arquivo, move a pasta de
save com um `sram.srm` dentro e a capa em `assets/cover/`, confirma que
os três apontam pro nome novo depois; um segundo teste confirmando que
um nome já canônico não sofre nenhuma escrita; um terceiro confirmando
que a ausência de `nointro.dat` é reportada, não um crash) e uma
captura de `--debug-settings main` mostrando a linha nova entre as
outras. Não testei contra a biblioteca real do usuário — é uma
operação que renomeia arquivos de verdade dele, então o teste de
ponta a ponta fica pra quando ele mesmo clicar o botão. Build/clippy
(`-D warnings`)/fmt/test (24 app, 3 novos) limpos.

## Revisão (2026-09-17, continuação 8): ano/editora do TOSEC embutidos no app

Pedido do usuário: "se for TOSEC tem como baixar o que está lá e embutir
no app?" — seguindo direto da revisão anterior, que citava o TOSEC como
uma possível fonte de metadados mais completa. Antes de escrever
qualquer código, baixei de verdade o pacote "DAT Pack - Complete" do
TOSEC (achando a URL direta navegando o site, não adivinhando) e abri um
dat real de SNES pra conferir — e a premissa da revisão anterior estava
**errada**: TOSEC não tem `<year>`/`<publisher>` como atributos XML
próprios, só `<description>`, que é sempre idêntica ao `name` do jogo.
Ano e editora vivem só dentro do próprio nome catalogado, codificados
pela convenção de nomenclatura do TOSEC — ex.
`"Chrono Trigger (1995)(Square)(US)[tr de]"`. Reportei isso ao usuário
com um trecho real do XML antes de seguir, em vez de embutir algo que
não entregaria o que foi pedido.

Perguntado de volta como proceder (`AskUserQuestion`, 4 opções), o
usuário escolheu extrair ano/editora do nome via a convenção do TOSEC
— não baixar um dat de outro projeto nem tentar mudar de formato.

**Geração offline, não runtime.** `scripts/gen_tosec_data.py` (novo,
versionado — o zip de ~96MB e o dat de ~1.3MB de origem não são, só o
script e a saída) parseia o dat real com `xml.etree.ElementTree`, extrai
ano/editora de cada `name` com uma regex sobre grupos `(...)` (primeiro
grupo que bate num ano de 4 dígitos; o grupo seguinte é a editora — não
trava no primeiro grupo entre parênteses porque um bom tanto dos nomes
começa com `(demo)` ou similar antes do ano) e escreve
`crates/domain/src/tosec_data.txt`, TSV simples (`CRC32\tano\teditora`).
Rodado uma vez contra o dat SNES "Games" do TOSEC: 3893 jogos batidos,
6 pulados (sem ano reconhecível no nome — prototipos/homebrew sem data),
92KB de saída. Só o fato bruto (ano, nome de editora) é guardado — nunca
o `name`/`description` catalogado do TOSEC em si.

**Módulo novo, `tosec.rs`.** Mesmo padrão já usado por `cheats.rs`:
`include_str!` do `.txt` gerado, index `HashMap<&str, TosecInfo>`
construído uma vez via `OnceLock`, `lookup(crc32) -> Option<&TosecInfo>`.
Sem parsing da convenção de nomenclatura em tempo de execução — só leitura
de TSV.

**Merge com o No-Intro DAT em `catalog.rs`.** `nointro_info_lines`
virou `info_lines(nointro: Option<&NoIntroGameInfo>, crc32: &str)`:
ano/editora vêm do DAT do usuário quando presentes, senão caem pro
`tosec::lookup` embutido; categoria/descrição continuam vindo só do DAT
(TOSEC não tem nenhum dos dois de verdade). Assim uma instalação nova,
sem nenhum DAT configurado, já mostra ano/editora pra quase qualquer ROM
de SNES — não mais um painel vazio até o usuário ir atrás de um DAT.

**Licenciamento**, resolvido com uma postura conservadora: diferente da
base de cheats (`libretro-database`, CC BY-SA 4.0 explícito), o TOSEC
não publica uma licença clara pro conteúdo dos seus datfiles — o GPL
mencionado no site é da ferramenta de CMS (Joomla), não dos dados. Por
isso só os dois fatos nus (ano, nome de editora) são embutidos, nunca o
nome/descrição catalogado do TOSEC — o mesmo raciocínio usado pra não
embutir uma lista telefônica inteira sem tratamento. Documentado em
`THIRD-PARTY-NOTICES.md`.

Verificado com 3 testes novos em `tosec.rs` (retorno `None` pra CRC32
desconhecido; sanidade do arquivo embutido — mais de mil entradas,
chaves de 8 hex maiúsculos, pelo menos uma com ano e editora — sem
travar num jogo específico, pra sobreviver a uma regeneração futura
contra uma versão mais nova do TOSEC) e 3 novos em `catalog.rs` (ano
1995/editora Square do TOSEC quando não há DAT; o DAT vence quando os
dois têm a mesma informação — testado com um ano diferente do DAT
sobrepondo o do TOSEC; e nenhum dos dois retorna nada pra um CRC32 que
nenhuma fonte conhece), todos usando o CRC32 real de Chrono Trigger
(US) como referência estável. Confirmado visualmente contra a
biblioteca real do usuário (`~/Documents/SNES Xperience`, sem
`nointro.dat` presente): `selector --filter Aladdin --frames 3 --shot`
mostra "lançamento 1993" / "editora Capcom" no painel, vindo só do
TOSEC embutido. Build/clippy (`-D warnings`)/fmt/test (18 domain, up de
15) limpos; arquivos temporários do download (~96MB de zip + extração)
apagados do scratchpad depois de gerar o `.txt` final.

## Revisão (2026-09-17, continuação 9): dat oficial do No-Intro + cobertura ampliada do TOSEC

Usuário reportou que "Street Fighter II Turbo" não mostrava
lançamento/editora, apesar do embutido do TOSEC cobrir a maioria da
biblioteca. Investigado calculando o CRC32 real do arquivo do usuário
(`D43BC5A3`, mesma lógica de `RomId::from_bytes` replicada em Python) e
procurando ele no dat SNES do TOSEC — não estava lá. O TOSEC cataloga
"Street Fighter II Turbo" só como "... - Hyper Fighting", em várias
variantes (US/JP/EU/betas/hacks), nenhuma com esse hash exato: o dump do
usuário é uma revisão (acabou confirmado depois: "(Rev 1)") que o TOSEC
não tem catalogada sob esse CRC32 específico, embora conheça o jogo.

Pedido seguinte do usuário: baixar um dat oficial do No-Intro pra ele, e
"somar os dois" (TOSEC + No-Intro) num arquivo próprio, integrado ao
app. O No-Intro não tem link direto de download (Parte 4 do plano
original já registrava isso) — o dat sai do DAT-o-MATIC
(`datomatic.no-intro.org`) por um fluxo de cliques (tela de
confirmação → geração de um token de uso único → download), automatizado
aqui via o navegador embutido clicando os botões reais (uma tentativa via
`fetch()` direto no token falhou — cada requisição gera um token novo,
de uso único, então só o clique de verdade no botão visível completa o
fluxo). O arquivo baixado (`Nintendo - Super Nintendo Entertainment
System (20260913-204915).dat`, 4129 jogos) foi conferido: tem a entrada
exata `"Street Fighter II Turbo (USA) (Rev 1)"`, CRC32 `d43bc5a3` — bate
com o arquivo do usuário.

**Duas coisas feitas com esse dat, nenhuma delas embute o dat em si no
binário:**

1. **Instalado como o `nointro.dat` de verdade do usuário**
   (`~/Documents/SNES Xperience/nointro.dat`) — a funcionalidade de DAT
   opcional já existia (plano original, Parte 4) mas nunca tinha um dat
   de verdade carregado; agora "Renomear ROMs pro padrão No-Intro" (da
   revisão 7) também passa a funcionar de verdade, e o painel ganha
   `categoria`/`descrição` pra qualquer jogo que o dat conhecer.

2. **`gen_tosec_data.py` ganhou um terceiro argumento opcional**: o
   caminho do dat do No-Intro. Quando presente, o script lê só a relação
   `id`/`cloneofid` de cada `<game>` (que hashes são revisões/regiões do
   mesmo jogo) — nenhum nome ou texto do No-Intro — e propaga o
   ano/editora que o TOSEC já extraiu pra um membro da família pros
   outros membros que o TOSEC não catalogou sozinho. Resultado: 578
   CRC32 novos cobertos (3893 → 4471 no `tosec_data.txt`), incluindo
   agora `D43BC5A3` → `1993`/`Capcom`, herdado da entrada TOSEC da
   revisão original que ele já conhecia.

Postura de licenciamento mantida igual à do TOSEC: como o No-Intro
também não publica uma licença explícita pro conteúdo dos seus dats
(procurado no DAT-o-MATIC, no site principal e na wiki — nada
encontrado além do aviso padrão antipirataria), só a *relação* entre
hashes é usada pra decidir cobertura, nunca o nome/descrição do
No-Intro — documentado em `THIRD-PARTY-NOTICES.md` junto da nota do
TOSEC.

Verificado: `grep D43BC5A3 tosec_data.txt` → `1993 Capcom`; rebuild +
`cargo test --workspace` (18 domain, mesmos de antes, nenhum novo teste
precisava pinar nesse CRC32 específico já que a lógica de propagação
mora só no script gerador, não em código Rust) limpos; captura headless
(`selector --filter "Street Fighter" --shot`) contra a biblioteca real
do usuário confirma "lançamento 1993" / "editora Capcom" / "categoria
Games" / "descrição Street Fighter II Turbo (USA) (Rev 1)" no painel
(os dois últimos vindo do `nointro.dat` recém-instalado, os dois
primeiros do TOSEC ampliado). Arquivos temporários (dat/zip baixados,
cookies, HTML intermediário) apagados do scratchpad e do `~/Downloads`
do usuário (onde o navegador embutido salva downloads reais) depois de
copiar o que interessava pros lugares certos.

## Revisão (2026-09-17, continuação 10): painel sem categoria/descrição/nome interno

Pedido direto do usuário, imediatamente depois de ver o painel do
Street Fighter II Turbo com o `nointro.dat` recém-instalado mostrando
"categoria: Games" e "descrição: Street Fighter II Turbo (USA) (Rev
1)" — informação redundante/técnica demais pra quem só quer ver se vale
jogar. `info_lines` (`catalog.rs`) perdeu o bloco que extraía
`category`/`description` do `NoIntroGameInfo` (os campos continuam
existindo na struct e sendo parseados do DAT — só pararam de virar
linha do painel); `game_info_lines` (`shelf.rs`) perdeu o bloco condicional
de "nome interno" (cabeçalho da própria ROM).

Efeito colateral bem-vindo: como o painel tem altura fixa e corta linhas
que não cabem (`draw_shelf_panel`'s `cy + GLYPH_H <= limit`), tirar essas
três linhas devolveu espaço pra "jogado"/"tempo total" reaparecerem em
jogos com DAT carregado — antes, um jogo com categoria+descrição do DAT
enchia o painel e empurrava essas duas pra fora do limite visível.

Verificado com `selector --filter "Street Fighter" --shot` contra a
biblioteca real do usuário (com `nointro.dat` carregado): painel agora
mostra só lançamento/editora/tamanho/jogado/tempo total — nem
categoria, nem descrição, nem nome interno. Build/clippy
(`-D warnings`)/fmt/test (18 domain) limpos.

## Revisão (2026-09-18): tela cheia por padrão, botão de fechar, animação de cartucho

Três pedidos numa tarada só: (1) abrir em tela cheia por padrão (com a
opção de desligar em Configurações, que já existia); (2) um botão de
fechar o app no canto superior esquerdo; (3) uma animação pra inserir/
ejetar o cartucho.

**Tela cheia por padrão.** Só o valor default de `Config::load` mudou
(`fullscreen: false` → `true`) — o toggle em Configurações e o próprio
formato do `xperience.cfg` continuam idênticos. Só afeta uma instalação
nova (sem `xperience.cfg` ainda): o arquivo do usuário já tinha
`fullscreen = true` gravado de um teste anterior, então nada mudou pra
ele nesta revisão especificamente, mas qualquer instalação futura (ou
uma pasta de app nova) já abre em tela cheia sem precisar entrar em
Configurações primeiro.

**Botão de fechar, canto superior esquerdo.** Fazia falta principalmente
*por causa* do item 1 — em tela cheia não tem barra de título do sistema
pra fechar a janela. Implementado como chrome global do `Cabinet`, não
por tela: um campo `show_close: bool` (novo, `Cabinet::set_close_button`)
que cada tela liga/desliga explicitamente ao entrar — idle, estante
(grade/filtro/histórico) e configurações ligam; `run_game` desliga
(nunca herdado de uma tela anterior, já que o `Cabinet` é reaproveitado
o jogo inteiro). De propósito ausente durante a partida: Desligar/Ejetar
já são o jeito de sair de um jogo, e um botão de "fechar tudo" ali só
aumentaria o risco de um clique errado derrubar o app inteiro em vez de
só pausar/desligar.

Desenhado como um quadrado "X" reaproveitando `draw_button` (mesma
linguagem visual dos outros botões), sempre a 14px do canto do
`canvas_rect` — o retângulo onde o "gabinete" de fato desenha, já
ajustado pro letterbox de uma tela ultrawide (`window_to_output` já
subtrai esse offset antes de qualquer hit-test, então o botão acerta o
alvo certo em qualquer proporção de tela). Desenhado por último em cada
`present_*`/`capture_*` que uma tela sem jogo usa (`present_static`,
`composite_screen` — que `composite_shelf` já encadeia —,
`frame_shelf_fade_in`, e as variantes offscreen `capture_2d`/
`capture_shelf`/`capture_static_bmp`, pras capturas de `--shot`
combinarem com a janela de verdade), sempre atrás do próprio flag —
`present_frame`/`present_pause`/`present_modal` (só usados durante o
jogo) nunca ganharam a chamada, então gameplay não precisa de nenhuma
lógica extra pra não mostrar o botão.

**Animação de inserir/ejetar cartucho.** A arte de cartucho do painel
(quando o jogo tem uma local, `assets/cartridge/<rom>.*`) já existia
como imagem estática — a animação é um "wipe" de cima pra baixo por
cima dela, não um objeto 3D nem um asset novo. `PanelInfo` ganhou
`cartridge_reveal: f32` (1.0 default, só a animação mexe nele via
`Cabinet::set_cartridge_reveal`); `draw_image_absolute` virou uma casca
fina sobre uma nova `draw_image_absolute_revealed`, que recorta a
textura de *origem* (não a área de destino) às `reveal` primeiras linhas
— crop na origem em vez de `set_clip_rect` compartilhado do canvas
porque já tem um viewport ativo nesse ponto do desenho, e mexer no clip
rect ali arriscaria vazar pro próximo draw call sem eu perceber. A conta
do recorte (`reveal_rects`) foi puxada pra função pura, testável sem um
canvas/textura de verdade — 3 testes novos (reveal 1.0 = imagem inteira,
reveal 0.5 = metade de cima só, um `reveal` bem pequeno não gera um rect
de altura zero que o SDL rejeitaria).

`runner.rs` ganhou `cartridge_insert_animation`/`cartridge_eject_animation`,
no mesmo estilo de `power_on_burst`/`power_off_burst`/`eject_clunk` que já
existiam (loop bloqueante de ~320ms, `present_static` a cada quadro,
ruído sintetizado on-the-fly, sem asset de áudio) — `eject_clunk` virou
uma casca fina sobre uma nova `tone_click(plat, freq)` compartilhada, já
que as duas animações também precisavam do mesmo "clique" mecânico (só
com frequências diferentes: 180Hz "encaixou", 130Hz "soltou", 90Hz o
"resiste" que já existia). Chamada de inserção logo depois de
`cab.set_panel` (onde a cartucho passa de "nada" pra "instalado, off,
esperando Ligar" — mesmo ponto que já existia antes desta revisão),
pulada com `spec.shot.is_none()` (mesmo raciocínio de sempre: um
`--shot` não tem clique nenhum que dispararia isso) e só quando existe
arte de cartucho pra revelar. Chamada de ejeção dentro do próprio
`UiEvent::Eject` (o branch que já existia pra "console desligado,
ejetar de verdade"), antes de sair do loop com `GameExit::Ejected`.

Verificado: build/clippy (`-D warnings`)/fmt/test (24 platform, 3 novos)
limpos; capturas headless (`--shot`) do botão de fechar aparecendo na
estante e em configurações, contra a biblioteca real do usuário. A
animação em si (tempo real, ~320ms, áudio incluso) não dá pra capturar
num BMP de um quadro só — verificada por revisão de código e por seguir
exatamente o mesmo padrão de `power_on_burst`/`power_off_burst`, já
comprovado em produção nesta mesma tela; uma checagem ao vivo com
computer-use não foi possível sem reimplantar o app instalado do
usuário fora do fluxo de "atualiza no meu mac" que ele mesmo controla.

## Revisão (2026-09-18, continuação): back cover no painel + tentativa (abandonada) de auto-baixar o No-Intro

Usuário perguntou se o `nointro.dat` está embutido no app e se podia
apagar o arquivo da pasta. Resposta: não, só o TOSEC (fatos soltos,
ano/editora) está embutido; o `nointro.dat` é o arquivo que eu mesmo
baixei e instalei numa revisão anterior — apagar funciona, mas perde
nome canônico e o botão de renomear (a correção específica do Street
Fighter II Turbo continua valendo de qualquer jeito, já que foi
propagada pro `tosec_data.txt` embutido).

Perguntado se dava pra "configurar isso também pra ficar dentro do
app" (embutir os nomes do No-Intro também, não só usar como opcional).
Expliquei a diferença de risco: TOSEC só teve dois *fatos* extraídos,
nunca o nome catalogado; o No-Intro é justamente o nome canônico em si
— o produto curado do projeto, não um fato incidental. Perguntado via
`AskUserQuestion` (3 opções: baixar sozinho no 1º uso / embutir no
binário / deixar como está), o usuário escolheu "baixar sozinho no 1º
uso", no mesmo espírito do botão que já baixa o núcleo snes9x do
buildbot.

**Investigação técnica antes de escrever qualquer código Rust:** testei
via `curl` (com cookie jar, User-Agent de navegador) a mesma sequência
de 3 passos que já tinha mapeado manualmente pelo navegador (formulário
de customização → disclaimer com token de uso único → página "manager"
com um segundo token que finalmente serve o zip). Resultado: o
DAT-o-MATIC **baniu meu acesso** na primeira tentativa —
"Something went wrong with your client... The ban won't be lifted
until you contact me" — claramente uma proteção anti-bot que reconhece
esse padrão de acesso direto (pular pra URL de download sem navegar
pelas telas antes) como abusivo. Automatizar isso dentro do app
arriscaria banir o IP do próprio usuário do site do No-Intro sempre que
alguém rodasse o app pela primeira vez sem um `nointro.dat` já
presente — um risco real, não hipotético, já observado ao vivo.

Reportei a descoberta ao usuário e revertida a recomendação: nenhum
código de auto-download foi escrito. `nointro_fetch`/similar não existe
neste repositório. Ficou como estava antes (arquivo opcional, instalado
manualmente por mim ou pelo usuário) — usuário concordou ("ok").

**Back cover no painel da estante.** Pedido separado, na mesma mensagem:
mostrar o back cover (contra-capa da caixa) abaixo do logo/cartucho no
painel da estante. O usuário já tinha criado `assets/backcover/` e
colado ~725 arquivos de uma base externa nomeados
`"Titulo[IDNumerico].png"` (um pacote de box-art de terceiros, não do
No-Intro em si) — pediu pra eu renomear os que correspondem às 19 ROMs
da biblioteca real dele pro padrão que `cover`/`logo`/`cartridge` já
usam (nome de arquivo = stem exato da ROM, sem sanitizar).

Renomeação feita à mão, por correspondência de título (removendo o
sufixo `[ID]`, com atenção a diferenças de pontuação — a base de
origem troca `:` e `'` por `_`, ex. `"Donkey Kong Country 2_ Diddy_s
Kong Quest[1525].png"` → `"Donkey Kong Country 2 - Diddy's Kong Quest
(USA) (En,Fr) (Rev 1).png"`, e "Ninjawarriors" bateu exato com o nome
peculiar de uma palavra só que o próprio cabeçalho da ROM usa). Os
~706 arquivos sem ROM correspondente na biblioteca atual foram
deixados como estão, sem apagar nada — claramente um acervo maior,
guardado pra quando mais jogos forem adicionados.

**Código**: `ShelfPanelInfo` ganhou `backcover_img: Option<u64>`, mesmo
padrão de `cartridge_img` (id de textura já cacheada por
`Cabinet::set_image`, `None` = não desenha nada); `draw_shelf_panel`
(`cabinet.rs`) desenha logo abaixo do cartucho, mesma altura (210px),
mesmo corte por `limit` que os outros blocos já respeitam.
`shelf.rs` ganhou `backcover_dir` (`assets/backcover/`),
`tried_backcover` (cache "já tentei esse sha1 nesta visita", mesmo
esquema de `tried_cartridge`) e `backcover_id` (hash da textura, salt
próprio pra não colidir com `cover_id`/`wheel_id`/`cartridge_id`).

Verificado: build/clippy (`-D warnings`)/fmt/test limpos; captura
headless (`selector --filter Aladdin --shot`) contra a biblioteca real
do usuário confirma a arte de back cover aparecendo logo abaixo do
cartucho no painel.

## Revisão (2026-09-18, continuação 2): back cover maior, acima do cartucho, e um bug real de corte

Pedido de ajuste rápido: aumentar o back cover e trocar a ordem com o
cartucho (back cover primeiro agora). Mudança mecânica em
`draw_shelf_panel` — `BACKCOVER_H` de 210 pra 280px, blocos trocados de
posição.

A captura de verificação pegou um bug de verdade, não relacionado à
mudança em si mas exposto por ela: com o back cover maior, sobrava
menos espaço vertical antes do `limit` (o topo da área dos botões
Voltar/Configurações), e a linha "editora" — vinda do loop genérico de
`panel.info`, que desenha rótulo + valor em duas chamadas — só checava
`cy + GLYPH_H <= limit` (espaço pra UMA linha) antes de desenhar as
DUAS. "Capcom" acabou desenhado direto em cima do botão "Voltar".
Corrigido pra checar `GLYPH_H * 2` (rótulo + valor) antes de começar
cada entrada — não é uma solução perfeita pra um valor que quebra em
3+ linhas (não acontece hoje, já que categoria/descrição não aparecem
mais aqui), mas resolve o caso real.

Verificado: nova captura mostra "lançamento 1993" sem "editora" (não
coube mais, cortado limpo — sem sobreposição). Build/clippy
(`-D warnings`)/fmt/test limpos.

## Revisão (2026-09-18, continuação 3): rolagem no painel da estante

Pedido direto, na sequência natural do bug de corte que acabei de
consertar: em vez de simplesmente cortar o que não cabe (back cover,
cartucho, lançamento, editora...), rolar.

`draw_shelf_panel` foi reestruturada em torno de uma lista de "blocos"
(`enum PanelBlock { Image(id, altura), Release(valor), Field(rotulo,
valor) }`) — cada um mede a própria altura (`panel_block_height`) e se
desenha (`draw_panel_block`) de um jeito consistente entre as duas
funções (a mesma disciplina que já corrigiu o bug do corte: medir e
desenhar nunca podem discordar). `Release` ficou separado de `Field`
porque desenha rótulo e valor na mesma linha (um "destaque"), diferente
de `Field`, que empilha rótulo em cima do valor.

Tudo abaixo do cabeçalho fixo (logo/título) vira essa lista — back
cover, cartucho, lançamento, e cada par rótulo/valor de `info`. Se a
soma de tudo já cabe no espaço disponível, nada muda visualmente (sem
botões de rolagem, mesmo comportamento de sempre). Se não cabe, reserva
espaço pros botões "^ Cima"/"v Baixo" (mesmo componente visual que o
modal de cheats já usa pra rolar sua lista de linhas) e desenha só os
blocos a partir de `panel.scroll`, parando quando o próximo não coube
mais.

**Onde mora o estado do scroll**: em `shelf.rs`, não no `Cabinet` —
mesmo padrão que `top_row` (rolagem da grade principal) já segue.
`panel_scroll: usize` incrementa/decrementa nos cliques dos botões, sem
limite superior verificado ali (`draw_shelf_panel` clampa sozinha pra
desenhar, igual ao `scroll_modal` do modal de cheats já faz — deixar o
número "correr livre" e sempre reclampar na hora de desenhar é mais
simples que sincronizar um valor corrigido de volta pro chamador a
cada quadro). Reseta pra 0 sempre que o jogo focado muda (chave
`(in_recent, indice)`, comparada a cada quadro) — sem isso, trocar de
um jogo com painel longo pra um curto podia deixar o scroll "preso"
além do fim do conteúdo novo até o clamp da própria função de desenho
entrar em ação.

Verificado: `selector --filter Aladdin --shot` contra a biblioteca real
do usuário — agora Aladdin (back cover + cartucho + lançamento +
editora não cabem todos juntos) mostra "^ Cima" desabilitado (já no
topo) e "v Baixo" habilitado, confirmando que a rolagem ativa sozinha
quando necessário e fica ausente quando não. Não testei o clique nos
botões ao vivo (headless `--shot` captura só um quadro fixo, sem jeito
de simular clique nele) — a lógica em si (clique incrementa/decrementa
`panel_scroll`, redesenho usa o valor clampado) foi verificada por
revisão de código, seguindo de perto o padrão já comprovado do
`scroll_modal`/modal de cheats. Build/clippy (`-D warnings`)/fmt/test
limpos.

## Revisão (2026-09-18, continuação 4): resto do backcover renomeado, pros jogos que ainda não tem

Pedido de continuação: renomear pro padrão No-Intro também os ~706
arquivos de `assets/backcover/` que sobraram sem ROM correspondente —
pra já estarem no nome certo quando o usuário adicionar mais jogos,
sem precisar renomear na mão depois.

Sem a ROM em mãos, não dá pra saber o CRC32 exato — a única fonte de
verdade disponível é o próprio nome do jogo. Novo script,
`scripts/rename_art_to_nointro.py` (nome atual — generalizado numa
revisão seguinte pra também cobrir o pacote de cartucho; checado no
repo, é
plausível que o usuário cole mais arte de tempos em tempos e queira
rodar de novo): lê o `nointro.dat` inteiro, indexa cada `<game name>`
pelo título "base" (tira as tags de região/revisão/idioma do fim,
mesma ideia do `gen_tosec_data.py` com o TOSEC) normalizado (minúsculo,
pontuação vira espaço, "&" vira "and", e o artigo do padrão No-Intro
"Titulo, The" volta pro começo — inclusive quando tem mais coisa depois
dele, tipo "Legend of Zelda, The - A Link to the Past"), e casa cada
arquivo `"Titulo[id numerico].png"` do pacote de terceiros contra esse
índice pelo título normalizado. Quando um título bate com mais de uma
região no dat (comum — quase todo jogo tem edição US/EU/JP), prioriza
(USA), depois (World), depois (Europe), e evita uma edição (Beta)/
(Proto)/(Demo) quando existe uma comercial de verdade — nunca sobrescreve
um destino que já existe, pra poder rodar de novo sem risco.

Testado antes em uma cópia (`cp -R` pro scratchpad), não direto na
pasta real — duas rodadas de ajuste (a primeira bateu 680/704, achei
dois problemas sistemáticos por amostragem: tags tipo "(Absolute
Entertainment)" que o pacote de terceiros usa pra desambiguar, sem
nada a ver com região do No-Intro, atrapalhando o casamento; e o
padrão "X, The - Y" do No-Intro só sendo tratado quando "The" cai no
fim absoluto da string, não no meio de um subtítulo) — corrigidos os
dois, 694/704 bateram na segunda rodada. Os 10 que sobraram são
diferenças de nome de verdade entre as fontes (ex. "WCW SuperBrawl" vs
o "WCW Super Brawl" do No-Intro, uma palavra grudada diferente) ou
jogos que não existem no conjunto SNES do No-Intro (carts de
competição, homebrew) — ficaram com o nome antigo, sem tentar
adivinhar.

Aplicado na pasta de verdade do usuário depois de validado na cópia:
694 renomeados, os 19 já certos (biblioteca atual) intocados, 10 sem
correspondência mantidos como estavam. Conferido que os 19 da
biblioteca real continuam com o arquivo certo depois da rodada.

## Revisão (2026-09-18, continuação 5): cartucho sumido pra 8 das 19 ROMs

Usuário reportou (sem detalhe de qual jogo): "tem cartuchos que não
estão aparecendo das ROMs que estão na pasta". Comparação direta —
`stem` de cada `.sfc` em `roms/` contra `assets/cartridge/` — achou 8
das 19 sem arquivo batendo: as três Donkey Kong Country, Killer
Instinct, Marvel Super Heroes, Ninjawarriors, Super Mario World 2 e
Super Metroid.

Causa: o pacote de arte de cartucho (743 arquivos, diferente do pacote
de backcover — este já vem quase no padrão No-Intro, só sem as tags de
revisão/idioma) tinha "Donkey Kong Country (USA).png" mas a ROM do
usuário é "...(USA) (Rev 2).sfc"; mesma história pra `(Rev 1)`/
`(En,Fr)`/`(En,Ja)` faltando nos outros seis, mais dois casos de nome
base diferente ("Marvel Super Heroes - War of the Gems" no pacote vs
"Marvel Super Heroes **in** War of the Gems", o nome que o No-Intro
realmente usa; "Ninja Warriors, The" no pacote vs "Ninjawarriors", o
nome peculiar de uma palavra só que vem do próprio cabeçalho da ROM e
que o No-Intro também usa).

Corrigido com 8 `mv` diretos (não um script — só 8 casos, já
identificados um por um comparando contra a ROM de verdade, não contra
o dat do No-Intro por título como o script do backcover fez; aqui a
resposta certa já estava na mão, no nome do arquivo `.sfc`). Conferido
`cover`/`logo` também — só têm 2-3 arquivos cada (os que o usuário
colocou manualmente antes), sem pacote nenhum ali pra ter esse mesmo
tipo de descompasso.

Verificado: as 19 ROMs batem com `cartridge/` agora; captura headless
do painel do Killer Instinct mostra "v Baixo" (rolagem detectando mais
conteúdo — o cartucho está lá, só fora da primeira tela visível, uma
confirmação indireta de que a arte carregou). Não fiz a mesma rodada
"pros jogos que ainda não tem" no cartucho como fiz no backcover —
o pedido desta vez foi só sobre as ROMs que já estão na pasta; ofereço
fazer o mesmo passo pro resto do pacote de cartucho se o usuário
quiser.

## Revisão (2026-09-18, continuação 6): resto do cartucho renomeado, e pastas `-extra` separando o que ainda não é da biblioteca

Dois pedidos numa mensagem: (1) fazer no cartucho o mesmo que já foi
feito no backcover (renomear o resto do pacote pro padrão No-Intro,
mesmo pros jogos que o usuário ainda não tem); (2) mover tudo que não
é da biblioteca atual pra outra pasta, de fácil acesso na hora de
adicionar mais jogos.

**Generalizando o script.** `rename_backcover_to_nointro.py` virou
`rename_art_to_nointro.py` — o pacote de cartucho não usa o mesmo
convenção "Titulo[id].png" do pacote de backcover; já vem quase pronto,
só falta a tag de revisão/idioma que o No-Intro exige (o mesmo problema
da revisão anterior, só que em massa: 743 arquivos, não 8). Trocada a
condição de disparo: em vez de "só mexe se tiver um `[id]` no nome",
agora é "só mexe se o nome não bater *exatamente* com uma entrada do
dat" — cobre os dois formatos com o mesmo código, e não haveria razão
pra manter dois scripts quase idênticos.

Testado numa cópia primeiro: 620 já exatos (sem mexer), 70 renomeados,
8 pulados (arquivos duplicados do próprio pacote, diferindo só em
maiúsculas — ex. "Cutthroat Island" vs a grafia real do No-Intro
"CutThroat Island" —, destino já ocupado, nada perdido), 45 sem
correspondência (protótipos/carts de competição/homebrew fora do
conjunto comercial do No-Intro, ou diferença de nome real entre as
fontes). Aplicado na pasta de verdade com o mesmo resultado; conferido
que as 19 ROMs da biblioteca continuam com cartucho batendo depois.

**Separar biblioteca atual do resto do pacote.** Novo script,
`scripts/sync_art_with_roms.py` — para cada pasta de arte
(`cover`/`logo`/`cartridge`/`backcover`), move pro lado quem não bate
com nenhuma ROM em `roms/` (pra uma pasta irmã `<tipo>-extra/`, nome
que o app não escaneia — invisível pro `find_local_art` de propósito,
só uma prateleira). De mão dupla e idempotente: um arquivo que já está
em `<tipo>-extra/` e passa a bater com uma ROM nova (adicionada depois)
volta sozinho pra `<tipo>/` da próxima vez que o script rodar — é
literalmente o "fácil acesso" que foi pedido: adicionar o jogo, rodar o
script de novo, a arte aparece.

Achado durante o teste (numa cópia, antes de tocar nos arquivos de
verdade): "Rock N' Roll Racing" tinha uma variante "Rock n' Roll
Racing" (n minúsculo) no pacote de cartucho — como o `n`/`N` só difere
em maiúscula, e o HFS+/APFS do Mac trata isso como o mesmo arquivo na
prática, mas a comparação de string em Python não, o script quase
moveu o arquivo *certo* pra pasta errada (não é uma variação de nome
dentro da MESMA pasta, que o filesystem resolveria sozinho — é mover
pra uma pasta *diferente*, onde a diferença de maiúscula importa de
verdade). Corrigido: antes de decidir mover pra `-extra/`, o script
agora também checa por um match ignorando maiúsculas contra as ROMs
possuídas e, se achar um, só corrige a grafia no lugar (rename simples,
mesma pasta) em vez de mover.

Aplicado de verdade: `cartridge` e `backcover` ficaram só com os 19
arquivos da biblioteca atual cada um; `cartridge-extra/` e
`backcover-extra/` guardam o resto (724 e 704 respectivamente).
`cover`/`logo` não tinham nada pra mover (só os 2-3 arquivos que o
usuário já tinha colocado manualmente, todos de jogos que já tem) — as
pastas `-extra` vazias correspondentes foram removidas.

Verificado: as 19 ROMs continuam com cartucho e back cover depois da
separação; captura headless do "Rock N' Roll Racing" confirma a arte
carregando com o nome corrigido. Nenhum código Rust mudou nesta
revisão — só os dois scripts Python e os arquivos de dados do usuário.
