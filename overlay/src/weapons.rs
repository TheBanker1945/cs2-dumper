/// Whether left click with this item shoots: false for knives, grenades, the C4 and
/// unknown items.
pub fn is_gun(id: u16) -> bool {
    weapon_info(id).is_some_and(|(_, gun)| gun)
}

/// Display name for an item definition index (`C_EconItemView::m_iItemDefinitionIndex`),
/// and whether the weapon has a magazine worth showing. Designer names can't be used
/// for this: the M4A1-S reports `weapon_m4a1` and the MP5-SD `weapon_mp7`.
pub fn weapon_info(id: u16) -> Option<(&'static str, bool)> {
    let gun = |name| Some((name, true));
    let other = |name| Some((name, false));
    match id {
        1 => gun("Desert Eagle"),
        2 => gun("Dual Berettas"),
        3 => gun("Five-SeveN"),
        4 => gun("Glock-18"),
        7 => gun("AK-47"),
        8 => gun("AUG"),
        9 => gun("AWP"),
        10 => gun("FAMAS"),
        11 => gun("G3SG1"),
        13 => gun("Galil AR"),
        14 => gun("M249"),
        16 => gun("M4A4"),
        17 => gun("MAC-10"),
        19 => gun("P90"),
        23 => gun("MP5-SD"),
        24 => gun("UMP-45"),
        25 => gun("XM1014"),
        26 => gun("PP-Bizon"),
        27 => gun("MAG-7"),
        28 => gun("Negev"),
        29 => gun("Sawed-Off"),
        30 => gun("Tec-9"),
        31 => other("Zeus x27"),
        32 => gun("P2000"),
        33 => gun("MP7"),
        34 => gun("MP9"),
        35 => gun("Nova"),
        36 => gun("P250"),
        38 => gun("SCAR-20"),
        39 => gun("SG 553"),
        40 => gun("SSG 08"),
        41 | 42 | 59 | 80 | 500..=599 => other("Knife"),
        43 => other("Flashbang"),
        44 => other("HE Grenade"),
        45 => other("Smoke"),
        46 => other("Molotov"),
        47 => other("Decoy"),
        48 => other("Incendiary"),
        49 => other("C4"),
        57 => other("Healthshot"),
        60 => gun("M4A1-S"),
        61 => gun("USP-S"),
        63 => gun("CZ75-Auto"),
        64 => gun("R8 Revolver"),
        _ => None,
    }
}
