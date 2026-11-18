// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What a QS/MC load source carries that is QS/MC's own. These go on
//! every installed disk: none may change once one is installed.

use appliance::LoadSourceNames;

/// Slot B's UKI on the ESP of a load-source disk.
pub const SLOT_B_PATH: &str = "EFI/QSMC/B.EFI";

/// QS/MC's load-source names, types and magics.
pub static LOAD_SOURCE: LoadSourceNames = LoadSourceNames {
    record_type: 0x7e12_d3ff_d7f5_454b_a065_b4d8_98ca_2ffc,
    data_type: 0xb2e1_4a0f_e805_43b6_8bed_a926_6d0c_dd6c,
    data_fs_uuid: 0xe72f_9d32_46f4_4246_9259_352d_dcf3_f26e,
    esp_name: "QSMC ESP",
    record_name: "QSMC-AB",
    data_name: "QSMCDATA",
    vendor: "QSMC",
    slot_b_path: SLOT_B_PATH,
    esp_label: "QSMC ESP",
    // `QSMC`.
    esp_volume_id: 0x5153_4d43,
    guid_seed: *b"QSMC",
    record_magic: *b"QSMCREC\0",
    label_magic: *b"QSMCSET\0",
    resume_magic: *b"QSMCRSM\0",
    resume_map_magic: *b"QSMCRMAP",
    intent_magic: *b"QSMCIMAP",
};

#[cfg(test)]
mod tests {
    use super::*;

    fn guid(bits: u128) -> String {
        let h = format!("{bits:032x}");
        format!(
            "{}-{}-{}-{}-{}",
            &h[..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..]
        )
    }

    /// What is on every installed disk. A change here orphans them.
    #[test]
    fn the_load_source_s_names_are_qs_mc_s() {
        let n = &LOAD_SOURCE;
        assert_eq!(guid(n.record_type), "7e12d3ff-d7f5-454b-a065-b4d898ca2ffc");
        assert_eq!(guid(n.data_type), "b2e14a0f-e805-43b6-8bed-a9266d0cdd6c");
        assert_eq!(guid(n.data_fs_uuid), "e72f9d32-46f4-4246-9259-352ddcf3f26e");
        assert_eq!(
            (n.esp_name, n.record_name, n.data_name),
            ("QSMC ESP", "QSMC-AB", "QSMCDATA")
        );
        assert_eq!((n.vendor, n.slot_b_path), ("QSMC", "EFI/QSMC/B.EFI"));
        assert_eq!((n.esp_label, n.esp_volume_id), ("QSMC ESP", 0x5153_4d43));
        assert_eq!(&n.esp_volume_id.to_be_bytes(), b"QSMC");
        assert_eq!(&n.guid_seed, b"QSMC");
        assert_eq!(&n.record_magic, b"QSMCREC\0");
        assert_eq!(&n.label_magic, b"QSMCSET\0");
        assert_eq!(&n.resume_magic, b"QSMCRSM\0");
        assert_eq!(&n.resume_map_magic, b"QSMCRMAP");
        assert_eq!(&n.intent_magic, b"QSMCIMAP");
        assert!(n.slot_b_path.contains(n.vendor));
    }

    /// QS/VS's types (its `release::LOAD_SOURCE`), which a QS/MC disk
    /// must never carry, nor QS/VS take a QS/MC disk for its own.
    #[test]
    fn no_type_is_qs_vs_s() {
        let qsvs = [
            0x1f07_2256_9b60_4f79_a93e_f2af_75b7_2775_u128,
            0x5a62_035d_0a21_4e5c_8bd1_e390_8ef0_dfb9,
            0xfe6b_2094_8863_4006_8bcd_fadc_df20_6e94,
        ];
        let n = &LOAD_SOURCE;
        for ours in [n.record_type, n.data_type, n.data_fs_uuid] {
            assert!(!qsvs.contains(&ours), "{}", guid(ours));
        }
        assert_ne!(n.record_type, n.data_type);
        assert_ne!(&n.record_magic, b"QSVSREC\0");
        assert_ne!(n.record_magic, LoadSourceNames::DEMO.record_magic);
    }
}
