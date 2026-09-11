//! A small, hand-picked slice of the libretro-database `cht` collection
//! (CC BY-SA 4.0 — see `THIRD-PARTY-NOTICES.md`; the plan's "MIT" was a
//! misremembering), embedded so the app needs no network at runtime (plan
//! §4.4). Only the *codes* come from there — an address/value pair is a
//! fact, not a creative work. Every description below is written for this
//! app, not copied from the database's own.
//!
//! Matched by the SNES cartridge header's internal title (21 bytes, already
//! trimmed by [`crate::rom::RomId`]), not the ROM file name or hash — the
//! header survives a re-dump or a rename that a hash or filename wouldn't.

/// One cheat: a short description and the code string passed straight
/// through to the core's `retro_cheat_set` (raw hex, Game Genie or Pro
/// Action Replay — whatever format the database shipped it in).
pub struct CheatDef {
    pub desc: &'static str,
    pub code: &'static str,
}

/// Cheats curated for `internal_name` (case-insensitive, trimmed), or an
/// empty slice if we haven't picked any for that cartridge yet.
pub fn for_title(internal_name: &str) -> &'static [CheatDef] {
    let key = internal_name.trim().to_ascii_uppercase();
    for (title, defs) in TABLE {
        if *title == key {
            return defs;
        }
    }
    &[]
}

macro_rules! cheats {
    ($($desc:literal => $code:literal),* $(,)?) => {
        &[$(CheatDef { desc: $desc, code: $code }),*]
    };
}

type Row = (&'static str, &'static [CheatDef]);

// Descriptions are plain ASCII on purpose: the panel's bitmap font
// (`font8x8::legacy`) only covers 0-127, so an accented letter would render
// as `?` — see `cabinet::draw_text_absolute`. Same reason the rest of the
// in-game PT-BR copy ("Desligar", "Ejetar", "comandos"...) never needed one.
const TABLE: &[Row] = &[
    (
        "ALADDIN",
        cheats![
            "Invencibilidade total" => "7E034704",
            "Vidas no maximo" => "7E03640A",
            "Gemas no maximo" => "7E036B99",
        ],
    ),
    (
        "DONKEY KONG COUNTRY",
        cheats![
            "Vida infinita" => "7E057901",
            "Vidas infinitas" => "7E057763",
            "999 bananas" => "7E052C09+7E052D09",
        ],
    ),
    (
        "DIDDY'S KONG QUEST",
        cheats![
            "255 vidas ao comecar" => "EE65-3D67",
            "Pulo turbinado do Diddy" => "EDD0-735A",
            "Mais moedas Kong ao comecar" => "626D-4EBD",
        ],
    ),
    (
        "DONKEY KONG COUNTRY 3",
        cheats![
            "Invencivel" => "B768-C34D",
            "Vidas infinitas" => "C26E-73CD",
            "Loja nao cobra moedas de prata" => "8023-EACA+C261-83B7",
        ],
    ),
    (
        "SUPER STAR SOCCER",
        cheats![
            "Tempo infinito" => "7E1A4655",
            "Sem faltas (Copa Internacional / Serie Mundial)" => "7E1F9601",
            "Sem impedimento (Copa Internacional / Serie Mundial)" => "7E1F9001",
        ],
    ),
    (
        "KILLER INSTINCT",
        cheats![
            "Vida infinita" => "3DC1-4DA4+DDC1-4FD4+DDC1-4F04",
            "Nocaute de um golpe" => "6DC5-4DD4",
            "Invencibilidade (Jogador 1)" => "3DBB-3F07+DDBB-3F67+EDBB-34D7+A7BB-3407",
        ],
    ),
    (
        "WAR OF THE GEMS",
        cheats![
            "Vida infinita" => "B9D9-74D4",
            "Folego infinito" => "E15A-5F66",
            "Quase invencivel" => "E916-740D",
        ],
    ),
    (
        "MEGAMAN X2",
        cheats![
            "Tanques de vida no maximo" => "7E1FD120",
            "9 vidas infinitas" => "7E1FB309",
            "Giga Crush infinito" => "7E1FCB5C",
        ],
    ),
    (
        "POWER RANGERS",
        cheats![
            "Energia infinita" => "C286-6DF4",
            "Vidas infinitas" => "3CAA-DFDF",
            "Comeca com energia cheia" => "AD82-64DD",
        ],
    ),
    (
        "MORTAL KOMBAT 3 R2.1",
        cheats![
            "Jogador 1 nao apanha" => "7E544701",
            "Libera os itens \"Kool Stuff\"" => "7EEC1C02",
            "Modo hiper para os dois jogadores" =>
                "7E017600+7E027600+7E037600+7E047600+7E057600+7E067600+7E077600+7E087600+7E097600+7E0A7600+7E0B7600+7E0C7600+7E0D7600",
        ],
    ),
    (
        "THE NINJAWARRIORS",
        cheats![
            "Vida infinita" => "7E18B2C8",
            "Tempo infinito" => "62C4-6DD7",
            "Invencivel" => "7E191202",
        ],
    ),
    (
        "ROCK N' ROLL RACING",
        cheats![
            "Comeca com $990.000" => "BBCF-CDD5",
            "Armas frontais infinitas" => "C2BF-476F",
            "Sem dano ao bater nos outros carros" => "3CE5-CD67",
        ],
    ),
    (
        "STREET FIGHTER2 TURBO",
        cheats![
            "Vida infinita (Jogador 1)" => "7E05307F",
            "Tempo infinito" => "DDA5-7F04",
            "Jogadores invisiveis" => "8E62-87A9",
        ],
    ),
    (
        "SUNSET RIDERS",
        cheats![
            "Vidas infinitas" => "7E1FBA03",
            "Invencibilidade" => "7E014010",
            "Nocaute de um tiro" => "6D64-34A9",
        ],
    ),
    (
        "SUPER MARIOWORLD",
        cheats![
            "Moedas sempre no maximo" => "7E0DBF63",
            "99 vidas infinitas" => "7E0DBE63",
            "Cronometro sempre em 999" => "7E0F3109+7E0F3209+7E0F3309",
        ],
    ),
    (
        "YOSHI'S ISLAND",
        cheats![
            "Vidas infinitas, volta pro anel do meio" => "C2EE-64BF",
            "Estrelas carregam no maximo" => "4A32-6DDD+DF32-6D0D",
            "Comeca com 99 vidas" => "17B7-0023",
        ],
    ),
    (
        "SUPER METROID",
        cheats![
            "Imune a ataques inimigos" => "7E18A801",
            "Vida infinita" => "7E09C2DB+7E09C305",
            "999 misseis infinitos" => "7E09C6E7+7E09C703",
        ],
    ),
    (
        "T.M.N.T. 4",
        cheats![
            "Vida infinita" => "7E044A56",
            "Invencibilidade total" => "7E046E1C",
            "Vidas infinitas" => "DDAC-6F67",
        ],
    ),
    (
        "TOP GEAR",
        cheats![
            "Combustivel infinito (Jogador 1)" => "C225-6429",
            "Nitro infinito (Jogador 1)" => "3C84-6D64",
            "Comeca com 9 nitros" => "DB63-6DDD",
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::for_title;

    #[test]
    fn matches_case_and_whitespace_insensitively() {
        assert_eq!(for_title(" aladdin ").len(), 3);
        assert_eq!(for_title("ALADDIN").len(), 3);
    }

    #[test]
    fn unknown_title_has_no_cheats() {
        assert!(for_title("SOME HOMEBREW").is_empty());
    }
}
