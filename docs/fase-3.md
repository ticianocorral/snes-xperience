# Fase 3 — A moldura

Plano §6: cena estática composta, shader de CRT, cartucho com rótulo, botões do
console, sinal off, trava de ejeção, transição entre as telas.

Progresso:

- [x] **Gabinete atrás do tubo** — a tela do jogo fica recuada num gabinete
      escuro; a imagem é a coisa mais clara do quadro (§3.2)
- [x] **Janela única** — `xperience` cria um `Cabinet` e desenha estante *ou*
      jogo no vão da tela, sem recriar janela na troca
- [ ] Sequência de sinal off (chuvisco curto → ruído baixo) e transição entre as
      telas por ela — a estante passa a compor pelo tubo aqui
- [ ] Cartucho encaixado no console, com o rótulo (`texture` do ScreenScraper)
- [ ] Botões do console (desligar / ejetar / reset) com trava de ejeção
- [ ] Teste de duas horas (§8) antes de investir em arte/modelagem

## `Cabinet` (`xperience-platform::cabinet`)

Uma janela só pro app todo. `Ui` e `Video` viraram um tipo só, `Cabinet`, que
tem os dois caminhos de desenho e a mesma moldura sempre por cima:

- **Jogo:** `present_frame` faz o que o `Video` fazia — sobe o framebuffer pra
  uma textura e desenha a malha do tubo CRT no vão da tela.
- **Estante:** `begin_2d` / `fill` / `text` / `text_wrapped` / `image_fit` /
  `clip` / `present_2d` — o 2D do antigo `Ui`, com as coordenadas em
  *screen-local* (0,0 = canto do vão): cada chamada é deslocada pra origem do
  vão e recortada nele.
- **Moldura:** todo quadro limpa com a cor do recuo, desenha o conteúdo e por
  cima a malha de anel do gabinete (`build_bezel_mesh`), quatro quads da borda
  da janela (cinza-plástico) até a borda da tela (quase preto) — a tela num poço
  sombreado.

O vão é a janela recuada por frações fixas — `BEZEL_SIDE`/`BEZEL_TOP` 7 %,
`BEZEL_CHIN` 11 % (o "queixo") — e, pro jogo, o maior 4:3 centralizado nele
(`screen_area` + `fit_aspect_in`, ambos com teste). Malhas cacheadas por tamanho.

`xperience` cria **um** `Cabinet` antes do laço e passa `&mut` dele tanto pra
`shelf::run` quanto pra `runner::run_game`; a troca de tela não toca na janela.
`emu-run` e `selector` fazem o próprio `Cabinet` do mesmo jeito.

Cores em `cabinet.rs`: `CABINET` = `(40, 37, 33)`, `RECESS` = `(4, 4, 5)`.

Verificado: `emu-run --shot` (jogo pelo gabinete, inalterado); `selector
--frames` headless (estante no gabinete, sem panic); testes de `screen_area` /
`fit_aspect_in` / `wrapped_height`. O visual da estante recuada ainda precisa de
uma conferida ao vivo (headless 2D no macOS não compõe).

## A seguir

A estante é desenhada **plana** no recuo (cópia direta). O próximo passo — a
sequência de sinal off — é o ponto natural pra fazer o 2D compor pelo tubo
(render-to-texture do vão), já que o chuvisco e a transição entre estante e jogo
vão querer a mesma superfície curva.
