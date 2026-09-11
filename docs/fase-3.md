# Fase 3 — A moldura

Plano §6: cena estática composta, shader de CRT, cartucho com rótulo, botões do
console, sinal off, trava de ejeção, transição entre as telas.

Progresso:

- [x] **Gabinete atrás do tubo** — a tela do jogo fica recuada num gabinete
      escuro; a imagem é a coisa mais clara do quadro (§3.2)
- [x] **Janela única** — `xperience` cria um `Cabinet` e desenha estante *ou*
      jogo no vão da tela, sem recriar janela na troca
- [x] **Estante pelo tubo** — o 2D vai pra um buffer do tamanho do vão e é
      deformado pela mesma malha CRT do jogo
- [x] **Sinal off** — ao sair do jogo, meio segundo de chuvisco pelo tubo com
      zumbido de RF decaindo, assentando num hiss fraco, e corta (§3.3)
- [x] **"A estante entra por cima"** — os primeiros quadros da estante
      aparecem misturados com o chuvisco residual, não um corte seco
- [x] **Cartucho no slot** — procedural (sem Blender): rótulo `texture` do
      ScreenScraper quando existe, senão o nome da ROM. Nunca ausente.
- [ ] **Teste de duas horas** (§9) — em andamento pelo usuário
- [ ] Botões do console (desligar / ejetar / reset) com trava de ejeção

## `Cabinet` (`xperience-platform::cabinet`)

Uma janela só pro app todo. `Ui` e `Video` viraram um tipo só, `Cabinet`. Todo
quadro limpa com a cor do recuo, desenha o conteúdo deformado pelo tubo e por
cima a malha de anel do gabinete (`build_bezel_mesh`).

- **Jogo:** `present_frame` — sobe o framebuffer pra uma textura e desenha a
  malha do tubo CRT no maior 4:3 do vão.
- **Estante:** `frame_2d(bg, |s| …)` — o *closure* desenha 2D plano
  (`Screen::fill` / `text` / `text_wrapped` / `image_fit` / `clip`) num buffer do
  tamanho do vão (coordenadas 0..vão); o buffer é então deformado pela mesma
  malha CRT. `capture_2d` faz o mesmo pra um alvo offscreen e salva BMP (headless).
- **Sinal off:** `present_static(level)` — enche uma textura pequena (320×240)
  de ruído cinza (xorshift, teto bem abaixo do branco — **sem flash**) e compõe
  pelo tubo. `level` 1.0 = nevasca, 0.12 = hiss quase parado.

O vão é a janela recuada por frações fixas — `BEZEL_SIDE`/`BEZEL_TOP` 7 %,
`BEZEL_CHIN` 11 % (o "queixo"). Cores: `CABINET` = `(40, 37, 33)`,
`RECESS` = `(4, 4, 5)`.

## Transição (`xperience::signal_off`)

Ao `run_game` devolver `GameExit::ToShelf` (Esc no jogo), `xperience` roda
`signal_off` antes de voltar à estante: ~0,65 s de `present_static` com `level`
caindo de 1.0 → ~0,12, e em paralelo um buffer de ruído estéreo com amplitude
decrescente numa stream de áudio curta (22 kHz). No fim, `AudioOut::clear` — o
zumbido corta, não arrasta (§3.3). `signal_off` devolve o `level` em que parou.
Fechar a janela do jogo (`GameExit::Quit`) não passa por isso.

**Entrada por cima:** esse `level` vira `ShelfOpts::fade_in` da próxima chamada
de `shelf::run`. Os primeiros 18 quadros usam
`Cabinet::frame_2d_fade_in(bg, draw, static_level, shelf_alpha)` — desenha a
estante no buffer de sempre, mas compõe **dois** `render_geometry` no lugar de
um: o chuvisco pela malha CRT (alfa 1.0) e por cima a estante pela mesma malha
com alfa 0..1 crescendo por quadro (precisa de `BlendMode::Blend` na textura da
tela). Depois do quadro 18, volta pro `frame_2d` normal.

## Cartucho no slot (`Cabinet::set_cartridge`)

Puramente procedural, como o gabinete — sem depender de arte 3D. Um retângulo
("casca" + friso, cores `CART_SHELL`/`CART_RIM`) no queixo do gabinete,
canto inferior direito, do tamanho de ~62 % da altura do queixo:

- Com rótulo (`texture` do ScreenScraper, escolhido na estante e passado pelo
  `Pick::Play { texture, .. }`): a arte decodificada (`image`, ≤300 px) dentro
  da casca, redimensionada mantendo proporção.
- Sem rótulo: o nome da ROM (`file_stem`) em texto, truncado pra caber.
- **Nunca ausente** — mesmo sem scrape, o slot mostra o nome (plano §3.2: "nunca
  é o primeiro item a ser cortado").

É mobília do gabinete, não passa pelo tubo — desenhada direto na janela, tanto
em `present_frame` quanto em `capture_bmp` (mesmas funções livres
`draw_cartridge_slot` / `cartridge_slot_rect`, chamadas nos dois caminhos).
`emu-run --cartridge-label img.png` deixa testar sem catálogo.

## Verificado

- `emu-run --shot` — jogo pelo gabinete, idêntico
- `emu-run --shot --cartridge-label` — rótulo no slot, e sem a flag, nome em
  texto — os dois caminhos headless, com e sem arte
- `selector --frames --shot` — estante pelo tubo, **byte a byte idêntica** ao
  shot anterior (sem regressão das mudanças de blend/cartucho no caminho 2D)
- testes de `screen_area` / `fit_aspect_in` / `wrapped_height`

O chuvisco, o zumbido e a mistura de entrada usam o mesmo caminho de composição
já conferido (mesma malha, mesmo `render_geometry`), mas não têm captura
headless própria — falta ver ao vivo.

## A seguir

**Teste de duas horas em andamento** pelo usuário (plano §7/§9) — o próximo
passo depende do resultado: ajustar contraste/curvatura se cansar, ou seguir
pros botões do console (desligar / ejetar / reset) com trava de ejeção se a
moldura sustentar a sessão longa.
