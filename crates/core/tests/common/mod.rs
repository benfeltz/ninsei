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

/// The builtin data with one more net's AI in `ai.ron`: model `probe` (a sentry, cpu 1, ram 2,
/// `verbs` as a model writes them, roomy HDD, every numbered verb 0 to 999) and its version 1.0
/// running `program` (and a last `WAIT`, which a fight passes over: every version walks). Its
/// row in `Data::ice` is `probe-1.0`.
#[allow(dead_code)]
pub fn with_probe_ai(verbs: &str, program: &str) -> Result<Data, String> {
    let ranges = verbs
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "Segfault")
        .map(|v| format!("({v}, 0, 999)"))
        .collect::<Vec<_>>()
        .join(", ");
    let ai = include_str!("../../../../data/ai.ron");
    let model = format!(
        "models: [\n        (id: \"probe\", company: \"Probe\", name: \"PROBE\", class: Sentry, \
         cpu: 1, ram: 2, hdd: 40, verbs: [{verbs}], ranges: [{ranges}], walks: [Wait]),"
    );
    let version = format!(
        "versions: [\n        (model: \"probe\", version: \"1.0\", strength: 10, program: \"{program}\\n63999 WAIT\"),"
    );
    let ai = ai
        .replacen("models: [", &model, 1)
        .replacen("versions: [", &version, 1);
    Data::parse(Sources {
        ai: &ai,
        ..Sources::builtin()
    })
}
