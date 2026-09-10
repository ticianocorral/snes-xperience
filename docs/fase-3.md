# Fase 3 — A moldura

Plano §6: cena estática composta, shader de CRT, cartucho com rótulo, botões do
console, sinal off, trava de ejeção, transição entre as telas.

Progresso:

- [x] **Gabinete atrás do tubo** — a tela do jogo fica recuada num gabinete
      escuro; a imagem é a coisa mais clara do quadro (§3.2)
- [ ] Estante desenhada dentro do mesmo gabinete (uma janela só, sem recriar)
- [ ] Sequência de sinal off (chuvisco curto → ruído baixo) e transição entre as
      telas por ela
- [ ] Cartucho encaixado no console, com o rótulo (`texture` do ScreenScraper)
- [ ] Botões do console (desligar / ejetar / reset) com trava de ejeção
- [ ] Teste de duas horas (§8) antes de investir em arte/modelagem

## Gabinete (`xperience-platform::video`)

O `Video` já desenhava o quadro do jogo através de uma malha com distorção de
barril (tubo CRT). Agora ele **recua a tela num gabinete**:

1. limpa a janela com a cor do recuo (quase preto);
2. desenha uma malha de anel de quatro quads da borda da janela (cor do
   gabinete, cinza-quente escuro) até a borda da tela (cor do recuo) — um chanfro
   que põe a tela num poço sombreado;
3. desenha a malha do tubo CRT dentro da tela.

A tela é a janela recuada por frações fixas — `BEZEL_SIDE`/`BEZEL_TOP` 7 %,
`BEZEL_CHIN` 11 % embaixo (o "queixo" do gabinete) — e então o maior retângulo
4:3 que cabe nesse vão, centralizado (`screen_area` + `fit_aspect_in`). A malha do
gabinete é cacheada e só reconstruída quando a janela ou a tela muda de tamanho
(`ensure_bezel`, chave `(win_w, win_h, screen_w, screen_h)`).

Cores em `video.rs`: `CABINET` = `(40, 37, 33)`, `RECESS` = `(4, 4, 5)`. Ambas
bem mais escuras que qualquer quadro de jogo — o gabinete e o "móvel" são mais
escuros que a tela (§3.2).

Verificado headless com `emu-run --shot` (o mesmo caminho de `capture_bmp`
desenha o gabinete): a imagem fica recuada, com o chanfro e o queixo visíveis.

## A seguir

A estante ainda abre a própria janela lisa. O próximo passo é a **janela única**:
`xperience` cria um gabinete persistente e desenha estante *ou* jogo no vão da
tela, sem recriar janela na troca — pré-requisito pra sequência de sinal off e a
transição entre as telas passar por ela.
