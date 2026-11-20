// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's own messages (`docs/MESSAGES.md`): every message a
//! web page shows, with its first-level text, cause, recovery and
//! severity. Its identifier links to `/qsmc/msg/<ID>`, the second-level
//! text, as `qsweb` renders every message row.
//!
//! The families: `QMC8nnn` Console Management, `QMC9nnn` sign-on and
//! sessions. The prototype's later families (`QMC1nnn` connections to
//! `QMC7nnn` updates) arrive with their phases.

#![forbid(unsafe_code)]

use qsweb::msg::{Kind, Message, MessageText, SecondLevel, substitute};

/// One message of the catalogue.
#[derive(Clone, Copy, Debug)]
pub struct Def {
    /// `QMC9001`.
    pub id: &'static str,
    /// 00 to 99.
    pub severity: u8,
    /// The first-level text, `&1`, `&2` substituted.
    pub text: &'static str,
    /// What happened.
    pub cause: &'static str,
    /// What to do.
    pub recovery: &'static str,
}

macro_rules! catalogue {
    ($($name:ident = $id:literal, $sev:literal, $text:literal, $cause:literal, $recovery:literal;)*) => {
        $(
            #[doc = $text]
            pub const $name: Def = Def { id: $id, severity: $sev, text: $text, cause: $cause, recovery: $recovery };
        )*
        /// Every message.
        pub const ALL: &[Def] = &[$($name),*];
    };
}

catalogue! {
    // Sign-on and sessions.
    SIGNON_NOT_CORRECT = "QMC9001", 30, "User ID or password not correct.",
        "The user ID is not a user of this console, or the password is not its password.",
        "Type the user ID and the password again. After five wrong passwords in a row the user ID is disabled for five minutes.";
    SIGNON_LOCKED = "QMC9002", 30, "User ID &1 is disabled for &2 more seconds.",
        "Five wrong passwords in a row were typed for the user ID.",
        "Wait until the time has passed, or ask an Administrator to set a new password, which enables it at once.";
    SIGNON_DISABLED = "QMC9003", 30, "User ID &1 is disabled.",
        "An Administrator disabled the user ID.",
        "Ask an Administrator of this console to enable it.";
    SIGNON_TOO_MANY = "QMC9004", 30, "Too many sign-on attempts from this address. Try again in &1 seconds.",
        "More sign-on attempts came from this address in the last minute than the console takes.",
        "Wait the time given and sign on again.";
    SIGNED_OFF = "QMC9005", 0, "You are signed off.",
        "You signed off, or your session ended.",
        "Sign on to go on.";
    SESSION_ENDED = "QMC9006", 10, "Your session ended. Sign on again.",
        "The session was idle longer than the idle timeout, reached the maximum session length, or its user ID was disabled or removed.",
        "Sign on again. The timeouts are on Language, locale and session.";
    NOT_AUTHORIZED = "QMC9010", 30, "User &1 is not authorized to &2.",
        "The user's role does not have the task the page needs. Roles and their tasks are on Users and roles.",
        "Ask an Administrator of this console for a role with the task.";
    FORM_EXPIRED = "QMC9011", 30, "The form has expired. Nothing was changed.",
        "The form was not sent with the current session's token: it was opened in an earlier session, or did not come from this console.",
        "Open the page again and send the form from it.";
    NOT_FOUND = "QMC9020", 20, "There is no page &1.",
        "The address does not name a page of this console.",
        "Go to the page from the navigation tree.";
    LATER_RELEASE = "QMC9030", 0, "&1 arrives in a later release of this console.",
        "This function of the console is not available yet.",
        "Use the functions under Console Management.";
    NO_SYSTEMS = "QMC9031", 0, "No managed system has been added to this console.",
        "Managed systems are added and reached over QSI, which arrives in a later release.",
        "Use the functions under Console Management.";

    // Users and roles.
    USER_CREATED = "QMC8001", 0, "User &1 created with role &2.",
        "An Administrator created the user.", "None.";
    USER_CHANGED = "QMC8002", 0, "User &1 changed.",
        "An Administrator changed the user's properties.", "None.";
    PASSWORD_CHANGED = "QMC8003", 0, "Password of user &1 changed.",
        "The user's password was set.", "None.";
    USER_DELETED = "QMC8004", 0, "User &1 deleted.",
        "An Administrator deleted the user.", "None.";
    USER_ENABLED = "QMC8005", 0, "User &1 enabled.",
        "An Administrator enabled the user.", "None.";
    USER_DISABLED = "QMC8006", 0, "User &1 disabled. Its sessions ended.",
        "An Administrator disabled the user; any session of the user ended.", "None.";
    ROLES_CHANGED = "QMC8007", 0, "Roles changed. They apply at each user's next sign-on.",
        "An Administrator changed which roles have which tasks.", "None.";
    LAST_ADMIN = "QMC8010", 30, "The last enabled Administrator cannot be removed, disabled or given another role.",
        "The console must keep one enabled Administrator.",
        "Make or enable another Administrator first.";
    NOT_VALID = "QMC8011", 30, "&1",
        "A value on the form is not valid.",
        "Correct the value and send the form again.";

    // Network settings.
    NETWORK_TRYING = "QMC8100", 10, "Network change on trial: it is taken back in &1 seconds unless it is kept.",
        "The change was applied on trial.",
        "A change of the address the browser came in on is kept when you sign on at the new address within the time; any other change when its interfaces work.";
    NETWORK_KEPT = "QMC8101", 0, "Network change kept.",
        "The console was reached within the trial, and the change was written.", "None.";
    NETWORK_RESTORED = "QMC8102", 20, "Network change taken back: &1",
        "The console was not reached within the trial, or the change did not work.",
        "Check the values and apply them again.";
    NETWORK_SAVED = "QMC8103", 0, "Network settings changed.",
        "The change does not touch the address the browser came in on, and was applied.", "None.";

    // Date and time.
    TIME_SAVED = "QMC8200", 0, "Date and time settings changed.",
        "The time servers or the display offset were changed.", "None.";
    CLOCK_SET = "QMC8201", 0, "Clock set to &1.",
        "The clock was set by hand.", "None.";

    // Certificates.
    CERT_MADE = "QMC8300", 0, "A new self-signed certificate is in use: SHA-256 &1.",
        "An Administrator made a new self-signed certificate. New connections are served with it.",
        "Accept the new certificate in the browser; compare its fingerprint with the one shown.";
    CERT_IMPORTED = "QMC8301", 0, "The imported certificate is in use: SHA-256 &1.",
        "An Administrator imported a certificate and its key. New connections are served with it.", "None.";
    CERT_REFUSED = "QMC8302", 30, "The certificate was not imported: &1",
        "The file is not a certificate with its key, or the key does not belong to it.",
        "Import the certificate chain and its private key together, as PEM or as PKCS #12 with its password.";
    CA_ADDED = "QMC8303", 0, "Certificate authority &1 is trusted.",
        "An Administrator imported a certificate authority's certificate without a key: it is added to the trusted authorities.",
        "None.";
    CA_REMOVED = "QMC8304", 0, "Certificate authority &1 is no longer trusted.",
        "An Administrator removed a certificate authority from the trusted authorities.", "None.";
    CERT_CHOOSE = "QMC8305", 0, "Choose the file to import under Import certificate.",
        "Import... asks for a file: the form under the trusted authorities takes it.",
        "Choose a PEM file with the chain and its key, or a PKCS #12 file with its password. A certificate authority's certificate alone is added to the trusted authorities.";

    // Backup and restore.
    BACKUP_MADE = "QMC8400", 0, "Backup made: &1.",
        "A backup of the console was made and downloaded.", "Keep the file and its passphrase apart.";
    RESTORE_PENDING = "QMC8401", 10, "The backup of &1 is restored when the console restarts.",
        "The backup was checked and is waiting for the restart.",
        "Restart the console from Shut down or restart.";
    BACKUP_PASSPHRASE = "QMC8402", 30, "The passphrase is not this backup's, or the file is damaged.",
        "The backup could not be opened with the passphrase given.",
        "Type the passphrase the backup was made with.";
    BACKUP_DAMAGED = "QMC8403", 30, "The file is not a backup of a QS/MC console.",
        "The file is damaged or of another kind.", "Choose the backup file the console made.";
    BACKUP_NEWER = "QMC8404", 30, "The backup is of release &1, newer than this console's &2.",
        "A backup can be restored on its release or a later one.",
        "Update the console to the backup's release first.";
    REMOTE_NOT_IN_V1 = "QMC8405", 20, "Copying a backup to a remote host is not in this release. Download the backup instead.",
        "A backup holds the console's identity and users; in this release it leaves the console only through the browser.",
        "Choose Download as file.";
    PASSPHRASE_RULES = "QMC8406", 30, "The passphrase must be 12 to 128 characters.",
        "The passphrase does not follow the rules: a backup holds the console's identity and users.",
        "Type a passphrase of 12 to 128 characters, and keep it apart from the file.";

    // Shut down or restart.
    SHUTDOWN_STARTED = "QMC8500", 0, "The console is powering off.",
        "An Administrator asked the console to power off.", "Start it at the machine.";
    RESTART_STARTED = "QMC8501", 0, "The console is restarting.",
        "An Administrator asked the console to restart.", "Sign on again when it is back.";
    ANNOUNCED = "QMC8502", 0, "Message from &1: &2",
        "A user shutting down or restarting the console left a message for everyone signed on.",
        "None.";

    // Language, locale and session.
    SETTINGS_SAVED = "QMC8600", 0, "Settings changed.",
        "The settings were saved.", "None.";
    NOT_IN_V1 = "QMC8700", 20, "&1 is not in this release.",
        "The function is shown by the design but not provided by this release of the console.",
        "See the console's documentation for what this release does.";
}

/// The definition of `id`.
pub fn def(id: &str) -> Option<&'static Def> {
    ALL.iter().find(|d| d.id == id)
}

/// A message row of `def` with `data`.
pub fn message(kind: Kind, def: &Def, data: &[&str]) -> Message {
    Message::new(kind, def.id, &substitute(def.text, data)).with_data(data)
}

/// The catalogue as `qsweb` reads second-level text.
pub struct Catalogue;

impl MessageText for Catalogue {
    fn lookup(&self, id: &str, data: &[&str]) -> Option<SecondLevel> {
        let d = def(id)?;
        Some(SecondLevel {
            id: d.id.into(),
            text: substitute(d.text, data),
            cause: substitute(d.cause, data),
            recovery: substitute(d.recovery, data),
            severity: d.severity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_qmc_and_once_each() {
        for (i, d) in ALL.iter().enumerate() {
            assert!(d.id.len() == 7 && d.id.starts_with("QMC"), "{}", d.id);
            assert!(d.id[3..].bytes().all(|b| b.is_ascii_digit()), "{}", d.id);
            assert!(ALL[..i].iter().all(|o| o.id != d.id), "{} twice", d.id);
            assert!(!d.text.is_empty() && !d.cause.is_empty() && !d.recovery.is_empty());
        }
        let m = message(Kind::Error, &SIGNON_LOCKED, &["qsadmin", "300"]);
        assert_eq!(m.text, "User ID qsadmin is disabled for 300 more seconds.");
        let sl = Catalogue.lookup("QMC9002", &["qsadmin", "300"]).unwrap();
        assert_eq!(sl.severity, 30);
        assert!(Catalogue.lookup("CPF0001", &[]).is_none());
    }

    /// docs/MESSAGES.md lists every message with its severity and its
    /// three texts, and no other.
    #[test]
    fn the_document_lists_the_catalogue() {
        let doc = include_str!("../../../docs/MESSAGES.md");
        for d in ALL {
            let row = format!(
                "| `{}` | {} | {} | {} | {} |",
                d.id, d.severity, d.text, d.cause, d.recovery
            );
            assert!(
                doc.lines().any(|l| l == row),
                "docs/MESSAGES.md lacks: {row}"
            );
        }
        let rows = doc
            .lines()
            .filter(|l| l.starts_with("| `QMC") && l.matches(" | ").count() == 4)
            .count();
        assert_eq!(rows, ALL.len());
    }
}
