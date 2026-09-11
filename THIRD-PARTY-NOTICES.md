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

## snes_ntsc (LGPL v2.1+)

`crates/ntsc/vendor/snes_ntsc/` contém `snes_ntsc` 0.2.2 de Shay Green (blargg),
sob **LGPL v2.1 ou posterior**. Fonte verbatim de
`github.com/libretro/snes9x/tree/master/filter`; só `shim.c` é nosso. O preset
`Rf` em `crates/ntsc/src/lib.rs` é uma parametrização nossa (composite + menos
resolução + mais artefato/franja/bleed, sem merge de campos).

A LGPL exige que o usuário possa recompilar/religar essa parte. Como o projeto é
código aberto e não comercial, distribuir o binário junto do fonte cobre isso.
Se um dia virar produto fechado, linkar `snes_ntsc` dinamicamente ou fornecer os
objetos para religação.

## libretro-database — pasta `cht` (Fase 4)

`crates/domain/src/cheats.rs` embute uma lista curada de códigos vindos da
pasta `cht` do [`libretro-database`](https://github.com/libretro/libretro-database),
de `github.com/libretro/libretro-database/tree/master/cht/Nintendo%20-%20Super%20Nintendo%20Entertainment%20System`.
(O plano, §4.4, citava MIT de memória — o `LICENSE` do repositório é na
verdade **CC BY-SA 4.0**; corrigido aqui.)

O que foi usado é só o código de cada cheat — um par endereço/valor (ex.:
`7E034704`), fato bruto sem expressão autoral, do jeito que uma lista de
números de telefone não vira obra protegida por estar arrumada numa tabela.
Nenhuma descrição da base foi copiada: toda descrição em `cheats.rs` foi
escrita para este app. Ainda assim, por transparência e crédito — não porque
achamos que o CC BY-SA prende os códigos em si —, a atribuição:

> Códigos de cheat adaptados de `libretro-database`
> (github.com/libretro/libretro-database), licenciado sob
> [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).
> Copyright dos respectivos colaboradores do projeto libretro.

## Marcas

"Super Nintendo", "Super Famicom", "SNES" e o trade dress do console são marcas
da Nintendo. O console na cena é "inspirado em", não uma réplica (plano §7).
