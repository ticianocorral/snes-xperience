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
  --save-dir ./saves \
  --scale bilinear --ntsc rf   # scale: pixel|bilinear  ntsc: off|rf|composite|svideo|rgb|monochrome
```

Ou defina o core por ambiente: `export XPERIENCE_CORE=~/cores/snes9x_libretro.dylib`.

### Teclas

| Tecla        | Efeito                                          |
|--------------|------------------------------------------------|
| setas        | direcional                                     |
| Z / X        | B / A                                          |
| A / S        | Y / X                                          |
| Q / W        | L / R                                          |
| Enter        | Start                                          |
| Shift dir.   | Select                                         |
| Tab          | alterna a escala (pixel ↔ bilinear)            |
| N            | alterna o filtro NTSC (off/rf/composite/s-video/rgb/mono) |
| F            | tela cheia                                     |
| Backspace    | reset                                          |
| P            | pausa                                          |
| Esc          | sair                                           |

Um gamepad conectado é detectado automaticamente e tem prioridade de uso
(plano §3.1).

### Critério de aprovação

- A janela abre e mostra o jogo rodando a ~60 fps.
- Há som contínuo, sem estouros grosseiros.
- O gamepad controla o jogo.
- `Tab` alterna entre **pixel perfect** e **bilinear + tubo CRT**.
- `N` alterna o filtro Blargg NTSC do core (`composite` = visual RF).
- O log inicial mostra a identificação da ROM (crc32/sha1, nome interno,
  LoROM/HiROM) e o `av_info` do core (resolução, fps, sample rate).

### Modos de escala e filtro

| Modo       | Como funciona                                                        |
|------------|--------------------------------------------------------------------|
| `pixel`    | escala inteira + vizinho mais próximo, centralizado, com barras     |
| `bilinear` | amostragem linear desenhada através de uma **malha com distorção de barril** (`render_geometry`): a imagem estufa como tubo de TV, bordas curvas, cantos cortados e vinheta nas quinas. Sem scanline. |

O filtro **NTSC** é o `snes_ntsc` 0.2.2 do blargg, vendorizado em
`crates/ntsc/` (LGPL). `--ntsc` / tecla `N` alterna
`off → rf → composite → s-video → rgb → monochrome`. O `rf` é o visual de antena
(composite mais sujo, sem merge de campos); a saída de 256 vira 602 de largura,
que a malha do tubo reescala. O `snes9x_blargg` do core fica desligado.

Constantes do tubo em `crates/platform/src/video.rs`: `CRT_WARP` (curvatura),
`CRT_VIGNETTE`, `CRT_GRID`. O `sharp bilinear` e o `crt` placeholder do plano
§4.7 foram removidos a pedido; um shader de CRT completo (scanline, máscara de
fósforo) continua previsto para a Fase 3.

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
