# SNES Xperience

Emulador de SNES com moldura estática — projeto pessoal, sem fins comerciais.
Ver [`docs/plano-emulador-moldura.md`](docs/plano-emulador-moldura.md) para o
desenho completo, e `docs/fase-0.md` … `docs/fase-3.md` para o que já foi feito.

Estado atual: **Fases 0–2 prontas, Fase 3 em andamento.** `emu-run` roda uma ROM
com vídeo, som, gamepad, save state, SRAM e run-ahead, visual fixo NTSC RF + tubo
CRT. A Fase 2 entregou o catálogo (`library`), a estante na tela (`selector`:
capas, navegação por gamepad, busca, scrape sob demanda, ficha com logo e sinopse
rolante) e o binário `xperience`, que junta estante → jogo → estante num processo
só. Da Fase 3 já existem o gabinete escuro atrás do tubo, a janela única (estante
e jogo no mesmo gabinete, sem recriar), a estante deformada pelo mesmo tubo do
jogo, a sequência de sinal off entre as telas e o cartucho no slot com o rótulo.
Falta o teste de duas horas (em andamento) e os botões do console com trava de
ejeção.

## Arquitetura

Quatro camadas, dependências só para baixo (plano §2):

| Camada        | Crate                | Responsabilidade                                        |
|---------------|----------------------|--------------------------------------------------------|
| Apresentação  | `xperience-app`      | binários (`xperience`, `emu-run`, `selector`, `library`, `scrape-test`) + módulos `runner` / `shelf` que eles compartilham |
| Domínio       | `xperience-domain`   | identificação de ROM, catálogo SQLite, ScreenScraper   |
| Emulação      | `xperience-emulation`| core libretro carregado em runtime, laço de execução   |
| Plataforma    | `xperience-platform` | SDL3: o `Cabinet` (janela única — gabinete, tubo CRT, camada 2D do seletor), áudio, gamepad |

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

## Jogar

```bash
# uma vez: montar o catálogo a partir de uma pasta de ROMs
cargo run --bin library -- scan --roms /caminho/para/roms

# estante → jogo → estante, um processo só
cargo run --bin xperience -- --core /caminho/snes9x_libretro.dylib
```

`Esc` no jogo volta pra estante; `Esc` (ou fechar a janela) na estante encerra.
Catálogo e saves ficam em `~/.local/share/snes-xperience/`. Para rodar uma ROM
solta sem catálogo, use `emu-run` (ver Fase 0).

Com `SS_DEVID` / `SS_DEVPASSWORD` (ScreenScraper) no ambiente, a estante busca a
ficha e a capa do jogo em foco na hora; `--no-scrape` desliga. Para preencher o
catálogo inteiro de uma vez, `library scrape`.

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
