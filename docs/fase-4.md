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
