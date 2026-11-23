// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What QS/MC asks of its kernel: the fragment, the baked command line
//! and the symbols the final `.config` is checked for, group by group as
//! `kernel/config/qsmc.config` has them. Every symbol the fragment sets
//! is here, so a merge that drops one fails the build.

/// The kernel configuration fragment in `kernel/config/`.
pub const FRAGMENT: &str = "qsmc.config";

/// The baked command line, before each UKI's own word.
pub const CMDLINE: &str = "console=ttyS0,115200 console=tty0 panic=-1 quiet";

/// The groups the fragment sets `=y`, with their symbols.
pub const GROUPS_Y: &[(&str, &[&str])] = &[
    ("image", &["64BIT"]),
    (
        "networking",
        &["NET", "INET", "IPV6", "UNIX", "PACKET", "VLAN_8021Q"],
    ),
    (
        "supervision",
        &[
            "CGROUPS",
            "CGROUP_PIDS",
            "MEMCG",
            "SECCOMP",
            "SECCOMP_FILTER",
        ],
    ),
    (
        "load source",
        &[
            "XFS_FS",
            "BLK_DEV_UBLK",
            "IO_URING",
            "PCI",
            "BLK_DEV_NVME",
            "ATA",
            "SATA_AHCI",
            "SCSI",
            "BLK_DEV_SD",
            "SCSI_LOWLEVEL",
            "MEGARAID_SAS",
            "SCSI_MPT3SAS",
            "SCSI_SMARTPQI",
            "SCSI_HPSA",
            "SCSI_AACRAID",
            "SCSI_MPI3MR",
        ],
    ),
    (
        "ESP and firmware variables",
        &[
            "VFAT_FS",
            "NLS",
            "NLS_CODEPAGE_437",
            "NLS_ISO8859_1",
            "EFIVAR_FS",
        ],
    ),
    (
        "removable volumes",
        &[
            "ISO9660_FS",
            "JOLIET",
            "BLK_DEV_SR",
            "USB",
            "USB_XHCI_HCD",
            "USB_EHCI_HCD",
            "USB_STORAGE",
            "USB_UAS",
        ],
    ),
    (
        "installation",
        &[
            "EFI_PARTITION",
            "MSDOS_PARTITION",
            "HID_GENERIC",
            "FB_DEVICE",
        ],
    ),
    (
        "management port",
        &[
            "NETDEVICES",
            "ETHERNET",
            "NET_VENDOR_INTEL",
            "E1000",
            "E1000E",
            "IGB",
            "IGC",
            "IXGBE",
            "I40E",
            "ICE",
            "NET_VENDOR_BROADCOM",
            "TIGON3",
            "BNXT",
            "NET_VENDOR_MELLANOX",
            "MLX5_CORE",
            "MLX5_CORE_EN",
        ],
    ),
    (
        "virtio guest",
        &[
            "VIRTIO_MENU",
            "VIRTIO_PCI",
            "VIRTIO_BLK",
            "SCSI_VIRTIO",
            "VIRTIO_NET",
            "VIRTIO_CONSOLE",
            "HW_RANDOM",
            "HW_RANDOM_VIRTIO",
        ],
    ),
    (
        "VMware guest",
        &[
            "FUSION",
            "FUSION_SPI",
            "FUSION_SAS",
            "VMWARE_PVSCSI",
            "ATA_PIIX",
            "VMXNET3",
        ],
    ),
    (
        "Hyper-V guest",
        &[
            "HYPERVISOR_GUEST",
            "HYPERV",
            "HYPERV_NET",
            "HYPERV_STORAGE",
            "HYPERV_KEYBOARD",
        ],
    ),
    (
        "local console",
        &[
            "VT",
            "VT_CONSOLE",
            "FB",
            "FRAMEBUFFER_CONSOLE",
            "DRM",
            "DRM_FBDEV_EMULATION",
            "DRM_SIMPLEDRM",
            "SYSFB_SIMPLEFB",
            "SERIAL_8250",
            "SERIAL_8250_CONSOLE",
            "INPUT_EVDEV",
            "INPUT_KEYBOARD",
            "SERIO_I8042",
            "KEYBOARD_ATKBD",
            "HID",
            "USB_HID",
        ],
    ),
    (
        "base system",
        &[
            "RTC_CLASS",
            "RTC_DRV_CMOS",
            "RTC_INTF_SYSFS",
            "RTC_INTF_DEV",
            "ACPI_BUTTON",
            "DMI",
            "DMIID",
            "SIGNALFD",
        ],
    ),
    ("hardening", HARDENING_Y),
];

/// QS/VS's Phase 11 hardening, the symbols set `=y`.
pub const HARDENING_Y: &[&str] = &[
    "SECURITY_DMESG_RESTRICT",
    "HARDENED_USERCOPY",
    "FORTIFY_SOURCE",
    "SLAB_FREELIST_RANDOM",
    "SLAB_FREELIST_HARDENED",
    "RANDOM_KMALLOC_CACHES",
    "LIST_HARDENED",
    "BUG_ON_DATA_CORRUPTION",
    "SCHED_STACK_END_CHECK",
    "INIT_ON_ALLOC_DEFAULT_ON",
    "RANDOMIZE_KSTACK_OFFSET_DEFAULT",
    "ZERO_CALL_USED_REGS",
    "PANIC_ON_OOPS",
    "PSTORE",
    "EFI_VARS_PSTORE",
    "STRICT_KERNEL_RWX",
    "STACKPROTECTOR_STRONG",
    "VMAP_STACK",
    "RANDOMIZE_BASE",
    "RANDOMIZE_MEMORY",
    "CPU_MITIGATIONS",
    "SECURITY_YAMA",
    "SECURITY_LANDLOCK",
    "LEGACY_VSYSCALL_NONE",
];

/// The groups the fragment sets off, with their symbols.
pub const GROUPS_N: &[(&str, &[&str])] = &[
    (
        "image",
        &[
            "LOCALVERSION_AUTO",
            "KERNEL_GZIP",
            "RD_GZIP",
            "RD_BZIP2",
            "RD_LZMA",
            "RD_XZ",
            "RD_LZO",
            "RD_LZ4",
            "RD_ZSTD",
            "IKCONFIG",
        ],
    ),
    ("load source", &["SCSI_SCAN_ASYNC"]),
    ("local console", &["FB_EFI"]),
    ("base system", &["RTC_HCTOSYS"]),
    ("what partitions need", LEFT_OUT),
    (
        "nothing else",
        &[
            "NETFILTER",
            "EXT4_FS",
            "WIRELESS",
            "WLAN",
            "CFG80211",
            "BT",
            "SOUND",
            "MEDIA_SUPPORT",
        ],
    ),
    ("hardening", HARDENING_N),
];

/// What QS/VS has for running partitions and a console leaves out.
pub const LEFT_OUT: &[&str] = &[
    "VIRTUALIZATION",
    "KVM",
    "VHOST_MENU",
    "VHOST_NET",
    "BRIDGE",
    "BONDING",
    "TUN",
    "VFIO",
    "HUGETLBFS",
    "CHR_DEV_ST",
    "SCSI_FC_ATTRS",
];

/// QS/VS's Phase 11 hardening, the symbols set off.
pub const HARDENING_N: &[&str] = &[
    "SLAB_MERGE_DEFAULT",
    "SECURITY_SELINUX",
    "KEXEC",
    "CRASH_DUMP",
    "HIBERNATION",
    "DEVMEM",
    "DEVPORT",
    "PROC_KCORE",
    "X86_IOPL_IOPERM",
    "IA32_EMULATION",
    "LEGACY_TIOCSTI",
    "LDISC_AUTOLOAD",
    "BINFMT_MISC",
    "KPROBES",
    "UPROBE_EVENTS",
    "UPROBES",
    "EFI_CUSTOM_SSDT_OVERLAYS",
    "MAGIC_SYSRQ",
    "DEBUG_FS",
    // It selects DEBUG_FS: QS/VS's kernel has debugfs through it.
    "BLK_DEV_IO_TRACE",
    "USER_NS",
    "BPF_SYSCALL",
    "USERFAULTFD",
    "IPV6_SIT",
];

/// Every symbol of [`GROUPS_Y`], in order.
pub fn required_y() -> &'static [&'static str] {
    flat(GROUPS_Y, &REQUIRED_Y_CELL)
}

/// Every symbol of [`GROUPS_N`], in order.
pub fn required_n() -> &'static [&'static str] {
    flat(GROUPS_N, &REQUIRED_N_CELL)
}

type Cell = std::sync::OnceLock<Vec<&'static str>>;
static REQUIRED_Y_CELL: Cell = std::sync::OnceLock::new();
static REQUIRED_N_CELL: Cell = std::sync::OnceLock::new();

fn flat(
    groups: &'static [(&str, &[&'static str])],
    cell: &'static Cell,
) -> &'static [&'static str] {
    cell.get_or_init(|| groups.iter().flat_map(|(_, s)| s.iter().copied()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = include_str!("../../../../kernel/config/qsmc.config");

    fn set_y() -> Vec<&'static str> {
        TEXT.lines()
            .filter_map(|l| l.strip_prefix("CONFIG_")?.strip_suffix("=y"))
            .collect()
    }

    fn set_n() -> Vec<&'static str> {
        TEXT.lines()
            .filter_map(|l| l.strip_prefix("# CONFIG_")?.strip_suffix(" is not set"))
            .collect()
    }

    /// The check covers exactly what the fragment sets, both ways.
    #[test]
    fn the_lists_are_the_fragment() {
        let (mut y, mut want_y) = (set_y(), required_y().to_vec());
        y.sort_unstable();
        want_y.sort_unstable();
        assert_eq!(y, want_y);
        let (mut n, mut want_n) = (set_n(), required_n().to_vec());
        n.sort_unstable();
        want_n.sort_unstable();
        assert_eq!(n, want_n);
    }

    #[test]
    fn no_symbol_is_listed_twice_or_both_ways() {
        let y = required_y();
        let n = required_n();
        for (i, sym) in y.iter().enumerate() {
            assert!(!y[..i].contains(sym), "{sym} twice");
            assert!(!n.contains(sym), "{sym} both ways");
        }
        for (i, sym) in n.iter().enumerate() {
            assert!(!n[..i].contains(sym), "{sym} twice");
        }
    }

    /// What Section 2 of the phase names, each checked.
    #[test]
    fn every_phase_0_symbol_is_checked() {
        let y = required_y();
        for sym in [
            "XFS_FS",
            "BLK_DEV_UBLK",
            "IO_URING",
            "RTC_CLASS",
            "ACPI_BUTTON",
            "EFIVAR_FS",
            "CGROUPS",
            "USB_STORAGE",
            "VFAT_FS",
            "ISO9660_FS",
            "VIRTIO_PCI",
            "VIRTIO_BLK",
            "SCSI_VIRTIO",
            "VIRTIO_NET",
            "VIRTIO_CONSOLE",
            "HW_RANDOM_VIRTIO",
            "VMWARE_PVSCSI",
            "VMXNET3",
            "HYPERV",
            "HYPERV_NET",
            "HYPERV_STORAGE",
        ] {
            assert!(y.contains(&sym), "{sym}");
        }
        let n = required_n();
        for sym in [
            "KVM",
            "VHOST_NET",
            "BRIDGE",
            "TUN",
            "VFIO",
            "HUGETLBFS",
            "CHR_DEV_ST",
            "SCSI_FC_ATTRS",
        ] {
            assert!(n.contains(&sym), "{sym}");
        }
        assert_eq!(CMDLINE, "console=ttyS0,115200 console=tty0 panic=-1 quiet");
    }
}
