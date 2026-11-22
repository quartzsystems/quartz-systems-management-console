// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What is QS/MC's in its own code update (`docs/UPDATES.md`), over
//! `base/update`: the image's format, the messages it refuses with, the
//! codes it posts and the state files a release reads. The console runs
//! no partitions: nothing is stopped before its restart.
//!
//! - [`schemas`]: the state files this release reads, and their versions.

#![forbid(unsafe_code)]

pub mod schemas;

use update::{Code, Format, Messages};

/// A QS/MC update image: `QSMCUPDT`, `QSMC-UPDATE 1`, fixes `QMnnnnn`.
pub static FORMAT: Format = Format {
    magic: *b"QSMCUPDT",
    head: "QSMC-UPDATE 1",
    fix_prefix: release::FIX_PREFIX,
};

/// The catalogue's messages for each refusal.
pub static MESSAGES: Messages = Messages {
    busy: &qmcmsg::base::UPD_BUSY,
    cut: &qmcmsg::base::UPD_CUT,
    damaged: &qmcmsg::base::UPD_DAMAGED,
    name: &qmcmsg::base::UPD_NAME,
    not_kept: &qmcmsg::base::UPD_NOT_KEPT,
    not_verified: &qmcmsg::base::UPD_NOT_VERIFIED,
    not_written: &qmcmsg::base::UPD_NOT_WRITTEN,
    nothing_pending: &qmcmsg::base::UPD_NOTHING_PENDING,
    nothing_staged: &qmcmsg::base::UPD_NOTHING_STAGED,
    older: &qmcmsg::base::UPD_OLDER,
    partitions_run: &qmcmsg::base::UPD_PARTITIONS_RUN,
    pending: &qmcmsg::base::UPD_PENDING,
    product: &qmcmsg::base::UPD_PRODUCT,
    record: &qmcmsg::base::UPD_RECORD,
    signature: &qmcmsg::base::UPD_SIGNATURE,
    staged_already: &qmcmsg::base::UPD_STAGED_ALREADY,
    state: &qmcmsg::base::UPD_STATE,
};

/// The console's reference code for one of the service's.
#[must_use]
pub fn code(c: Code) -> &'static refcode::RefCode {
    match c {
        Code::Staged => &refcode::UPDATE_STAGED,
        Code::Verified => &refcode::UPDATE_VERIFIED,
        Code::Refused => &refcode::UPDATE_REFUSED,
        Code::Discarded => &refcode::UPDATE_DISCARDED,
        Code::Installing => &refcode::UPDATE_INSTALLING,
        Code::SlotWritten => &refcode::UPDATE_SLOT_WRITTEN,
        Code::Installed => &refcode::UPDATE_INSTALLED,
        Code::InstallFailed => &refcode::UPDATE_INSTALL_FAILED,
        Code::Cancelled => &refcode::UPDATE_CANCELLED,
    }
}

/// The product an image must be for: a test image's name says it is
/// one, and is updated only by a test image's update.
#[cfg(test_hooks)]
pub const PRODUCT: &str = release::TEST_NAME;
/// The product an image must be for.
#[cfg(not(test_hooks))]
pub const PRODUCT: &str = release::NAME;

/// Where the service keeps what is staged and what each slot holds.
pub const DIR: &str = "/data/update";

#[cfg(test)]
mod tests {
    #[test]
    fn a_qsmc_image_is_its_own() {
        assert_eq!(&super::FORMAT.magic, b"QSMCUPDT");
        assert_eq!(super::FORMAT.head, "QSMC-UPDATE 1");
        assert!(update::manifest::valid_fix(&super::FORMAT, "QM00001"));
        assert!(!update::manifest::valid_fix(&super::FORMAT, "QV00001"));
        assert_ne!(super::FORMAT.magic, *b"QSVSUPDT");
    }

    #[test]
    fn every_code_is_the_console_s() {
        use update::Code::*;
        for c in [
            Staged,
            Verified,
            Refused,
            Discarded,
            Installing,
            SlotWritten,
            Installed,
            InstallFailed,
            Cancelled,
        ] {
            assert!(
                super::code(c).code.starts_with("C3007")
                    || super::code(c).code.starts_with("A3007")
            );
        }
    }

    #[test]
    fn every_refusal_is_a_qmc_message() {
        let m = &super::MESSAGES;
        for x in [
            m.busy,
            m.cut,
            m.damaged,
            m.name,
            m.not_kept,
            m.not_verified,
            m.not_written,
            m.nothing_pending,
            m.nothing_staged,
            m.older,
            m.partitions_run,
            m.pending,
            m.product,
            m.record,
            m.signature,
            m.staged_already,
            m.state,
        ] {
            assert!(x.id.starts_with("QMC71"), "{}", x.id);
            assert!(qmcmsg::def(x.id).is_some());
        }
    }
}
