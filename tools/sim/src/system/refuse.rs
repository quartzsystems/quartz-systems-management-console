// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Refusals in a QS/VS host's words, led by QSI's identifier where QSI
//! names the conflict (QS/VS's `docs/QSISERVER.md`), and the naming
//! rules a host checks.

/// A refusal: the messages, the first first.
pub type Refusal = Vec<(String, String)>;

/// QSI's identifier that leads a QS/VS message in every family:
/// physical I/O's, an entitlement's, a create at an ID, a profile's
/// revision.
fn lead(id: &str) -> Option<&'static str> {
    use qsi::host::host::physio as p;
    use qsi::host::partition::entitlement as e;
    use qsi::host::profiles::msgid as pr;
    Some(match id {
        "QVS2401" => p::UNKNOWN_LOCATION,
        "QVS2402" => p::NOT_RELEASED,
        "QVS2403" => p::HELD,
        "QVS2404" => p::GROUP_SPLIT,
        "QVS2405" | "QVS2406" | "QVS2407" | "QVS2408" => p::NOT_ASSIGNABLE,
        "QVS2420" => p::NOT_A_DRIVE,
        "QVS2421" => p::NOT_VIRTUAL,
        "QVS2016" => qsi::host::partition::msgid::ID_TAKEN,
        "QVS2017" => qsi::host::partition::msgid::ID_OUT_OF_RANGE,
        "QVS2501" => e::NOT_RUNNING,
        "QVS2502" => e::DEDICATED,
        "QVS2503" => e::OUTSIDE_PROFILE,
        "QVS2504" => e::VIRTUALS,
        "QVS2505" => e::POOL_SHORT,
        "QVS2013" => pr::CHANGED,
        "QVS2012" => pr::EXISTS,
        "QVS2015" => pr::NOT_ACTIVE,
        _ => return None,
    })
}

/// QSI's identifier of a pool service's refusal, for the storage pools
/// managed (bit 20).
pub fn pools_lead(id: &str) -> Option<&'static str> {
    use qsi::host::vres::pools as p;
    Some(match id {
        "QVS3001" => p::BAD_NAME,
        "QVS3002" => p::NAME_TAKEN,
        "QVS3003" => p::NO_POOL,
        "QVS3004" => p::DISK_COUNT,
        "QVS3005" => p::NO_DISK,
        "QVS3006" => p::DISK_TAKEN,
        "QVS3007" => p::FOREIGN,
        "QVS3008" => p::TOO_SMALL,
        "QVS3009" => p::BLOCK_SIZES,
        "QVS3010" | "QVS3017" => p::SPARES,
        "QVS3011" => p::TOO_LARGE,
        "QVS3012" => p::NOT_EMPTY,
        "QVS3013" => p::HELD,
        "QVS3014" => p::NOT_USABLE,
        "QVS3015" => p::NOT_FAILED,
        "QVS3016" => p::STEP_FAILED,
        "QVS3019" => p::CANNOT_GROW,
        "QVS3205" => p::MEDIA_IN_USE,
        _ => return None,
    })
}

/// QSI's identifier of a switch service's refusal, for the switches
/// managed (bit 19).
pub fn switches_lead(id: &str) -> Option<&'static str> {
    use qsi::host::vres::switches as w;
    Some(match id {
        "QVS4001" => w::BAD_NAME,
        "QVS4002" => w::NAME_TAKEN,
        "QVS4003" => w::NO_SWITCH,
        "QVS4004" | "QVS4005" => w::BAD_VLAN,
        "QVS4006" | "QVS4105" => w::BAD_MTU,
        "QVS4007" => w::TOO_MANY,
        "QVS4008" | "QVS4009" => w::NAMED,
        "QVS4010" => w::NATIVE_NOT_CARRIED,
        "QVS4012" => w::NOT_BUILT,
        "QVS4015" => w::PORTS_ON,
        "QVS4101" => w::NO_INTERFACE,
        "QVS4102" => w::MANAGEMENT,
        "QVS4103" => w::OTHER_SWITCH,
        "QVS4104" => w::NO_BOND,
        "QVS4106" | "QVS4107" => w::UPLINK_SIZE,
        _ => return None,
    })
}

/// `notes` led by `table`'s identifier for the first, when it has one.
pub fn led_by(mut notes: Refusal, table: fn(&str) -> Option<&'static str>) -> Refusal {
    if let Some((id, text)) = notes.first().cloned()
        && let Some(q) = table(&id)
    {
        notes.insert(0, (q.into(), text));
    }
    notes
}

/// The host's message `id`, led by QSI's where QSI names it.
pub fn no(id: &str, text: impl Into<String>) -> Refusal {
    led_by(vec![(id.into(), text.into())], lead)
}

/// Several of the host's messages at once, as a profile's validation
/// gives them; QSI's identifier first where it names the first.
pub fn every(notes: Refusal) -> Refusal {
    led_by(notes, lead)
}

/// A value of an open enumeration this host does not have.
pub fn no_such(what: &str) -> Refusal {
    vec![(
        qsi::host::partition::msgid::NO_SUCH_VALUE.into(),
        format!("This host has no such {what}."),
    )]
}

/// A partition named by ID that is not there.
pub fn no_partition(id: u32) -> Refusal {
    no("QVS2001", format!("Partition {id} does not exist."))
}

/// A partition that is activated, where it must not be.
pub fn activated(name: &str) -> Refusal {
    no("QVS2002", format!("Partition {name} is activated."))
}

/// A partition that is not activated, where it must be.
pub fn not_activated(name: &str) -> Refusal {
    no("QVS2003", format!("Partition {name} is not activated."))
}

/// QS/VS's naming rule of partitions, pools and switches: 1 to 10 of
/// `A` to `Z`, `0` to `9` and `_`, a letter first.
pub fn name_ok(n: &str) -> bool {
    (1..=10).contains(&n.len())
        && n.starts_with(|c: char| c.is_ascii_uppercase())
        && n.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// A profile's name: QS/VS's rule, and the prototype's lower case and
/// `-` (`default`, `big-test`).
pub fn profile_name_ok(n: &str) -> bool {
    (1..=10).contains(&n.len())
        && n.starts_with(|c: char| c.is_ascii_alphabetic())
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A virtual disk's name: 1 to 32 letters, digits, `-` and `_`, a
/// letter first.
pub fn disk_name_ok(n: &str) -> bool {
    (1..=32).contains(&n.len())
        && n.starts_with(|c: char| c.is_ascii_alphabetic())
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A medium's name: a virtual disk's rule, and the prototype's `.`
/// (`QS400_CUM_26290.iso`).
pub fn media_name_ok(n: &str) -> bool {
    (1..=32).contains(&n.len())
        && n.starts_with(|c: char| c.is_ascii_alphabetic())
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Hundredths of a core as a console shows them: `1.50`.
pub fn units(u: u32) -> String {
    format!("{}.{:02}", u / 100, u % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_rules() {
        assert!(name_ok("POOL1") && !name_ok("pool1") && !name_ok("1POOL"));
        assert!(!name_ok("ABCDEFGHIJK"));
        assert!(profile_name_ok("big-test") && profile_name_ok("NORMAL"));
        assert!(media_name_ok("QS400_V1R0M0_INSTALL.iso") && !disk_name_ok("VD.1"));
    }

    #[test]
    fn qsi_leads_where_it_names_the_conflict() {
        let r = no("QVS2013", "stale");
        assert_eq!(r[0].0, "QSI0401");
        assert_eq!(r[1].0, "QVS2013");
        assert_eq!(no("QVS2001", "x").len(), 1);
        let r = led_by(vec![("QVS3019".into(), "fixed".into())], pools_lead);
        assert_eq!(r[0].0, "QSI1616");
    }
}
