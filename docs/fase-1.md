# Fase 1 — Emulador feio que funciona

O plano (§6): carrega ROM, roda, som, gamepad, save state, tela cheia. Sem
moldura, sem seletor. Os "três modos de escala" do plano foram substituídos pela
visualização única fixa (RF NTSC + tubo CRT) decidida na Fase 0.

Tudo mora em `emu-run`. Rodar é igual à Fase 0:

```bash
cargo run --release --bin emu-run -- \
  --core ~/cores/snes9x_libretro.dylib \
  --rom  ~/roms/jogo.sfc \
  --save-dir ./saves \
  --runahead 1
```

## Save states

`crates/emulation` expõe `retro_serialize` / `retro_unserialize` como
`Core::save_state` / `load_state` (e `save_state_into` para reusar buffer).

No `emu-run`, **10 slots** (`0`–`9`), arquivo
`<save-dir>/<sha1-da-rom>.state<n>`:

| Tecla | Efeito |
|---|---|
| F2 | grava no slot atual |
| F4 | recarrega o slot atual |
| `]` / `[` | próximo / anterior slot |

Indexado pelo **SHA1 da ROM** (plano §3.4: "Indexe pelo hash da ROM, nunca pelo
nome do arquivo"). ROM não identificada ⇒ sem slot, o app avisa.

Verificação: `cargo run -p xperience-emulation --example state_check -- <core> <rom>`
roda até o frame 600 por dois caminhos (direto, e com save no 300 + reload) e
compara o hash do frame. Para o snes9x o estado tem ~820 KiB.

## SRAM de bateria

`Core::sram()` / `load_sram()` sobre `retro_get_memory_data(RETRO_MEMORY_SAVE_RAM)`.

- Ao carregar: se existir `<save-dir>/<sha1>.srm`, é injetado no core.
- Durante o jogo: a cada ~10 s (`SRAM_FLUSH_FRAMES`) grava se mudou.
- Ao sair: flush final.

O snes9x não persiste `.srm` sozinho — é o frontend que lê e grava. SMW tem
2048 bytes de SRAM.

## Run-ahead

Plano §2: "Implemente run-ahead cedo." Barato porque o save state do snes9x é
rápido.

Por frame exibido, com `--runahead N` (padrão 1):

1. `core.run()` — avança o estado real; áudio desse frame vai pro dispositivo.
2. `save_state_into(buf)`.
3. `core.run()` × N — frames especulativos; **áudio descartado**.
4. Mostra o último frame especulativo.
5. `load_state(buf)` — rebobina pro estado real.

O input lido no início do loop alimenta os dois `run()`, então o jogador vê o
efeito do comando N frames antes. Custo: ~2× emulação + 1 save + 1 load por
frame (823 KiB → ~50 MB/s cada lado; irrelevante).

`--runahead 0` desliga. `--runahead` na linha de comando sobrepõe o config. Se o
core não serializa, desliga sozinho com aviso.

## Config (`config.toml`)

Ordem de busca: `--config PATH`, `$XPERIENCE_CONFIG`,
`$HOME/.config/snes-xperience/config.toml`. Se o último não existir, um arquivo
padrão comentado é escrito lá.

```toml
runahead = 1
fullscreen = false

[keyboard]
b = "Z"          # nome de tecla do SDL: "Left Shift", "F2", "]", ...
start = "Return"
save_state = "F2"
slot_next = "]"
# ...
```

Ações do teclado: os 12 botões (`up down left right a b x y l r select start`) e
`fullscreen reset pause save_state load_state screenshot slot_next slot_prev
frame_step fast_forward`. `Esc` (sair) é fixo. Gamepad **não** é remapeável — a
base de controllers do SDL já normaliza os aparelhos (plano §3.1).

## Dois jogadores

Jogador 1 = teclado **ou** o 1º gamepad; jogador 2 = o 2º gamepad. `Platform`
mantém até `MAX_PORTS` (2) gamepads abertos e reabre a lista em qualquer evento
de conexão/desconexão. `Input::held(port, botão)`.

## Fast-forward e frame-step

- **Tab** (segurar) — roda `FF_SPEED` (8) frames emulados por frame exibido, sem
  áudio e sem run-ahead; o limitador de fps é solto enquanto está segurado.
- **`\`** — com o jogo **pausado** (`P`), avança exatamente um frame.

## Outras utilidades

- **F12** — screenshot do quadro composto (tubo + NTSC) em
  `<save-dir>/shot-<epoch>.bmp`.
- **Guarda de áudio** — a fila do SDL nunca passa de ~0,15 s à frente; se
  encher, o áudio do frame é descartado (evita latência acumulando numa sessão
  longa — o teste de 2 h do plano §9).
- Título da janela = nome do arquivo da ROM.

## Fora do escopo da Fase 1

- Seletor, moldura, painel — Fases 2 a 4.
- Remap de gamepad e múltiplos perfis de config.
- OSD / menu na tela (por enquanto o feedback é no log).
