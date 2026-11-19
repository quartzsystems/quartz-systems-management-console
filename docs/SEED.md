# The seed

How a console gets its first configuration before there is a screen to
enter it on, and how a test gets any. The format and the step are QS/VS's
`base/seed`, unchanged; this document says what is QS/MC's.

## The base format

QS/VS's `docs/SEED.md` (in its repository, at the pinned commit) is the
reference: when a seed is read, the volume (a FAT file system, on a whole
disk or a partition), `seed.toml` and its `schema_version`, the `[host]`
and `[network]` tables with every key, and why a seed with a mistake in
it writes nothing. `base/seed` and `base/state` hold it, and both are
taken unchanged.

In short: at every IPL, after `/data` is mounted and the clock is set,
PID 1 runs the seed step. When `/data/state/host.toml` exists the
console is configured and nothing is looked for. Otherwise the block
devices are searched for a FAT volume with the console's label; its
`seed.toml` is checked as a whole, and `network.toml` and then
`host.toml` are written from it. A seed is applied once.

```toml
schema_version = 1

[host]
name = "mc-01"
time_zone = "UTC+01:00"
ntp_servers = ["time1.example.net", "192.0.2.123:123"]

[network]
mode = "static"
address = "192.0.2.10/24"
gateway = "192.0.2.1"
vlan = 100
dns = ["192.0.2.53"]
```

## What is QS/MC's

| What | QS/MC's |
|---|---|
| The volume's label | `QSMCSEED` (`release::SEED_LABEL`) |
| The codes | `C3001060 Seed checked`, `C3001061 Seed applied`, `A3001060 Seed not valid` (`docs/REFCODES.md`) |
| A default host name | `QSMC` and the last six hexadecimal digits of the first Ethernet link's MAC address |
| The interface roles | `open`, first: operators reach the console on it, with the default route, the name servers and the time; `private`, optional, toward the systems it manages, with no default route (`release::INTERFACE_ROLES`) |
| Tables of its own | none |

`[network]` is read in either form of `network.toml` (`base/state`,
version 2 since QS/VS's `f0ef0f1`): the keys of one interface, which is
then `open`, or one `[[network.interface]]` table an interface, each
with `role` and, besides version 1's keys, `carries_route`,
`carries_dns` and `carries_time`. A role QS/MC does not have, or
`private` first, is refused by the network service and by the
installer. Either form becomes a `network.toml` of version 2:

```toml
schema_version = 2

[[network.interface]]
role = "open"
mode = "dhcp"

[[network.interface]]
role = "private"
device = "eth1"
mode = "static"
address = "198.51.100.1/24"
carries_route = false
carries_dns = false
carries_time = false
```

`schema_version = 2` adds `[install]`, an unattended installation's
answers, which the installer reads (`lic/qsmcinstall`,
`docs/INSTALL.md`): the disks by serial number or location code, the
first service tools user's ID and its Argon2id hash. With it the
installer writes the disks, `network.toml` with both roles as given,
the users file and `host.toml`, and shows nothing but its progress. A
running console reads the `[host]` and `[network]` tables and leaves
`[install]` alone; an installed console is configured, so it reads no
seed at all.

What the console keeps for itself (`identity.toml` with its derived
serial number, `inventory.toml`, and the identity's key and
certificate) does not make it configured.

## Making a seed volume

Any FAT volume labelled `QSMCSEED` with `seed.toml` at its root: a USB
stick, or a virtual disk. The harness makes its own with `base/qemu`'s
`disk::seed_volume`, and its answers files with
`tools/conformance/src/suite/install/mod.rs` (`answers`).
