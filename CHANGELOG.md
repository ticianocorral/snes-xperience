# Changelog

Todas as mudanças relevantes deste projeto são registradas aqui.

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e o
versionamento segue [SemVer](https://semver.org/lang/pt-BR/). Enquanto a versão
for `0.x`, a API das crates e a interface de linha de comando podem mudar sem
aviso — só o incremento de _minor_ marca um conjunto de mudanças.

## [Não lançado]

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
  tem o caminho do jogo (`present_frame`) e o 2D da estante (`begin_2d` / `fill`
  / `text` / … / `present_2d`, coordenadas no vão da tela). `xperience` cria um
  `Cabinet` e passa `&mut` dele pra `shelf::run` e `runner::run_game` — a troca
  estante↔jogo não recria janela. Testes de `screen_area` / `fit_aspect_in`.

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

[Não lançado]: https://github.com/ticianocorral/snes-xperience/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ticianocorral/snes-xperience/releases/tag/v0.1.0
