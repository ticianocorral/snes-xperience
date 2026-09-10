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

No `emu-run`:

| Tecla | Efeito |
|---|---|
| F2 | grava o estado em `<save-dir>/<sha1-da-rom>.state` |
| F4 | recarrega esse arquivo |

Um slot só, indexado pelo **SHA1 da ROM** (plano §3.4: "Indexe pelo hash da ROM,
nunca pelo nome do arquivo"). ROM não identificada ⇒ sem slot, o app avisa.

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

`--runahead 0` desliga. Se o core não serializa, desliga sozinho com aviso.

## Fora do escopo da Fase 1 (ainda)

- Remapear controles (arquivo de config) — o mapa de teclado/gamepad é fixo.
- Múltiplos slots de save state.
- Seletor, moldura, painel — Fases 2 a 4.
