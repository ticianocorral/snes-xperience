# SNES Xperience

Emulador de SNES com moldura estática — projeto pessoal, sem fins comerciais.
Ver [`plano-emulador-moldura.md`](plano-emulador-moldura.md) para o desenho
completo.

Estado atual: **Fase 1 — emulador feio que funciona.** As quatro camadas estão
de pé; `emu-run` carrega e roda uma ROM com vídeo, som, gamepad, save state,
SRAM de bateria e run-ahead. Visualização fixa: NTSC RF + tubo CRT. Ainda não há
moldura, seletor nem painel. Ver [`docs/fase-0.md`](docs/fase-0.md) e
[`docs/fase-1.md`](docs/fase-1.md).

## Arquitetura

Quatro camadas, dependências só para baixo (plano §2):

| Camada        | Crate                | Responsabilidade                                        |
|---------------|----------------------|--------------------------------------------------------|
| Apresentação  | `xperience-app`      | binários que amarram tudo (`emu-run`, `scrape-test`)   |
| Domínio       | `xperience-domain`   | identificação de ROM, ScreenScraper                    |
| Emulação      | `xperience-emulation`| core libretro carregado em runtime, laço de execução   |
| Plataforma    | `xperience-platform` | SDL3: janela, tubo CRT (`render_geometry`), áudio, gamepad |

`xperience-ntsc` é um crate folha à parte: o `snes_ntsc` do blargg vendorizado.

A camada de emulação não sabe que existe uma moldura; a de plataforma não
conhece o core.

## Compilar

Precisa de Rust estável e do SDL3.

```bash
# macOS
brew install sdl3
# Ubuntu (SDL3 ainda não empacotado em toda distro — ver abaixo)

cargo build
cargo test
```

Se não houver SDL3 no sistema, compile-o junto (precisa de CMake + toolchain C):

```bash
cargo build --features xperience-platform/vendored-sdl
```

## Fase 0

Duas provas, descritas em [`docs/fase-0.md`](docs/fase-0.md):

1. **`emu-run`** — um core libretro (snes9x) carrega, roda uma ROM, com vídeo,
   som, gamepad e os três modos de escala.
   ```bash
   cargo run --bin emu-run -- --core caminho/snes9x_libretro.dylib --rom jogo.sfc
   ```
2. **`scrape-test`** — o ScreenScraper devolve `texture` e `wheel` para uma
   amostra de ROMs.
   ```bash
   export SS_DEVID=... SS_DEVPASSWORD=...
   cargo run --bin scrape-test -- --roms ./roms --limit 20
   ```

Nenhum core, ROM ou credencial é distribuído com o projeto. Ver `docs/fase-0.md`.

## Licenças de terceiros

Ver [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md). Ao distribuir um binário
que carrega o core do snes9x, o texto da licença do snes9x precisa acompanhar
(plano §1).
