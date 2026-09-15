# SNES Xperience

Emulador de SNES com moldura estática — projeto pessoal, sem fins comerciais.
Ver [`docs/plano-emulador-moldura.md`](docs/plano-emulador-moldura.md) para o
desenho completo, e `docs/fase-0.md` … `docs/fase-4.md` para o que já foi feito.

Estado atual: **Fases 0–3 prontas, Fase 4 em andamento**, mais uma revisão
grande fora da numeração original (ver `docs/fase-0.md`/`fase-2.md`/
`fase-3.md`/`fase-4.md`, seções "Revisão"): o app virou **portátil e
autoexecutável** — sem SQLite, sem ScreenScraper, tudo em pastas numa raiz só
(`roms/`, `core/`, `assets/`, `saves/`, `notes/`) — ao lado do executável no
Windows/Linux, `~/Documents/SNES Xperience` no macOS —, nomes de jogo
resolvidos por um DAT No-Intro local, capa/logo vêm de arte solta em
`assets/` (sem mais raspagem online), o núcleo snes9x baixa/atualiza pelo
próprio menu de configurações, a moldura trava em 16:9 (com faixas pretas
nas laterais num monitor ultrawide) em vez de distorcer, e a fonte de todo o
app é maior e antisserrilhada (Noto Sans Mono, em vez do bitmap 8×8
original). **Nenhum comando usa mais teclado** — só mouse e gamepad, em
toda tela (estante, configurações, tela inicial, jogo, pausa); o único
teclado que sobra por padrão é o joypad em si (D-pad/botões), já que nem
todo mundo tem gamepad plugado pra jogar. `emu-run` roda uma ROM solta com
vídeo, som, gamepad, save state, SRAM e run-ahead, visual fixo NTSC RF +
tubo CRT. O binário `xperience` junta tela inicial → estante → jogo → tela
inicial num processo só — a tela inicial é a mesma tela de "cartucho
ejetado" (logo do console em `assets/console.png` no lugar do logo do
jogo, botão "Inserir cartucho" no lugar da arte de cartucho,
"Configuracoes" no rodapé do painel) —, com o gabinete/tubo CRT de
verdade, o painel lateral (logo + arte de cartucho quando existem,
Power/Reset como chaves gangorra estilo console de verdade — Power
alterna, Reset é momentâneo e volta sozinho —, Ejetar entre elas,
comandos clicáveis — Pausar/Nota/Salvar/Carregar/Slot/Turbo —, cheats
ligados mostrados como informação, tempo de sessão) e uma tela de pausa
interativa (clique ou gamepad) com o caderno em página dupla: a esquerda
tem o interruptor de cheats (clique liga/desliga) e um editor de texto
livre (até 240 caracteres, salvo à parte em `notas.txt`), a direita pagina
pelos 15 slots fixos de captura do jogo — cada um pode ser fixado (evita
sobrescrita, "Nota" pula pro próximo livre) e nomeado —, e embaixo ficam
"Continuar"/
"Avancar quadro". Falta a tabela manual de senhas/dicas.

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

**App portátil, sem instalação**: na primeira execução o `xperience` cria as
pastas `roms/` (coloque seus arquivos aí), `core/`, `assets/{cover,logo,
cartridge}/`, `saves/`, `notes/`, mais `xperience.cfg` e `library.json` numa
raiz só — ao lado do executável no Windows/Linux; no macOS, sempre
`~/Documents/SNES Xperience` (o `.app` em si, tipicamente dentro de
`/Aplicativos`, não é onde o macOS espera dados gravados pelo app). Sem
`--core`/`$XPERIENCE_CORE`, o app procura `snes9x_libretro.{dylib,so,dll}`
em `core/` — e o menu de configurações (botão "Configuracoes" na tela
inicial ou na estante) tem uma opção pra baixar/atualizar esse core
sozinho, direto do buildbot do libretro. Nem o
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

Abre direto na tela inicial — a mesma tela de "cartucho ejetado" (TV fora
do ar, logo do console em `assets/console.png` no lugar do logo do jogo,
"Inserir cartucho" no lugar da arte de cartucho, "Configuracoes" no rodapé
do painel) — confirme/clique em "Inserir cartucho" pra abrir a estante.
Sem capa nenhuma em `assets/cover/`, a estante vira uma lista numerada
estilo menu de multicart; solte um `<nome-da-rom>.png` (mesmo nome do
arquivo da ROM, sem extensão) em `assets/cover/` ou `assets/logo/` pra dar
capa/logo a um jogo — capas são desenhadas em paisagem (a proporção
esperada pra arte de capa horizontal), não retrato.

Escolher um jogo só **insere o cartucho** — não liga o console sozinho,
igual ao hardware de verdade: a tela mostra o cartucho encaixado e o TV
ainda apagado até você clicar Power. Nenhum comando do console usa
teclado — tudo é botão clicável no painel lateral (ou gamepad, em
qualquer menu; só o D-pad/botões do próprio SNES continuam por teclado
por padrão, pra quem joga sem controle). No jogo, o painel tem: Power e
Reset desenhados como as chaves gangorra roxas do console de verdade —
Power alterna (fica pra cima ligado, pra baixo desligado; desligar salva e
vai pro sinal off, ligar de novo retoma exatamente de onde parou), Reset é
momentâneo (sobe com o clique, desce sozinho) —, Ejetar entre as duas (só
sai do slot com o console desligado — ligado, só um "clunk"), Pausar,
Nota (salva a tela atual num dos 15 slots fixos do jogo,
`notes/<nome do jogo>/01.png` .. `15.png`) e Nota slot (cicla qual dos 15
"Nota" grava — sobrescreve o que já tiver lá, mesmo modelo do save
state), Salvar/Carregar (o slot atual) e Slot (cicla entre 10) — mais
logo e arte de cartucho (`assets/cartridge/<rom>.*`, mesma convenção de
nome) quando existem. Cheats curados pro jogo (quando existe) aparecem só
como informação ali (os que estão ligados); pra ligar/desligar de
verdade, clique em Pausar — o caderno de pausa que abre no lugar do jogo
congelado tem o interruptor de cada cheat na página esquerda, mais um
botão "Escrever anotacao" (editor de texto livre, até 240 caracteres,
salvo à parte em `notes/<nome do jogo>/notas.txt`); a página direita
pagina pelos 15 slots ("< anterior"/"proxima >", "vazio" pros que não têm
nada ainda) — "Fixar" trava o slot atual contra sobrescrita (clicar Nota
de novo pula pro próximo slot livre; com os 15 fixados, o próprio botão
Nota avisa "sem espaco") e "Nomear print" dá um título curto ao slot pra
lembrar o que é; embaixo ficam "Continuar" e "Avancar quadro". Na estante,
os botões "Voltar" e "Configuracoes" no rodapé da ficha saem de volta pra
tela inicial ou abrem as configurações; fechar a janela (em qualquer
tela) encerra o app.

As **configurações** (botão "Configuracoes" na tela inicial ou na estante)
têm: controles (rebind do D-pad/botões — clique na ação, aperte a tecla
nova), núcleo snes9x (baixar/atualizar direto do buildbot do libretro) e
run-ahead/tela cheia. Salva em `xperience.cfg` a cada mudança.

Um `nointro.dat` (DAT XML do [No-Intro](https://datomatic.no-intro.org/),
"Nintendo - Super Nintendo Entertainment System") na raiz do app (ver
"App portátil" acima) dá o nome canônico do jogo (casado pelo CRC32 do
arquivo) em vez do nome interno do cabeçalho SNES ou do nome do arquivo —
opcional, baixe você mesmo, o app não tem como buscar isso sozinho.

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
