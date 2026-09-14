# Fase 2 — O seletor

Plano §6: varredura de pasta, identificação por hash, busca de metadados,
estante com capas, navegação por gamepad, preenchimento progressivo.

Progresso:

- [x] **Varredura + hash + catálogo**
- [x] **Estante com capas** (`selector`)
- [x] **Navegação por gamepad**
- [x] **Preenchimento progressivo** (placeholder → capa conforme decodifica)
- [x] **Busca por digitação**
- [x] **Binário `xperience` único** (estante → jogo → estante, sem shell)
- [x] **Busca de metadados sob demanda** (o jogo em foco é scrapeado na hora)
- [x] **Ficha completa** (logo `wheel` no topo quando existe; sinopse longa rola
      sozinha dentro de uma região recortada)

## Catálogo (`xperience-domain`)

### `library::scan(dir)`

Varre `dir` recursivamente atrás de `.sfc/.smc/.fig/.swc/.bs/.st/.bin`,
calcula CRC32/MD5/SHA1 de cada um (sem header de copiadora) e devolve
`Vec<ScannedRom>`. Arquivos ilegíveis são pulados com aviso.

### `Catalog` — SQLite

Cache local agressivo (§4.2), aberto em
`$HOME/.local/share/snes-xperience/catalog.db` por padrão. Duas tabelas:

| `rom` | `sha1` (PK), `crc32`, `path`, `size`, `internal_name`, `added_at`, `last_played_at`, `play_count` |
|---|---|
| `meta` | `sha1` (FK), `name`, `year`, `developer`, `publisher`, `genre`, `players`, `region`, `synopsis`, `cover_path`, `texture_path`, `wheel_path`, `scraped_at` |

Sem linha em `meta` = nunca scrapeado ⇒ o seletor mostra placeholder.

- `upsert_rom` — insere ou atualiza caminho/tamanho; `added_at` só na inserção.
- `set_meta` — grava a ficha; caminhos de arte usam `COALESCE` (não apagam arte
  já baixada num re-scrape).
- `mark_played` — `last_played_at = agora`, `play_count += 1`.
- `list(Order::Shelf)` — **último jogado primeiro, depois recém-adicionado**
  (§3.1), num único `ORDER BY`. `Order::Name` para a busca.
- `unscraped(limit)` — os ainda sem ficha, mais antigos primeiro.
- `prune_missing` — remove ROMs cujo arquivo sumiu.

## ScreenScraper estendido

`GameInfo` (era `GameMedia`) agora traz `year`, `developer`, `publisher`,
`genre`, `players`, `synopsis` (regionalizados / por idioma: `en > pt > es`) e a
mídia **`box-2D`** (capa) além de `texture`/`wheel`. `Client::download(url,
dest)` baixa uma arte para disco (anexa as credenciais de dev, que a API exige
nas mídias também). `ScrapeError::QuotaExhausted` é distinto agora.

## Ferramenta `library`

```bash
library scan   --roms DIR   [--catalog PATH] [--prune]
library scrape               [--catalog PATH] [--limit N] [--art DIR] [--sleep MS]
library list                 [--catalog PATH] [--order shelf|name]
```

- `scan` — varre e povoa o catálogo (idempotente).
- `scrape` — para cada ROM sem ficha (até `--limit`, padrão 20), consulta o
  ScreenScraper, baixa capa/texture/wheel para `--art` (padrão `<dir do
  catálogo>/art`), grava a ficha. Pausa `--sleep` ms entre chamadas e **para**
  na primeira resposta de cota esgotada. Precisa de `SS_DEVID` / `SS_DEVPASSWORD`
  no ambiente.
- `list` — imprime a estante em texto.

## Seletor (`selector`)

Estante rolável de capas + painel de detalhes à direita, tudo desenhado numa
camada 2D mínima (`xperience-platform::Ui`): retângulos, texto 8×8 (`font8x8`,
sem fonte de sistema) e imagens com letterbox. Sem OSD/menu — feio que funciona.

```bash
selector [--catalog PATH] [--order shelf|name] [--no-scrape]
```

- **Navegação:** setas / d-pad movem a seleção na grade; PgUp/PgDn e ombros do
  controle pulam uma página; Home/End; A/Enter escolhe; B/Esc sai (ou limpa a
  busca). Gamepad em primeiro lugar (§3.1).
- **Busca por digitação:** basta digitar — filtra por substring no título,
  Backspace edita, Esc limpa.
- **Preenchimento progressivo:** capas e logos (`wheel`) em disco são
  decodificados (`image`, reduzidos para ≤512 px, alfa preservado) numa thread e
  viram textura conforme chegam; enquanto isso o tile mostra o título.
- **Ficha:** o painel mostra o logo `wheel` no topo quando existe (senão o
  título em texto), depois ano/desenvolvedora/editora/gênero/jogadores/região/
  partidas, e a sinopse. Sinopse longa fica numa região recortada (`Ui::clip`)
  que, depois de ~1,3 s parada, rola sozinha até o fim aparecer; recomeça ao
  trocar de jogo.
- **Scrape sob demanda:** com `SS_DEVID` / `SS_DEVPASSWORD` no ambiente, quando a
  seleção pousa num jogo sem ficha por ~8 quadros, uma thread consulta o
  ScreenScraper, baixa capa/texture/wheel para `<dir do catálogo>/art` e grava a
  ficha; a estante recarrega e a capa entra na fila de decodificação. Um pedido
  por jogo (nunca repete), ~700 ms entre chamadas, para de vez ao esgotar a cota.
  O painel mostra `scraping…` / `scrape quota reached`. `--no-scrape` desliga.
- **Saída:** imprime o caminho da ROM escolhida no stdout e sai 0; cancelou,
  sai 1. `mark_played` é chamado na escolha.

O laço da estante mora em `xperience_app::shelf`; o binário `selector` é só uma
casca fina em volta dele (imprime o caminho / código de saída). O download de
arte (`download_art`) foi para `xperience_domain::art`, compartilhado com o
`library scrape` em lote.

## Binário `xperience` — tudo junto

```bash
xperience --core <lib> [--catalog DB] [--config config.toml]
          [--save-dir DIR] [--system-dir DIR] [--order shelf|name] [--runahead N]
          [--no-scrape]
```

Um processo só: abre a estante, roda o jogo escolhido, volta pra estante, repete.
`Esc` dentro do jogo volta pra estante; `Esc` / fechar janela na estante, ou
fechar a janela do jogo (Cmd-Q / botão vermelho), encerra o app. Sem `scripts/`.

Por dentro, reaproveita os dois laços fatiados em módulos de biblioteca:

| Módulo | O quê | Também usado por |
|---|---|---|
| `xperience_app::shelf::run` | a estante; devolve `Pick::Play(path)` ou `Pick::Quit` | `selector` |
| `xperience_app::runner::run_game` | o laço do emulador; devolve `GameExit::ToShelf` ou `GameExit::Quit` | `emu-run` |

O `Platform` (SDL) é criado uma vez e emprestado (`&mut`) pra cada tela — a
janela da estante e a do jogo são criadas e destruídas a cada troca. A distinção
"voltar" (Esc) × "encerrar" (fechar janela / Cmd-Q) veio de um `UiEvent` novo,
`CloseRequested`, separado do `Quit`.

Padrões ficam em `~/.local/share/snes-xperience/` (`catalog.db`, `saves/`).

## Fase 2 — concluída

Todos os itens do §6 do plano estão feitos: varredura + catálogo, estante com
capas, navegação por gamepad, preenchimento progressivo, busca por digitação,
scrape sob demanda, o binário `xperience` único e a ficha completa. O que falta
pro projeto é a **moldura** (Fase 3): o mesmo tubo CRT do `emu-run` como janela
sempre presente, com a estante e o jogo desenhados dentro dela.

## Revisão (2026-09-14): estante sobre o sinal off, lista sem capa, clique

Três pedidos do usuário, depois que a moldura (Fase 3) e o painel (Fase 4) já
estavam no ar:

- **Estante sobre a TV sem sinal.** `shelf::run` não desenha mais num fundo
  liso — chama `Cabinet::frame_2d_fade_in(bg, render, nível, alpha)` o tempo
  todo, não só nos 18 quadros de entrada depois de um eject. Antes, esse
  alpha subia até 1.0 e a função dava lugar a `frame_2d` (fundo liso);
  agora ele só sobe até `SHELF_ALPHA` (0.92) e fica ali, então o chuvisco
  fraco (`idle::RESTING_STATIC`, o mesmo nível do console desligado) continua
  sangrando por baixo da estante o tempo todo — sutil o bastante pra não
  atrapalhar a leitura da grade/lista. Isso deixou o `--shot` headless
  (`capture_2d`, sem o chuvisco) levemente diferente do que se vê ao vivo —
  aceitável pra uma ferramenta de teste, não pro usuário final.
- **Lista quando não há capa nenhuma.** Antes, sem `texture`/capa a estante
  ainda tentava desenhar tiles vazios com o título quebrado dentro — parece
  app quebrado, não um catálogo sem arte ainda. Agora, se **nenhum** jogo em
  `view` tem capa decodificada (`Cabinet::has_image`, novo — o mesmo que
  `Screen::has_image`, mas chamável antes de montar o closure de desenho),
  a estante inteira vira uma lista numerada de uma coluna só, estilo menu de
  multicart de NES pirata: `"{:03}  {}"` (índice + título em caixa alta),
  barra de seleção inteira preenchida (`HILITE`) em vez de contorno em torno
  de um tile. `GridLayout` (`shelf.rs`, novo) unifica os dois modos — a
  navegação por D-pad/gamepad (`cols`/`vis_rows`) não muda nada, só a
  geometria de célula (`cell_w`/`cell_h`/`item_w`/`item_h`) e o desenho.
  Ainda progressivo: assim que a primeira capa chega (scrape ligado), a
  estante volta pra grade sozinha no próximo quadro.
- **Clique de verdade na estante.** Ver `docs/fase-4.md`, que documenta a
  infraestrutura de mouse inteira (nasceu ali, junto dos botões do painel);
  aqui só o consumo: `Cabinet::hit_screen_point` mapeia o clique (já em
  coordenadas de output, via `window_to_output`) pro espaço 2D da estante —
  aproximado, ignora a curvatura do tubo (`CRT_WARP` é sutil, 0.06), preciso
  o bastante pra acertar tile/linha. `GridLayout::tile_at` devolve o índice;
  clicar um item novo seleciona, clicar de novo no já selecionado joga —
  o mesmo dois passos que mover-depois-confirmar no controle.

## Revisão (2026-09-14): sem SQLite, sem ScreenScraper — app portátil

Mudança grande, a pedido do usuário: o catálogo desta fase (`Catalog` sobre
SQLite, seção acima) e o ScreenScraper (seção "ScreenScraper estendido")
**saíram inteiros**. `crates/domain/src/screenscraper.rs`, `art.rs` e a
dependência `rusqlite` não existem mais; os binários `library` e
`scrape-test` também foram removidos (sem banco pra popular/inspecionar,
sem API pra testar). O que fica desta fase: `library::scan` (intocado,
nunca teve acoplamento com SQLite) e o laço da estante em `shelf.rs`
(reescrito por dentro, mesma forma por fora).

**`Catalog` novo (`crates/domain/src/catalog.rs`)** — sem banco: escaneia
`roms/` do zero a cada `Catalog::open` (`library::scan`, o mesmo de sempre)
e funde com um `library.json` ao lado do executável — só o que uma
varredura não sabe por si (`added_at`/`last_played_at`/`play_count`, por
sha1). `RomRow` perdeu a tabela `meta` inteira; ganhou `nointro_name`
(abaixo). `CatalogEntry::title()`: No-Intro → nome interno do cabeçalho →
nome do arquivo — a camada "nome raspado" não existe mais.

**Nomeação por DAT No-Intro (`crates/domain/src/nointro.rs`, novo)** —
resolve o que o plano §4.1 sempre pediu ("identificação por hash... devolve
título canônico"), só que agora é o que existe de fato: `NoIntroDat::load`
lê um XML do No-Intro (`roxmltree`, dependência nova — nada no workspace
lia XML antes) e monta um `HashMap<CRC32, nome>`; `Catalog::open` faz um
`lookup` por ROM escaneada, sem rede, sem espera — o nome certo já sai no
primeiro quadro da estante, não precisa do esquema de "dwell" que o
ScreenScraper precisava. Opcional: sem o arquivo (`dirs::nointro_dat_path()`,
`app_root()/nointro.dat`), o app funciona como antes desta revisão. Testado
com o DAT real "Nintendo - Super Nintendo Entertainment System" (4129
entradas) contra ROMs reais do usuário — títulos com região/revisão saíram
certos de primeira (`Aladdin (USA)`, `Donkey Kong Country 2 - Diddy's Kong
Quest (USA) (En,Fr) (Rev 1)`).

**Capa/logo local (`shelf.rs`)** — sem ScreenScraper, sem worker thread:
`assets/cover/<nome-do-arquivo-da-rom>.{png,jpg,jpeg}` e `assets/logo/…`
(mesma convenção), decodificados sob demanda, só pras linhas visíveis, sem
thread nenhuma (arquivo local não tem cota nem latência de rede pra
justificar isso — a thread + canal de decodificação em paralelo que existia
saiu inteira). `assets/cartridge/` também nasce no boot, mas nada lê dela —
o cartucho continua fora de cena (decisão de uma sessão anterior). Testado
ao vivo: uma capa aparecendo já basta pra estante trocar de lista pra
grade sozinha; os jogos sem capa continuam com o título dentro do tile
(fallback que já existia).

**App portátil (`crates/app/src/dirs.rs`, reescrito)** — troca a base XDG
(`$HOME/.local/share`, `$HOME/.config`) por `app_root()`: a pasta do
executável, com um caso especial no macOS pra um `.app` empacotado (o
binário de verdade mora em `Nome.app/Contents/MacOS/`, três níveis dentro
do bundle — `app_root()` detecta esse padrão e sobe até a pasta que contém
o `Nome.app`, que é onde um usuário esperaria achar `roms/` no Finder).
`xperience.rs::main` cria `roms/`, `core/`, `assets/{cover,logo,
cartridge}/`, `saves/`, `notes/` no boot, e copia (uma vez, sem sobrescrever)
`saves/`/`notes/` do local antigo se existirem e o novo ainda estiver vazio
— proteção de progresso de jogo de verdade, diferente do play-count (esse
começa do zero, por decisão do usuário). `config.toml` virou `xperience.cfg`
(mesma sintaxe TOML por baixo, só nome/local novos — "cfg" aqui é a
convenção de emulador tipo RetroArch, não formato diferente).

**Núcleo do snes9x pelo menu (`crates/app/src/core_update.rs`, novo)** — a
linha "ScreenScraper" das configurações virou "Núcleo": baixa/atualiza o
`snes9x_libretro` direto do buildbot oficial do libretro
(`buildbot.libretro.com/nightly/<plataforma>/<arquitetura>/latest/
snes9x_libretro.<ext>.zip` — URLs confirmadas ao vivo pra macOS arm64/
x86_64, Windows x86_64, Linux x86_64), numa thread em segundo plano (mesmo
padrão thread+canal que o scrape usava, agora pro download). `xperience.rs`
não trava mais de cara sem um core — a checagem que existia em
`parse_args()` saiu de lá; agora o app abre normal (tela inicial → estante
→ configurações) mesmo sem núcleo nenhum, e só avisa na hora de efetivamente
jogar.

Ver também `docs/fase-3.md` (a moldura trava em 16:9 num monitor ultrawide,
mesma revisão) e `docs/fase-4.md` (o resto do painel/botões).

## Revisão (2026-09-14, patch 0.4.1): raiz do macOS vira `~/Documents`

O `app_root()` descrito acima (subir do bundle até o `.app`) resolvia pra
dentro de `/Aplicativos` depois de instalar pelo DMG — não é gravável/
esperado nesse SO escrever dados de usuário ali. `dirs::app_root()` no
macOS agora ignora a localização do executável por completo e usa sempre
`~/Documents/SNES Xperience` (criada no primeiro uso); a detecção de
bundle `.app`/`Contents/MacOS` saiu de `dirs.rs`. Windows/Linux não
mudaram — continuam com a pasta ao lado do executável, que já é gravável e
óbvia nesses dois SOs. `xperience.rs::migrate_old_data` ganhou uma segunda
migração, só macOS: se a raiz antiga ao lado do `.app` (mesma detecção de
bundle, agora só usada aqui) tiver `roms/`/`core/`/`assets/`/`saves/`/
`notes/` e a nova em `~/Documents` ainda estiver vazia, copia uma vez —
quem já tinha rodado o DMG 0.4.0 não perde ROMs/progresso.
