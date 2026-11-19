# QS/MC

Quartz Systems/Management Console: the appliance from which an operator
manages QS/VS hosts with their QS/400 partitions, and later standalone
QS/400 systems. The Linux kernel is the only inherited component;
everything above it is Rust, static musl, x86_64 only.

Its only management surface is a web UI, with a small local console on
the display and serial port for the basics. It reaches what it manages
over QSI, Quartz's own protocol, on TCP 2300. It takes QS/400's build
pipeline and QSI crates, and QS/VS's product-neutral `base/` crates, as
git dependencies pinned at one commit each.

## Status

Phase 0: the toolchain. A clean clone builds a reproducible, bootable
image whose Rust PID 1 prints a banner, and the two pins are proven to
work together. There is no web UI, QSI client or installer yet. See
`docs/PHASE0.md`.

## Building

On Linux (WSL Ubuntu works), with the prerequisites of `docs/PHASE0.md`:

```
cargo run -p build -- doctor
cargo run -p build -- all
cargo run -p build -- test
cargo run -p build -- test --media iso
```

## License

MIT; see `LICENSE`. Third-party notices are in `lic/release/NOTICES.txt`.
