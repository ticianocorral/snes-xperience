# SNES Xperience

Emulador de SNES portátil com um console inteiro desenhado na tela —
gabinete, tubo CRT e painel de controle de verdade — em vez de só uma
janela com o jogo. Projeto pessoal, sem fins comerciais.

![Painel lateral com logo, cartucho, chaves Power/Reset e comandos, tela rodando Street Fighter II Turbo](docs/img/screenshot.png)

## Funcionalidades

- **Gabinete e tubo CRT de verdade**: filtro NTSC RF + moldura em volta da
  tela, travada em 16:9 (faixas pretas nas laterais num monitor ultrawide,
  em vez de esticar a imagem).
- **Painel lateral estilo console**: logo e arte de cartucho do jogo,
  Power e Reset desenhados como chaves gangorra roxas — Power alterna de
  verdade (liga/desliga e retoma de onde parou), Reset é momentâneo (sobe
  e desce sozinho) — com Ejetar entre as duas, que só solta o cartucho com
  o console desligado.
- **Escolher um jogo só insere o cartucho** — o console não liga sozinho,
  igual ao hardware de verdade; você clica Power quando quiser começar.
- **Só mouse e gamepad**: nenhum comando do console (power, reset, save
  state, notas, menus...) usa teclado. A única exceção é escrever uma
  anotação, que por natureza precisa de teclado; o D-pad/botões do próprio
  SNES continuam por teclado por padrão pra quem joga sem controle.
- **Caderno de pausa**: pausar o jogo abre um livro de duas páginas — de
  um lado o interruptor de cada cheat curado e um editor de texto livre
  (até 240 caracteres); do outro, 15 slots fixos de captura de tela por
  jogo, cada um podendo ser **fixado** (evita sobrescrita — "Nota" pula
  pro próximo slot livre) e **nomeado**.
- **App portátil, sem instalação**: sem banco de dados nenhum — tudo em
  pastas ao lado do executável (`roms/`, `core/`, `assets/`, `saves/`,
  `notes/`), ou em `~/Documents/SNES Xperience` no macOS.
- **Sem raspagem online**: capa, logo e arte de cartucho vêm de imagens
  que você mesmo solta em `assets/`; o nome canônico do jogo vem de um DAT
  [No-Intro](https://datomatic.no-intro.org/) local opcional.
- **Núcleo baixa sozinho**: o `snes9x_libretro` vem do buildbot oficial do
  libretro, direto do menu de configurações — não precisa procurar/copiar
  o arquivo à mão.

## Baixar

Pacotes prontos — DMG (macOS), zip com os `.exe` (Windows), AppImage
(Linux), todos com SDL3 já embutido — saem automático a cada release, na
aba [Releases](https://github.com/ticianocorral/snes-xperience/releases).

## Jogar

Na primeira execução o app cria sozinho as pastas `roms/`, `core/`,
`assets/{cover,logo,cartridge}/`, `saves/`, `notes/` e o arquivo
`xperience.cfg`. Coloque suas ROMs em `roms/` e o core do snes9x
(`snes9x_libretro.dylib`/`.so`/`.dll`) em `core/` — ou baixe-o direto pelo
menu de configurações, se preferir.

```bash
cargo run --bin xperience
```

Abre na tela inicial (que também é a tela de "cartucho ejetado" — a mesma
depois de voltar da estante ou ejetar um jogo); clique em "Inserir
cartucho" pra abrir a estante. Sem capa nenhuma em `assets/cover/`, a
estante vira uma lista numerada; solte um `<nome-da-rom>.png` (mesmo nome
do arquivo, sem extensão) em `assets/cover/` ou `assets/logo/` pra dar
capa/logo a um jogo — capas são desenhadas em paisagem, arte de frente
horizontal.

Um `nointro.dat` (DAT XML do No-Intro, "Nintendo - Super Nintendo
Entertainment System") na raiz do app dá o nome canônico do jogo em vez
do nome interno do cabeçalho ou do arquivo — opcional, baixe você mesmo,
o app não busca isso sozinho.

## Compilar

Precisa de Rust estável e do SDL3.

```bash
# macOS
brew install sdl3
cargo build
cargo test
```

Sem SDL3 no sistema, compile-o junto (precisa de CMake + toolchain C):

```bash
cargo build --features xperience-platform/vendored-sdl
```

`emu-run` roda uma ROM solta sem o resto do app (idle/estante/config) —
útil pra testar o core isoladamente:

```bash
cargo run --bin emu-run -- --core caminho/snes9x_libretro.dylib --rom jogo.sfc
```

Nenhum core ou ROM é distribuído com o projeto — ver
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

## Arquitetura

Quatro camadas, dependências só para baixo:

| Camada       | Crate                  | Responsabilidade                                                                          |
| ------------ | ---------------------- | ------------------------------------------------------------------------------------------ |
| Apresentação | `xperience-app`        | binários (`xperience`, `emu-run`, `selector`) + `runner`/`shelf`/`settings`/`core_update`   |
| Domínio      | `xperience-domain`     | identificação de ROM, catálogo (JSON, sem banco), nomeação por DAT No-Intro                 |
| Emulação     | `xperience-emulation`  | core libretro carregado em runtime, laço de execução                                       |
| Plataforma   | `xperience-platform`   | SDL3: o `Cabinet` (janela única — gabinete, tubo CRT, seletor), áudio, gamepad              |

`xperience-ntsc` é um crate folha à parte (o `snes_ntsc` do blargg,
vendorizado). A camada de emulação não sabe que existe uma moldura; a de
plataforma não conhece o core.

Para o histórico completo de implementação, fase a fase, ver
[`docs/plano-emulador-moldura.md`](docs/plano-emulador-moldura.md) e
`docs/fase-0.md` … `docs/fase-4.md`.

## Versionamento

Segue [SemVer 2.0.0](https://semver.org/lang/pt-BR/). Todo o workspace
compartilha uma versão (`[workspace.package]` em `Cargo.toml`); cada
release é uma tag `vX.Y.Z` e um item no [`CHANGELOG.md`](CHANGELOG.md).
Enquanto for `0.x`, a API das crates e as flags de linha de comando podem
mudar entre _minors_.

## Licenças de terceiros

Ver [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md). Ao distribuir um
binário que carrega o core do snes9x, o texto da licença dele precisa
acompanhar.
