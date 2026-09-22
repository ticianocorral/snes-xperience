# Plano — RetroAchievements no SNES Xperience

Conquistas do [RetroAchievements](https://retroachievements.org) no app: login
na configuração, identificação do jogo pelo hash da ROM, avaliação das
condições em tempo real contra a RAM do core snes9x, e a notificação de
"CONQUISTA DESBLOQUEADA" no queixo da TV — design já validado em
`docs/mocks/ra-notificacao*.png` (mock do `examples/ra_osd_mock.rs`).

O plano é honesto sobre o tamanho da coisa: RetroAchievements é o recurso
mais parecido com "um RetroArch inteiro dentro do app" que este projeto já
encarou. Por isso as fases são independentes — cada uma entrega valor sozinha
e nenhuma derruba o que existe. **Se a fase 2 terminar e as fases 3–5 parecer
desproporcionais, o plano aceita parar** com login + identificação + lista de
conquistas no painel (só leitura), sem o runtime de desbloqueio.

## O que já existe (e será reaproveitado)

- **Mock da notificação**: `Cabinet::set_demo_chin_osd` + `DEMO_BADGE_IMG`
  desenham o bloco "CONQUISTA DESBLOQUEADA / nome / pontos" no lado direito
  do queixo, com badge 64×64 e truncagem para não colidir com o nameplate.
  A fase 4 substitui o `demo_osd` por uma fila de OSD com tempo; a função de
  desenho (`draw_demo_chin_osd`) migra quase intacta (perde o "demo").
- **Acesso à RAM do core**: `xperience_emulation::Core` já resolve
  `retro_get_memory_data/size` (`sys.rs`) — é o mesmo caminho que o SRAM
  usa. Falta expor `RETRO_MEMORY_SYSTEM_RAM` (WRAM) num formato que o
  avaliador consiga ler por frame sem cópia.
- **HTTP + thread + mpsc**: `core_update`/`dat_update`/`update_check`
  definem o padrão (ureq, worker em thread, `try_recv` no loop de frames).
- **Tela de configurações em seções**: ganha a seção "conquistas" (usuário,
  hardcore, ativar/desativar) do mesmo jeito que "vídeo"/"sistema".
- **DAT No-Intro**: a identificação por CRC32 já existe no catálogo — mas o
  RA **não usa CRC32**, usa um hash próprio (ver fase 2).

## Fase 1 — Conta (configurações + nada além)

- Campo "usuário" e "token de web API" na seção conquistas (o RA não aceita
  login por senha em clientes de terceiros: o usuário gera o token em
  *Settings → Web API* no site). Campos no `Config`, persistidos no
  `xperience.cfg` como o resto — o token é sensível, o arquivo já é local.
- Botão "testar login" que chama `API_GetUserProfile` numa thread e mostra
  "ok (usuário)" / erro no próprio botão, mesmo padrão de feedback do
  "Baixar núcleo".
- Sem token configurado: **tudo** que segue fica desligado — nenhuma
  chamada de rede, nenhum menu novo. O RA é 100% opt-in.
- Sem login não há "hardcore": o conceito nem aparece.

## Fase 2 — Identificação do jogo (só leitura)

- **Hash**: o RA identifica SNES por um hash próprio do ROM (`rhash` do
  rcheevos: SHA1 sobre o banco de ROM ignorando header/trailer conforme o
  mapeamento Hi/Lo/Ex). Implementação: algoritmo portado para Rust direto
  (é pequeno e documentado), **sem** vendorizar C nesta fase; o SHA1 já tem
  no catálogo (`RomId`), o que falta é o pré-processamento por mapeamento.
- `API_GetGameID` (via hash) → `API_GetGameExtended` devolve título, lista
  de conquistas (id, título, descrição, pontos, badge URL, e — importante —
  o **set de condições em `rcheevos`-format** que a fase 3 avalia).
- Cache em `saves/ra-cache/` por hash: a lista de conquistas e os badges
  (PNG 64×64 baixados uma vez, decodificados com `image` como as capas).
- Superfície mínima de UI: no painel lateral da estante, campo
  "conquistas: 0/58" para o jogo em foco (dado do cache; sem runtime ainda).
  Ponto de parada natural: **o plano vale até aqui** se o runtime for
  julgado desproporcional.

## Fase 3 — Runtime de avaliação (o coração, e o mais delicado)

- **rcheevos vendorizado** (`crates/ra/vendor/rcheevos/`, build via `cc`
  crate, licença zlib — mesma família do que o projeto já embute). Tentativa
  de reescrever o avaliador de condições em Rust é explicitamente
  **descartada**: a gramática de condições (addsub/hits/pause/reset,
  memtypes, alt groups) é o tipo de coisa que só re-implementa errado.
- Bindings mínimos (`crates/ra/src/`, `-sys` + wrapper seguro):
  `rc_runtime_init/load_backup/new_game(avaliar o buffer de definições)/
  tick(memória, len) → unlocks`. O runtime do rcheevos já resolve serializar
  progresso (`.rap` do jogo) — guardar em `saves/`.
- **Memória**: por frame, ponteiro+ tamanho da `SYSTEM_RAM` do snes9x via
  `retro_get_memory_data` (o rcheevos do RetroArch usa exatamente isso para
  SNES; os endereços dos sets já vêm nesse espaço). Avaliação a cada frame é
  barata (o rcheevos é incremental); se medir caro, avaliar a cada N frames
  — os sets de SNES toleram.
- **Hardcore** (default: ligado quando há login): desliga cheats, save/load
  state, run-ahead especulativo e rewind do livro de pausa durante a
  sessão — mesmas travas do RetroArch. Configurável, mas desligar hardcore
  vale conquistas "softcore" (o RA marca sozinho pelo flag do submit).
- **Envio**: `API_AwardAchievement` em thread (fila + retry curto;
  falha de rede não perde o unlock — fica pendente no `.rap` e reenvia).
- **Login em jogo já rodando**: não — identificação acontece ao inserir o
  cartucho (fluxo natural do app); trocar de jogo ejetar/reinserir como
  sempre.

## Fase 4 — Notificação e painel (a parte visível)

- **Fila de OSD no queixo** (substitui o `demo_osd`): `Cabinet::push_osd
  (título, pontos, badge_id, ttl ~6s)`; o loop de composição desenha o
  primeiro da fila no bloco direito do queixo e expira por tempo — mesmo
  desenho do mock, incluindo truncagem contra o nameplate e a sombra de
  2px. Duas notificações seguidas enfileiram, não se sobrepõem.
- Som de unlock: um "jingle" curto (Pixabay, como os foleys do console),
  respeitando o volume master.
- **Livro de pausa**: aba "Conquistas" ao lado de anotações/cheats — lista
  do jogo com badge, nome, pontos e estado (desbloqueada em verde /
  oculta como "???" quando o set pede segredo / bloqueada), rolagem igual à
  das notas. No hardcore, a aba cheats mostra o aviso em vez da lista.
- **Painel do jogo**: "conquistas 12/58" atualiza ao vivo.

## Fase 5 — Ajustes finos

- Sincronia ao retomar (ejetar + reinserir o mesmo jogo recarrega o `.rap`).
- Percentual no painel, ordem por "mais próxima de desbloquear" (o
  `API_GetGameExtended` dá progresso por conquista quando há sessão).
- Reset de sessão (botão power) re-avalia `reset` conditions — o rcheevos
  tem hook `rc_runtime_reset`; ligar no power do console, não no pausar.

## Riscos e decisões explícitas

- **Ordem das fases é por risco**: 1–2 são rede+cache (padrão conhecido);
  3 é a única com C vendorizado e semântica de gameplay; 4–5 só usam o que
  3 expõe. Se 3 estourar o escopo, 1–2 já entregam valor e o mock vira
  "lista de conquistas estática".
- **Run-ahead**: com RA ativo em hardcore, run-ahead desliga (o estado
  especulado validaria condição de hit que o rewind "não aconteceu"). Fora
  do hardcore, fica ligado — avaliação roda só no frame real.
- **Core novo da buildbot**: sets escritos para a RAM do snes9x; se um dia
  o core trocar (ares), o espaço de memória muda e os sets precisam ser
  revistos — documentar no THIRD-PARTY-NOTICES junto das licenças
  (rcheevos zlib, badges/conteúdo © RetroAchievements, só cache local).
- **Termos do RA**: client de terceiros precisa identificar-se com
  User-Agent próprio e respeitar rate limit; token é do usuário, nunca
  coletado pelo app além do cfg local.
- **Testes**: fases 1–2 com mocks de HTTP (padrão ureq já usado nos testes
  `--ignored` de rede real); fase 3 com um set sintético minúsculo (uma
  condição `byte == valor`) rodando contra um core de verdade em headless;
  fase 4 com BMP headless como o resto da UI.

## Ordem de implementação sugerida (commits)

1. `config` + seção conquistas em settings (fase 1).
2. `crates/ra` com hash + identificação + cache + campo no painel (fase 2).
3. vendor rcheevos + bindings + tick no loop de frame + hardcore (fase 3).
4. OSD do queixo de verdade + aba no livro de pausa (fase 4).
5. Ajustes, resets e docs (`docs/fase-5.md` no formato das outras fases).
