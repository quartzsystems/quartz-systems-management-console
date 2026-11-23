# Qualifying QS/MC V1R0M0

Every `NEEDS-CODY` item of Phases 0 to 8 in one runbook, in the order to
run them. Each step names its pages or panels, what to expect, and what
to record. The tag waits on all of them (`docs/RELEASE_NOTES_V1R0M0.md`).
Record each result in a copy of the table at the end, with the date,
the machine and the build.

The machines: a console machine (any x86_64 server or PC that meets
`docs/INSTALL.md`'s table), a hypervisor for the virtual console (KVM,
VMware or Hyper-V), the Dell running QS/VS as the real host, and a
machine able to run a standalone QS/400.

## 1. The keys

1. On a trusted machine with no network, run `cargo run -p build --
   keys /media/keys` (`docs/RELEASING.md`, "The keys"). Expect it to
   print the release key's public half and the db certificate's
   SHA-256, and to write five files.
   **Record:** both printed values.
2. Copy the folder to two offline volumes kept apart.
   **Record:** where each copy is.
3. Gather the streams' folder: `qsmc.pub` (this release key's public
   half), `qsvs.pub` and `qs400.pub` (from QS/VS's and QS/400's
   releases). **Record:** the three keys' first eight hex digits.
4. If a release workflow is to build the release, put the secrets in
   the repository's `release` environment as QS/VS's `docs/RELEASING.md`
   names them for QS/MC (`QSMC_SIGNING_KEY`, `QSMC_SB_KEY`,
   `QSMC_SB_CERT_B64`). **Record:** done or not.
5. Build the release candidate from a clean tree:
   `QSMC_SIGNING_KEY=… QSMC_SB_KEY=… QSMC_SB_CERT=… QSMC_STREAM_KEYS=…
   cargo run -p build -- release 1.0.0`. Expect "built a second time:
   every file the same" and `target/release/V1R0M0/`.
   **Record:** `SHA256SUMS`.

## 2. The processor's features

1. On the console machine, boot the ISO and read the installer's first
   screen; then, installed, the status screen's Processor row. Expect
   `has what the cryptography needs`. A processor without AVX2, ADX,
   AES or PCLMULQDQ is said by `A3001015`, and the web interface does
   not start.
   **Record:** the processor's model and the row.
2. On the hypervisor, give the virtual machine the host's processor
   model, and read the same row. **Record:** the hypervisor, its version,
   the model given, the row.

## 3. A console installed on a real machine

1. Write the release's ISO to a USB stick, boot it, and install
   attended on two disks, mirrored (`docs/INSTALL.md`). Expect the
   installer's screens, its summary's Secure Boot line, the restart into
   slot A, and the local console's sign-on.
   **Record:** the disks, the Secure Boot line, the time to the first
   sign-on.
2. Enrol `qsmc-db.cer` in the firmware's db and turn Secure Boot on
   (`docs/INSTALL.md`, "Secure Boot"). Expect the console to start and
   the status screen's Signature row to say `release key, Secure Boot
   on`. Boot a test image's ISO: expect the firmware to refuse it.
   **Record:** the firmware, the row, the refusal.
3. The local console at the display and the serial port: every panel of
   `docs/LOCALCONSOLE.md`, the pointer clicking into a field and a key's
   label, Page Up and Down on a list. **Record:** any panel that differs
   from its golden screen.
4. Sign on to the web interface from a browser at the `open` address.
   **Record:** the browser and its version.
5. The drivers QEMU never exercised: the machine's disk controller and
   network adapter. **Record:** their names, and that the status screen
   shows the load source and the `open` address.

## 4. A console installed as a virtual machine

1. On the hypervisor, install unattended from the ISO with a seed
   (`docs/SEED.md`). Expect only the progress screen, then the console
   at the seed's address. **Record:** the hypervisor's disk and network
   adapter types.
2. Pull the virtual machine's power during the install's write step,
   then boot the ISO again. Expect it to install again from the start.
   **Record:** the outcome.

## 5. The real host

1. **Paired.** On the Dell's Service Tools, make a pairing password; on
   the console's Servers, Add the host with it (`mgmt-pair`'s steps).
   Expect the host Connected on Connectivity. **Record:** the host's
   release and build, and the console's certificate fingerprint as the
   host shows it.
2. **A partition created and QS/400 installed through the terminal
   window.** Create a partition with the wizard, load QS/400's
   installation image (Virtual optical, upload), activate it from D
   attended, and answer the install in the terminal window. Expect its
   codes as they happen on the partition's row, the console at QS/400's
   sign-on, and function 21. **Record:** the codes seen, the time the
   install took, any key that did not reach the partition.
3. **A disk pulled and its event read.** Pull a pool's disk from the
   Dell. Expect a Serviceable event at the console with the host's code
   and the disk's location; repair it as the event's guided repair says;
   expect the event closed. **Record:** the event's number, code and
   location.
4. **The host updated and rolled back.** Publish a QS/VS V1R0M1 signed
   by the qsvs stream's release key on the stream; install it on the
   host from Code updates. Expect it confirmed and Connected again.
   Then install one that cannot confirm (QS/VS's own broken build) and
   expect the host back on V1R0M0. **Record:** each outcome and time.
5. **The console updated and rolled back.** The same for the console
   with a QS/MC V1R0M1 on the `qsmc` stream, and from a removable volume
   at the local console's option 8. Expect each confirmed with its state
   migrated after the confirmation and its systems Connected again; a
   broken build rolled back with every state folder as it was.
   **Record:** each outcome.
6. **A plan captured and deployed.** Capture the Dell's plan, then
   deploy it to a second host, or back to the Dell after a reinstall,
   and activate its partitions with QS/400 on their load sources.
   Expect only what was missing made. **Record:** the steps, and
   anything that could not be put back the same.
7. **A standalone QS/400 paired.** Pair a standalone QS/400 system,
   open its console in the terminal window, and use its control panel.
   **Record:** its release, the functions listed.

## 6. Every page beside its prototype file

With the console managing the Dell and the standalone system, open each
of the prototype's 86 live files (`design/qsmc-prototype/`) and the page
the console serves for it side by side, in two browsers. Check
`docs/DESIGN_REVIEW.md` against what is seen. Then the Partitions list at
1024 by 768: expect 25 rows without the page body scrolling. Then, with
the browser's scripts off, sign on, open a partition's properties and
shut it down through its confirmation: expect each to work (only the
terminal window needs its script). Then with the stylesheet off: expect
every page read in order, its forms usable.
**Record:** any page that differs, and the browsers.

## 7. The soak beside a real host

Run `sec-soak` against the simulated fleet on a console machine beside
the Dell for 24 hours (`QSMC_SOAK_SECS=86400 cargo run -p build -- test
--suite sec-soak`), with the Dell paired and a partition's terminal
window open in a browser throughout. Expect the suite to pass and the
Dell Connected at the end. **Record:** the suite's line (what each
service held idle, at its peak and at the end).

## 8. Before the tag

1. `docs/DESIGN_REVIEW.md`: each item answered A or B, or left for a
   later release.
2. The fuzz targets' four-hour runs and the 24-hour soak recorded with
   no open finding (`docs/PHASE8.md`).
3. Tag `v1.0.0` at the commit built in step 1.5, and publish the
   release's directory as the `qsmc` stream.

| Step | Date | Machine | Build | Result |
|---|---|---|---|---|
| 1.1 | | | | |
