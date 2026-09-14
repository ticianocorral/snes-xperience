# SNES Xperience

Emulador de SNES com moldura estática — projeto pessoal, sem fins comerciais.
Ver [`docs/plano-emulador-moldura.md`](docs/plano-emulador-moldura.md) para o
desenho completo, e `docs/fase-0.md` … `docs/fase-4.md` para o que já foi feito.

Estado atual: **Fases 0–3 prontas, Fase 4 em andamento**, mais uma revisão
grande fora da numeração original (ver `docs/fase-2.md`/`fase-3.md`/
`fase-4.md`, seções "Revisão"): o app virou **portátil e autoexecutável** —
sem SQLite, sem ScreenScraper, tudo em pastas ao lado do executável (`roms/`,
`core/`, `assets/`, `saves/`, `notes/`), nomes de jogo resolvidos por um DAT
No-Intro local, capa/logo vêm de arte solta em `assets/` (sem mais raspagem
online), o núcleo snes9x baixa/atualiza pelo próprio menu de configurações,
e a moldura trava em 16:9 (com faixas pretas nas laterais num monitor
ultrawide) em vez de distorcer. `emu-run` roda uma ROM solta com vídeo, som,
gamepad, save state, SRAM e run-ahead, visual fixo NTSC RF + tubo CRT. O
binário `xperience` junta tela inicial (TV fora do ar) → estante → jogo →
tela inicial num processo só, com o gabinete/tubo CRT de verdade, o painel
lateral (logo, comandos clicáveis Power/Ejetar/Reset, cheats com
interruptor, captura de tela pro caderno, tempo de sessão) e a tela de
pausa (`P`) folheando o caderno em página dupla. Falta a escrita de
anotações por teclado e a tabela manual de senhas/dicas.

## Arquitetura

Quatro camadas, dependências só para baixo (plano §2):

| Camada        | Crate                | Responsabilidade                                        |
|---------------|----------------------|--------------------------------------------------------|
| Apresentação  | `xperience-app`      | binários (`xperience`, `emu-run`, `selector`) + módulos `runner` / `shelf` / `settings` / `core_update` que eles compartilham |
| Domínio       | `xperience-domain`   | identificação de ROM, catálogo (JSON, sem banco), nomeação por DAT No-Intro |
| Emulação      | `xperience-emulation`| core libretro carregado em runtime, laço de execução   |
| Plataforma    | `xperience-platform` | SDL3: o `Cabinet` (janela única — gabinete, tubo CRT, camada 2D do seletor), áudio, gamepad |

`xperience-ntsc` é um crate folha à parte: o `snes_ntsc` do blargg vendorizado.

A camada de emulação não sabe que existe uma moldura; a de plataforma não
conhece o core.

## Baixar

Pacotes prontos (DMG pro macOS, zip com os `.exe` pro Windows, AppImage pro
Linux) saem automático a cada tag `vX.Y.Z`, na aba
[Releases](https://github.com/ticianocorral/snes-xperience/releases) — SDL3
já vem embutido, não precisa instalar nada.

**App portátil, sem instalação**: na primeira execução o `xperience` cria, ao
lado do executável (ou ao lado do `.app` no macOS, não dentro dele), as
pastas `roms/` (coloque seus arquivos aí), `core/`, `assets/{cover,logo,
cartridge}/`, `saves/`, `notes/`, mais `xperience.cfg` e `library.json`. Sem
`--core`/`$XPERIENCE_CORE`, o app procura `snes9x_libretro.{dylib,so,dll}`
em `core/` — e o menu de configurações (`O` na estante) tem uma opção pra
baixar/atualizar esse core sozinho, direto do buildbot do libretro. Nem o
core nem as ROMs acompanham o pacote (ver
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md)).

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
# coloque as ROMs em roms/, ao lado do binário, e rode:
cargo run --bin xperience
```

Abre direto na tela inicial (TV fora do ar, botão "Inserir cartucho" no
lugar do logo) — confirme/clique pra abrir a estante. Sem capa nenhuma em
`assets/cover/`, a estante vira uma lista numerada estilo menu de multicart;
solte um `<nome-da-rom>.png` (mesmo nome do arquivo da ROM, sem extensão)
em `assets/cover/` ou `assets/logo/` pra dar capa/logo a um jogo.

No jogo: `Esc` desliga (salva, TV em sinal off) e `E` ejeta a partir daí,
voltando pra tela inicial; tentar ejetar ligado só resiste com um "clunk".
`Backspace` reseta o jogo sem sair da tela — os três (Power/Ejetar/Reset)
também são botões clicáveis no painel lateral. `,`/`.` movem o cursor na
lista de cheats do painel (quando o jogo tem algum curado) e `/` liga/
desliga o selecionado. `N` salva a tela atual no caderno daquele jogo
(markdown + PNG em `notes/`). `P` abre o caderno em página dupla no lugar
do jogo congelado; `P` de novo volta a jogar. `Esc` na estante volta pra
tela inicial; `Esc`/fechar a janela na tela inicial, ou fechar a janela do
jogo, encerra o app.

Na estante, `O` abre as **configurações**: controles (rebind, tecla nova
aperta e pronto), núcleo snes9x (baixar/atualizar direto do buildbot do
libretro) e run-ahead/tela cheia. Salva em `xperience.cfg` a cada mudança.

Um `nointro.dat` (DAT XML do [No-Intro](https://datomatic.no-intro.org/),
"Nintendo - Super Nintendo Entertainment System") ao lado do executável dá
o nome canônico do jogo (casado pelo CRC32 do arquivo) em vez do nome
interno do cabeçalho SNES ou do nome do arquivo — opcional, baixe você
mesmo, o app não tem como buscar isso sozinho.

## Fase 0

Prova original, descrita em [`docs/fase-0.md`](docs/fase-0.md): **`emu-run`**
— um core libretro (snes9x) carrega e roda uma ROM com vídeo, som, gamepad,
save state, SRAM e run-ahead. Visual fixo: NTSC RF + tubo CRT.

```bash
cargo run --bin emu-run -- --core caminho/snes9x_libretro.dylib --rom jogo.sfc
```

Nenhum core ou ROM é distribuído com o projeto. Ver `docs/fase-0.md`.

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
