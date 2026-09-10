# snes_ntsc (vendored)

`snes_ntsc` 0.2.2 by Shay Green (blargg) — <http://www.slack.net/~ant/>.
Licensed **LGPL v2.1 or later** (see the header block in `snes_ntsc.c`).

Files `snes_ntsc.{c,h}`, `snes_ntsc_config.h`, `snes_ntsc_impl.h` are verbatim
from <https://github.com/libretro/snes9x/tree/master/filter>. `shim.c` is ours.

`snes_ntsc_config.h` is left at its defaults: input `SNES_NTSC_RGB16` (5-6-5),
output depth 16. The `Rf` preset in `../../src/lib.rs` is a composite base with
lowered resolution and raised artifacts / fringing / bleed and no field merge —
the classic "RF antenna" look. It is our tuning, not an upstream preset.
