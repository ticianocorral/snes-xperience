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
- [ ] **Teste de duas horas** (§9) — recomendado *antes* de investir em
      cartucho/console, ver "A seguir"
- [ ] Cartucho encaixado no console, com o rótulo (`texture` do ScreenScraper)
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

## Verificado

- `emu-run --shot` — jogo pelo gabinete, idêntico
- `selector --frames --shot` — estante **pelo tubo**, antes e depois de ligar
  `BlendMode::Blend` na textura da tela (BMP idêntico — sem regressão no
  caminho normal)
- testes de `screen_area` / `fit_aspect_in` / `wrapped_height`

O chuvisco, o zumbido e a mistura de entrada usam o mesmo caminho de composição
já conferido (mesma malha, mesmo `render_geometry`), mas não têm captura
headless própria — falta ver ao vivo.

## A seguir

Recomendação do plano (§7, tabela de risco + §9): fazer o **teste de duas
horas** agora, com o jogo real, *antes* de investir em cartucho/console — evita
modelar em cima de uma moldura que cansa em sessão longa. Isso pede jogo ao
vivo, não dá pra automatizar aqui. **Pausado aqui por decisão do usuário
(2026-09-10) até o teste ser feito.**

Depois disso: cartucho encaixado com o rótulo (`texture` do ScreenScraper) e
botões do console com trava de ejeção — como não há pipeline de arte (Blender)
ainda, a forma mais provável é continuar procedural, como o gabinete.
