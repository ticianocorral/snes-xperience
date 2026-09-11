# Fase 4 — O painel

Plano §6: logo, cheats com interruptor, tela de pausa, anotações com captura
de tela, e cinco jogos com senha e dica escritas à mão.

Progresso:

- [x] **Painel lateral (esqueleto)** — coluna de widgets reais ao lado do tubo
      durante o jogo, com logo (ou nome) no topo e o tempo de sessão embaixo
- [ ] Comandos (item 3 do §3.2) — legenda dos botões do console
- [ ] Cheats com interruptor (`cht` do libretro-database, §4.4)
- [ ] Anotações com captura de tela (§3.4)
- [ ] Tela de pausa (layout de página dupla para anotações/senhas longas)
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

## Verificado

- `emu-run --shot --cartridge-label` (sem `--logo`): painel com o título
  quebrado em duas linhas, tempo de sessão, cartucho no canto do gabinete
  encolhido
- `emu-run --shot --cartridge-label --logo`: logo no topo do painel
- `emu-run --shot --shot-off --cartridge-label --logo`: painel + cartucho
  continuam visíveis durante o chuvisco de console desligado
- `selector --frames --shot`: estante **byte a byte idêntica** a antes — sem
  painel reservado lá, como esperado
- fmt / clippy / 15 suítes — verdes

## A seguir

Comandos (legenda simples dos botões — sem 3D do console ainda), depois
cheats com interruptor (a parte mais autocontida: banco `cht` embutido, sem
rede). Anotações e tela de pausa são as peças maiores, deixadas pro fim.
