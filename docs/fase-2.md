# Fase 2 — O seletor

Plano §6: varredura de pasta, identificação por hash, busca de metadados,
estante com capas, navegação por gamepad, preenchimento progressivo.

Progresso:

- [x] **Varredura + hash + catálogo** (este documento)
- [ ] **Busca de metadados sob demanda** — parcial: já há o `scrape` em lote
- [ ] **Estante com capas** (UI)
- [ ] **Navegação por gamepad**
- [ ] **Preenchimento progressivo** (placeholder → capa conforme chega)
- [ ] **Busca por digitação**

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

## A seguir

A UI da estante (SDL3 + `sdl3-image` para as capas), navegação por gamepad,
placeholder → capa progressivo, e um binário que lança o jogo escolhido
reaproveitando o laço do `emu-run` (refactor previsto).
