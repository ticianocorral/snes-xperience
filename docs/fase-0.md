# Fase 0 — Provar as premissas

Objetivo (plano §6): antes de escrever qualquer linha da moldura, confirmar que
os dois alicerces externos funcionam.

1. O core do snes9x carrega e roda pela API libretro.
2. O ScreenScraper devolve `texture` e `wheel` para uma amostra de ~20 ROMs.

Nada de moldura, seletor ou painel nesta fase.

---

## Prova 1 — `emu-run`

### O que você precisa providenciar

- **O core libretro do snes9x.** Não vem versionado no repositório. Baixe pelo
  buildbot oficial do libretro (`https://buildbot.libretro.com/nightly/`) ou
  instale via RetroArch:
  - macOS: `snes9x_libretro.dylib`
  - Linux: `snes9x_libretro.so`
  - Windows: `snes9x_libretro.dll`

  Neste checkout já existe `cores/snes9x_libretro.dylib` (arm64, Snes9x 1.63),
  baixado do buildbot para o teste local. Confirme que ele carrega:

  ```bash
  cargo run -p xperience-emulation --example probe -- ./cores/snes9x_libretro.dylib
  ```
- **Uma ROM de SNES** à qual você tem direito. Para um teste sem cópias
  protegidas, uma ROM homebrew serve (ex.: as demos livres de `pdroms` ou
  `superfamicom.org/homebrew`).

### Rodar

```bash
cargo run --release --bin emu-run -- \
  --core ~/cores/snes9x_libretro.dylib \
  --rom  ~/roms/minha-rom.sfc \
  --save-dir ./saves
```

Ou defina o core por ambiente: `export XPERIENCE_CORE=~/cores/snes9x_libretro.dylib`.

### Teclas

Só o joypad em si é teclado por padrão (o resto é mouse/gamepad — ver
"Revisão" no fim deste documento):

| Tecla        | Efeito                                          |
|--------------|------------------------------------------------|
| setas        | direcional                                     |
| Z / X        | B / A                                          |
| A / S        | Y / X                                          |
| Q / W        | L / R                                          |
| Enter        | Start                                          |
| Shift dir.   | Select                                         |

Um gamepad conectado é detectado automaticamente e tem prioridade de uso
(plano §3.1). Tela cheia, reset, pausa e sair do `emu-run` são botões
clicáveis no painel/caderno de pausa, não teclas.

### Critério de aprovação

- A janela abre e mostra o jogo rodando a ~60 fps.
- Há som contínuo, sem estouros grosseiros.
- O gamepad controla o jogo.
- A imagem aparece com o visual definitivo: **NTSC RF + tubo CRT**.
- O log inicial mostra a identificação da ROM (crc32/sha1, nome interno,
  LoROM/HiROM) e o `av_info` do core (resolução, fps, sample rate).

### A visualização

Só existe um caminho de apresentação, fixo:

1. **NTSC RF** — o frame cru passa pelo `snes_ntsc` 0.2.2 do blargg,
   vendorizado em `crates/ntsc/` (LGPL), com o preset `Rf` (composite mais
   sujo, sem merge de campos, fase de burst animada). 256 de largura vira 602.
   O `snes9x_blargg` do core fica desligado.
2. **Tubo CRT** — o resultado é amostrado com bilinear e desenhado por uma
   malha com distorção de barril (`render_geometry`): a imagem estufa como
   tubo de TV, bordas curvas, cantos cortados, vinheta nas quinas. Sem
   scanline.

Os ajustes ficam em constantes no fonte, não em flags:
`crates/platform/src/video.rs` → `CRT_WARP` (curvatura), `CRT_VIGNETTE`,
`CRT_GRID`; `crates/ntsc/src/lib.rs` → `Preset::Rf` (`artifacts`, `fringing`,
`bleed`, `resolution`).

Um shader de CRT completo (scanline, máscara de fósforo) continua previsto
para a Fase 3.

---

## Prova 2 — `scrape-test`

### Credenciais

O ScreenScraper exige um par de credenciais de desenvolvedor. Registre-se em
`https://www.screenscraper.fr/` e exporte:

```bash
export SS_DEVID=seu_devid
export SS_DEVPASSWORD=seu_devpassword
export SS_SOFTNAME=snes-xperience        # opcional
export SS_USER=sua_conta                 # opcional, cota maior (plano §4.2)
export SS_PASSWORD=sua_senha             # opcional
```

### Rodar

```bash
cargo run --bin scrape-test -- --roms ~/roms --limit 20
```

Para cada ROM ele calcula CRC32/MD5/SHA1 (sem o header de 512 bytes, como os
DATs do No-Intro), consulta `jeuInfos.php` casando por hash + tamanho + nome, e
imprime uma linha: arquivo, nome canônico, tem `texture`?, tem `wheel`?

### Critério de aprovação

- Pelo menos a maioria das ROMs da amostra casa com um jogo.
- A coluna final resume quantas trouxeram **as duas** mídias.
- A linha de cota mostra quantas requisições ainda restam no dia.

Se muitas ROMs não casarem, quase sempre é ROM com header/trimada ou nome de
arquivo muito fora do padrão — anote quais para o fallback da Fase 2.

---

## Depois da Fase 0

Com as duas provas passando, seguir para a Fase 1 (plano §6): emulador feio que
funciona — save state, run-ahead, configuração de input, sem moldura.
O laço em `xperience-emulation` já expõe `save_state` / `load_state` para o
run-ahead começar cedo.

## Revisão (2026-09-14): ScreenScraper removido, `scrape-test` também

A Prova 2 documentada acima provou que dava pra casar ROM com metadado
online — mas o app virou portátil (sem banco) e sem raspagem (capa/logo
agora são arte local em `assets/`, nome vem de um DAT No-Intro local, ver
`docs/fase-2.md`). `scrape-test` não tem mais nada pra testar e foi
removido junto do `Client`/`GameInfo` do ScreenScraper. Fica como registro
histórico de que a integração funcionou, não como binário vivo.

## Revisão (2026-09-14): sem atalho de teclado pra comandos do console

Pedido do usuário: nenhum comando do console (ligar/desligar, ejetar,
reset, pausa, save/load state, slot, turbo, screenshot, nota, cheats) usa
mais tecla nenhuma — viram só botão clicável no painel lateral (ou no
caderno de pausa, uma vez pausado), com navegação por mouse ou gamepad em
todo menu (estante, configurações, tela inicial). Ver `docs/fase-3.md`/
`fase-4.md`, seções "Revisão", pelo detalhe de cada tela. O único teclado
que sobrou por padrão é o joypad em si (tabela acima) — continua
existindo porque nem todo mundo tem um gamepad plugado pra jogar.
