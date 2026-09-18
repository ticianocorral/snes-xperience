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

## libretro-database — pasta `cht` (Fase 4, revisão)

`crates/domain/src/cheats_data.txt` embute a pasta `cht` inteira do
[`libretro-database`](https://github.com/libretro/libretro-database) pro
SNES — `github.com/libretro/libretro-database/tree/master/cht/Nintendo%20-%20Super%20Nintendo%20Entertainment%20System`,
gerado por `scripts/gen_cheats_data.py` (revisão: antes era uma lista
curada de ~20 jogos escolhidos a dedo; agora é a base toda, ~2400 jogos,
pra cobrir qualquer ROM que o jogador adicionar, não só as poucas
testadas na hora). (O plano, §4.4, citava MIT de memória — o `LICENSE` do
repositório é na verdade **CC BY-SA 4.0**; corrigido aqui.)

Diferente da revisão anterior deste arquivo: desta vez tanto o código de
cada cheat (endereço/valor, fato bruto sem expressão autoral — do jeito
que uma lista de números de telefone não vira obra protegida por estar
arrumada numa tabela) quanto a **descrição** são reproduzidos como estão
na base, sem reescrever — na escala de milhares de linhas, reescrever
cada uma não seria viável nem teria sentido (são rótulos factuais curtos,
não prosa). Por isso `cheats_data.txt` em si é distribuído sob a mesma
licença da base (CC BY-SA 4.0 — o *share-alike* da licença já pede isso
de qualquer coleção derivada; ver `scripts/gen_cheats_data.py` pra como
foi gerado, prova de que não é uma cópia opaca).

> Cheats (códigos e descrições) adaptados de `libretro-database`
> (github.com/libretro/libretro-database), licenciado sob
> [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).
> Copyright dos respectivos colaboradores do projeto libretro.
> `crates/domain/src/cheats_data.txt` é distribuído sob os mesmos termos.

## TOSEC — ano/editora embutidos (Fase 4, revisão)

`crates/domain/src/tosec_data.txt` embute um recorte do datfile SNES
"Games" do [TOSEC](https://www.tosecdev.org/) (The Old School Emulation
Center), gerado por `scripts/gen_tosec_data.py`. Só duas informações são
guardadas por CRC32: **ano** e **editora** — extraídas do nome que o
TOSEC dá a cada jogo (o formato usa parênteses, ex.
`"Chrono Trigger (1995)(Square)(US)"`), não copiadas do dat de outra
forma. O TOSEC não tem campos `<year>`/`<publisher>` próprios como um dat
Logiqx/No-Intro — o nome catalogado É a única fonte dessas duas datas.

Diferente da base de cheats acima, o TOSEC **não publica uma licença
explícita** para o conteúdo dos seus datfiles (o GPL mencionado no site é
da ferramenta de CMS Joomla, não dos dados). Por isso, só os dois fatos
nus (um ano, um nome de editora) são embutidos — nunca o nome catalogado
do TOSEC nem sua descrição completa; o título mostrado na estante e no
painel do app continua vindo do cabeçalho da própria ROM ou de um DAT
No-Intro opcional, exatamente como antes. Um DAT No-Intro do usuário, se
carregado, sempre tem prioridade sobre esse ano/editora embutido quando
os dois concordam em ter a mesma informação (`catalog.rs::info_lines`).

**Cobertura ampliada via DAT-o-MATIC (No-Intro), sem embutir nada dele.**
Um dump específico (ex. uma revisão "(Rev 1)") às vezes não bate com
nenhuma entrada catalogada pelo TOSEC por CRC32, mesmo quando o TOSEC
claramente conhece o jogo sob outro dump. `scripts/gen_tosec_data.py`
aceita opcionalmente o dat oficial do SNES do
[No-Intro](https://datomatic.no-intro.org/) (baixado manualmente pelo
site — sem link direto, mesma situação de sempre) só para ler a relação
`id`/`cloneofid` de cada `<game>` — "estes CRC32s são revisões/regiões do
mesmo jogo" — e propagar o ano/editora já extraído do TOSEC para outros
CRC32s da mesma família que o TOSEC não catalogou sozinho. Nenhum nome,
descrição ou outro texto do No-Intro é lido para o arquivo final; o valor
gravado continua sendo 100% o ano/editora que o TOSEC forneceu para
algum membro da família, só aplicado a mais hashes.



## Marcas

"Super Nintendo", "Super Famicom", "SNES" e o trade dress do console são marcas
da Nintendo. O console na cena é "inspirado em", não uma réplica (plano §7).
