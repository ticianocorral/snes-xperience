# Avisos de terceiros

Este projeto **não distribui** nenhum core de emulação, ROM, BIOS ou arte. As
notas abaixo se aplicam quando você monta um binário que carrega esses
componentes.

## snes9x (core libretro)

O caminho de emulação usa o core `snes9x_libretro`. A licença do snes9x permite
uso, cópia, modificação e distribuição — em binário e em código — para fins
**não comerciais**, sem taxa, desde que o aviso de licença e o copyright
acompanhem **todas** as cópias.

Ao publicar um binário do SNES Xperience que embarque ou baixe o core do snes9x,
inclua o texto integral da licença do snes9x junto do binário. O texto oficial
está no repositório do snes9x (`snes9x.h` / `LICENSE`).

Se algum dia o projeto virar produto pago, o core precisa ser trocado — o
substituto natural é o **ares** (licença ISC, v121+).

## SDL3 (zlib)

A camada de plataforma usa [SDL3](https://www.libsdl.org/), sob a licença zlib.

## libretro-database — pasta `cht` (Fase 4)

Os cheats virão da pasta `cht` do `libretro-database` (licença MIT), embutidos
no app. A serem adicionados quando a Fase 4 começar.

## Marcas

"Super Nintendo", "Super Famicom", "SNES" e o trade dress do console são marcas
da Nintendo. O console na cena é "inspirado em", não uma réplica (plano §7).
