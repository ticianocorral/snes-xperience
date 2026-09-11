# Fase 4 — O painel

Plano §6: logo, cheats com interruptor, tela de pausa, anotações com captura
de tela, e cinco jogos com senha e dica escritas à mão.

Progresso:

- [x] **Painel lateral (esqueleto)** — coluna de widgets reais ao lado do tubo
      durante o jogo, com logo (ou nome) no topo e o tempo de sessão embaixo
- [x] **Comandos** (item 3 do §3.2) — legenda dos botões do console
- [x] **Cheats com interruptor** (`cht` do libretro-database, §4.4)
- [x] **Captura de tela pro caderno** (§3.4) — falta só a leitura/escrita
      longa, que é item da tela de pausa, abaixo
- [x] **Tela de pausa (leitura)** — página dupla, sem tubo, mostra o caderno
      do jogo; a escrita por teclado fica pro próximo incremento
- [ ] Tela de pausa (escrita) — subsistema de entrada de texto, ainda não
      existe no app
- [ ] Senhas e dicas — tabela manual, cinco jogos pra começar (§4.5/§4.6)

## Desvio deliberado do §3.2

O plano lista "cartucho encaixado no console" como item 2 da coluna do
painel. Na Fase 3 eu já tinha posto o cartucho no queixo do próprio gabinete
(mobília da TV, não do painel) — decisão tomada antes do painel existir, e que
já passou pelo teste de duas horas. Mantive assim em vez de mover: refazer
teria custo alto pra ganho estético pequeno, e o cartucho já cumpre o papel de
"nunca é o primeiro item a ser cortado". O painel cobre os itens 1 e 6
(logo, tempo de sessão) e vai cobrir 3-5 (comandos, cheats, anotações) nos
próximos incrementos.

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

## A seguir

Escrita na pausa: precisa de um subsistema de entrada de texto que a
plataforma ainda não tem (`Platform::poll` só traduz teclas discretas via
`KeyMap` — nada de captura de texto livre/Unicode). Depois disso, o texto
digitado entra no `.md` ao lado das capturas. Por fim, senhas/dicas manuais
pra cinco a dez jogos (§4.5/§4.6) — conteúdo que preciso escrever com o
usuário, não vou inventar senha de jogo.
