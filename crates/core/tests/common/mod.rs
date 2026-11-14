//! Helpers more than one of the core's test files share.

use ninsei_core::{Data, Sources};

/// The builtin ICE with one more row, `probe`: `head` gives its class, tier, strength, cpu and
/// ram as RON fields, then its `verbs` (as a row writes them) and `program`. Its HDD is roomy and
/// every verb that takes a number takes 0 to 999, so only the program is on trial.
pub fn with_probe_row(head: &str, verbs: &str, program: &str) -> Result<Data, String> {
    let ranges = verbs
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "Segfault")
        .map(|v| format!("({v}, 0, 999)"))
        .collect::<Vec<_>>()
        .join(", ");
    let ice = include_str!("../../../../data/ice.ron");
    let end = ice.rfind(']').unwrap();
    let row = format!(
        "    (id: \"probe\", name: \"Probe\", {head}, verbs: [{verbs}], hdd: 40, \
         ranges: [{ranges}], program: \"{program}\"),\n"
    );
    let ice = format!("{}{row}{}", &ice[..end], &ice[end..]);
    Data::parse(Sources {
        ice: &ice,
        ..Sources::builtin()
    })
}
