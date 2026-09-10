# SNES Xperience

Emulador de SNES com moldura estática — projeto pessoal, sem fins comerciais.
Ver [`docs/plano-emulador-moldura.md`](docs/plano-emulador-moldura.md) para o
desenho completo, e `docs/fase-0.md` … `docs/fase-2.md` para o que já foi feito.

Estado atual: **Fase 2 — o seletor (em andamento).** As Fases 0 e 1 estão
prontas: `emu-run` roda uma ROM com vídeo, som, gamepad, save state, SRAM e
run-ahead, visual fixo NTSC RF + tubo CRT. Da Fase 2 já existe o catálogo
(varredura de pasta, hash, cache SQLite, ScreenScraper) via o binário `library`;
falta a estante na tela. Ainda não há moldura nem painel.

## Arquitetura

Quatro camadas, dependências só para baixo (plano §2):

| Camada        | Crate                | Responsabilidade                                        |
|---------------|----------------------|--------------------------------------------------------|
| Apresentação  | `xperience-app`      | binários (`emu-run`, `library`, `scrape-test`)         |
| Domínio       | `xperience-domain`   | identificação de ROM, catálogo SQLite, ScreenScraper   |
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

1. **`emu-run`** — um core libretro (snes9x) carrega e roda uma ROM com vídeo,
   som, gamepad, save state, SRAM e run-ahead. Visual fixo: NTSC RF + tubo CRT.
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

## Versionamento

Segue [SemVer 2.0.0](https://semver.org/lang/pt-BR/). Todo o workspace compartilha
uma versão (`[workspace.package]` em `Cargo.toml`). Enquanto for `0.x`, a API das
crates e as flags de linha de comando podem mudar entre _minors_ — cada release
é uma tag `vX.Y.Z` e um item no [`CHANGELOG.md`](CHANGELOG.md).

- _major_ (`1.0.0`): reservado para quando a moldura/seletor existirem e a
  interface estabilizar.
- _minor_: novos recursos ou mudança de comportamento observável (hoje, ~uma
  fase do plano).
- _patch_: correções sem mudança de interface.

## Licenças de terceiros

Ver [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md). Ao distribuir um binário
que carrega o core do snes9x, o texto da licença do snes9x precisa acompanhar
(plano §1).
