// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The IPL in order, as Phase 0 has it: the panic path, the mounts, the
//! console, the code and the banner, the slot, the processor, then the
//! running console's loop.

pub mod cmdline;
#[cfg(target_os = "linux")]
mod mounts;

#[cfg(target_os = "linux")]
pub fn run() -> ! {
    use crate::console::Console;
    use crate::product::{self, QSMC};
    use refcode::post;

    crate::sys::panic::install();
    let failed = mounts::kernel(QSMC.run_dir);
    let mut console = Console::open();
    console.line(&post::line(&refcode::IPL_STARTED, ""));
    console.line(&(QSMC.banner)());

    let line = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
    let source = cmdline::parse(&line);
    (QSMC.note)(product::SLOT, source.name());
    console.line(&source.line());
    if source == cmdline::Source::Install {
        console.line(cmdline::MEDIA_LINE);
    }

    if failed.is_empty() {
        console.line(&post::line(&refcode::MOUNTED, ""));
    } else {
        let what: Vec<String> = failed.iter().map(|(at, e)| format!("{at} ({e})")).collect();
        console.line(&post::line(&refcode::NOT_MOUNTED, &what.join(", ")));
    }

    let lacking = crate::cpu::lacking();
    (QSMC.note)(product::PROCESSOR, &crate::cpu::line(&lacking));
    console.line(&crate::cpu::line(&lacking));
    if !lacking.is_empty() {
        console.line(&post::line(&refcode::PROCESSOR_LACKS, &lacking.join(", ")));
    }

    crate::sys::run::forever(&mut console)
}
