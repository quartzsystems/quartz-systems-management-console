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
    // Terminal windows and the control panel.
    CONSOLE_CLOSED = "QMC4001", 30, "Console connection to partition &1 closed: &2.",
        "The console went: the managed system closed it, another console took it over, the partition ended, or the session to the system was lost.",
        "Choose Connect to open it again; with Take over on Open terminal window to take it from another console.";
    CONSOLE_DISCONNECTED = "QMC4002", 0, "Terminal window to &1 disconnected.",
        "You disconnected the window; the console stays with the managed system and any other window that has it open.",
        "Choose Connect to open it again.";
    // A host's virtual resources.
    IMAGE_UPLOADED = "QMC2401", 0, "Image &1 uploaded to &2's media library.",
        "The host took the whole image and checked it.",
        "None.";
    UPLOAD_NOT_TAKEN = "QMC2402", 30, "Image &1 was not uploaded to &2. The messages that follow say why.",
        "The upload was refused or ended before the host had the whole image; the host keeps nothing of it.",
        "Act on the messages and upload the image again.";
    UPLOAD_REFUSED = "QMC2403", 30, "The upload was not taken: &1.",
        "The browser's form did not arrive as the console's page sends it, or it was cut.",
        "Upload the image again from Virtual Optical, with the page's own form.";
    UPLOAD_TOO_LARGE = "QMC2404", 30, "Image &1 is larger than the &2 GB an upload may be.",
        "The console refuses an upload past its limit before it reads it.",
        "Copy the image to the host another way, or use a smaller one.";
    NO_FETCH = "QMC2405", 20, "The console does not fetch an image from an address.",
        "The console reaches only its managed systems and its product stream; it fetches nothing from an address typed into a page.",
        "Download the image to your workstation and upload it.";
    IMAGE_LOADED = "QMC2406", 0, "Image &1 loaded for partition &2.",
        "The host loaded the image into the partition's virtual optical device.",
        "None.";
    IMAGE_UNLOADED = "QMC2407", 0, "Image &1 unloaded from partition &2.",
        "The host unloaded the image from the partition's device.",
        "None.";
    IMAGE_DELETED = "QMC2408", 0, "Image &1 deleted from &2's media library.",
        "The host deleted the image.",
        "None.";
    VDISK_CREATED = "QMC2410", 0, "Virtual disk &1 created in pool &2.",
        "The host made the virtual disk.",
        "None.";
    VDISK_ASSIGNED = "QMC2411", 0, "Virtual disk &1 assigned to partition &2 in its default profile.",
        "The disk is a LUN on a client adapter of the partition's default profile; a running partition has it at its next activation.",
        "None.";
    VDISK_NOT_ASSIGNED = "QMC2412", 30, "Virtual disk &1 was made, but not assigned to partition &2. The messages that follow say why.",
        "The disk was made; giving it to the partition's profile was refused. The disk stays, unassigned.",
        "Act on the messages, then assign the disk on the partition's Virtual adapters page.";
    VDISK_RESIZED = "QMC2413", 0, "Virtual disk &1 is now &2 GB.",
        "The host grew the disk.",
        "None.";
    VDISK_DELETED = "QMC2414", 0, "Virtual disk &1 deleted from &2.",
        "The host deleted the disk and everything on it.",
        "None.";
    VDISK_UNASSIGNED = "QMC2415", 0, "Virtual disk &1 taken off the profiles of &2's partitions.",
        "The disk is no longer a LUN of any profile that named it; it stays in its pool.",
        "None.";
    POOL_CREATED = "QMC2420", 0, "Storage pool &1 created on &2.",
        "The host made the pool from the disks chosen.",
        "None.";
    POOL_CHANGED = "QMC2421", 0, "Storage pool &1 on &2 changed.",
        "The host took the change.",
        "None.";
    SWITCH_CREATED = "QMC2425", 0, "Virtual switch &1 created on &2.",
        "The host made the switch and its uplink.",
        "None.";
    SWITCH_CHANGED = "QMC2426", 0, "Virtual switch &1 on &2 changed.",
        "The host took the change.",
        "None.";
    SWITCH_DELETED = "QMC2427", 0, "Virtual switch &1 deleted from &2.",
        "The host deleted the switch.",
        "None.";
    VOLUME_CREATED = "QMC2430", 0, "Tape volume &1 created on &2.",
        "The host made the volume in its media library.",
        "None.";
    VOLUME_CHANGED = "QMC2431", 0, "Tape volume &1 on &2 changed.",
        "The host took the change.",
        "None.";
    VOLUME_DELETED = "QMC2432", 0, "Tape volume &1 deleted from &2.",
        "The host deleted the volume.",
        "None.";
    VOLUME_GIVEN = "QMC2433", 0, "Tape volume &1 given to &2's tape device.",
        "The volume is the device's medium in the partition's default profile.",
        "None.";
    TAPE_REMOVED = "QMC2434", 0, "Tape device &1 taken off partition &2's default profile.",
        "The device is no longer in the profile; its volume stays.",
        "None.";
    PASS_THROUGH = "QMC2435", 20, "A host's own tape drive is given to a partition on the partition's I/O page.",
        "A drive passed through to a partition is released for partitions at the host first; the partition's profile then names it.",
        "Release the drive at the host's Service Tools, then add it on the partition's I/O page.";
    PROC_POOL_CHANGED = "QMC2440", 0, "Shared processor pool &1 on &2 made or changed.",
        "The host took the pool.",
        "None.";
    PROC_POOL_DELETED = "QMC2441", 0, "Shared processor pool &1 deleted from &2.",
        "The host deleted the pool.",
        "None.";
    // Partitions and their profiles.
    WAITING_FOR_CONSOLE = "QMC3002", 0, "Partition &1 is waiting for a console (&2). Open a terminal window to continue the IPL.",
        "The partition's IPL stopped until a console connects to it.",
        "Open a terminal window to the partition.";
    ACTIVATED = "QMC3101", 0, "Activation of partition &1 started.",
        "The host took the request; the partition's codes follow its IPL.",
        "None.";
    SHUTTING_DOWN = "QMC3102", 0, "Shut down of partition &1 started.",
        "The host, or the partition's system, took the request.",
        "None.";
    RESTARTING = "QMC3103", 0, "Restart of partition &1 started: it is shut down and activated again.",
        "The host took the request.",
        "None.";
    SELECT_PARTITION = "QMC3104", 30, "Select a partition for &1.",
        "The action needs a partition and none was chosen.",
        "Check the box of a partition and choose the action again.";
    NO_SUCH_PARTITION = "QMC3105", 30, "No partition &1 on managed system &2.",
        "The partition is not in what the console keeps of the host: it was deleted, or the host has not said it yet.",
        "Choose a partition from the Partitions list.";
    ACTIVATE_FAILED = "QMC3110", 30, "Activate request for &1 failed: &2.",
        "The host refused to activate the partition; its messages follow this one.",
        "Act on the host's messages, then activate the partition again.";
    PARTITION_CHANGED = "QMC3120", 0, "Partition &1 changed.",
        "The host took the change.",
        "None.";
    AT_NEXT_ACTIVATION = "QMC3121", 10, "The changes to the profile of running partition &1 take effect at its next activation.",
        "A host gives a partition its profile's resources when it activates it.",
        "Restart the partition when the change is wanted now.";
    ENTITLEMENT_CHANGED = "QMC3122", 0, "The processing units, sharing and weight of running partition &1 are changed now.",
        "A host changes a running shared partition's entitlement at once.",
        "None.";
    NO_OPEN_FIRMWARE = "QMC3123", 20, "Open firmware is not a boot mode of &1.",
        "A QS/VS host has no open firmware: a partition starts normally or in service mode with a manual IPL.",
        "Choose Normal or Service - manual IPL.";
    CONSOLE_IS_TERMINAL = "QMC3124", 20, "The partition's console is the terminal window.",
        "A QS/VS host gives a partition no physical console adapter: its console is the virtual serial adapter 0.",
        "Open a terminal window to reach the partition's console.";
    SERIAL_FIXED = "QMC3125", 20, "The console's serial adapters are fixed.",
        "Adapters 0 and 1 are the partition's console, the terminal window; they are not added, changed or removed.",
        "None.";
    NO_ADDRESS = "QMC3126", 20, "Partition &1 has not said its address.",
        "The partition's system says its addresses over its service channel; it has not, or the channel is down.",
        "Reach Navigator at the partition's own address.";
    PARTITION_CREATED = "QMC3130", 0, "Partition &1 (ID &2) created, Not activated.",
        "The host made the partition and its first profile.",
        "Activate it when it is wanted.";
    ID_IN_USE = "QMC3131", 30, "Partition ID &1 is in use: partition &2 has it.",
        "Each partition of a host has an ID of its own.",
        "Choose a free ID; the step shows the next one.";
    NO_HOST_FOR_PARTITION = "QMC3132", 30, "No managed system can have a partition made from this console.",
        "A partition is made on a connected QS/VS host that offers it.",
        "Add a QS/VS host, or wait until it is connected.";
    PARTITION_DELETED = "QMC3133", 0, "Partition &1 deleted.",
        "The host deleted the partition and its profiles, and its virtual disks when chosen.",
        "None.";
    NOT_CREATED = "QMC3134", 30, "Partition &1 was not created: the host refused its first profile.",
        "Finish makes the partition and then its profile. The host refused the profile, so the partition made for it was deleted again, and nothing of it is left at the host.",
        "Correct the step the host's messages name, and select Finish again.";
    PROFILE_STALE = "QMC3201", 30, "Profile &1 was changed since this page was read.",
        "Another session changed the profile, so the change was refused. The page shows the profile as it is now, with your entries.",
        "Check your entries against the profile and send them again.";
    PROFILE_SAVED = "QMC3202", 0, "The running configuration saved as profile &1.",
        "The host saved what the partition holds now as the profile.",
        "None.";
    PROFILE_DEFAULT = "QMC3203", 0, "Profile &1 is the default.",
        "An activation uses it unless another is chosen.",
        "None.";
    PROFILE_COPIED = "QMC3204", 0, "Profile &1 copied as &2.",
        "The host made the new profile.",
        "None.";
    PROFILE_DELETED = "QMC3205", 0, "Profile &1 deleted.",
        "The host deleted the profile.",
        "None.";
    // Custom groups.
    GROUP_SAVED = "QMC3301", 0, "Custom group &1 saved.",
        "The console keeps the group and its members.",
        "None.";
    GROUP_EXISTS = "QMC3302", 30, "A custom group named &1 exists.",
        "Each group has a name of its own.",
        "Choose another name, or edit that group.";
    GROUP_DELETED = "QMC3303", 0, "Custom group &1 deleted.",
        "The group is gone; its systems and partitions are as they were.",
        "None.";
    // Scheduled operations.
    SCHEDULED = "QMC3401", 0, "&1 scheduled, first at &2.",
        "The scheduler keeps the operation and runs it at its times in your name.",
        "None.";
    SCHEDULE_CHANGED = "QMC3402", 0, "Scheduled operation &1 changed.",
        "The scheduler runs it as changed, in your name from now on.",
        "None.";
    SCHEDULE_REMOVED = "QMC3403", 0, "Scheduled operation &1 removed.",
        "It is not run again; its runs stay in the list.",
        "None.";
    RAN_NOW = "QMC3404", 0, "Scheduled operation &1 run now: &2.",
        "The scheduler ran it in its owner's name.",
        "See the run's status in the list.";
    SCHEDULE_REFUSED = "QMC3405", 30, "The operation was not scheduled: &1",
        "The scheduler refused it.",
        "Correct the operation and add it again.";
    OWNER_MAY_NOT = "QMC3406", 30, "User &1 may no longer &2: the scheduled run was refused.",
        "A scheduled operation runs in its owner's name, and the owner's role is checked when it runs: it no longer has the task, or the user is disabled or gone.",
        "Change the operation to an owner who may do it, or give the role the task.";
    NO_SCHEDULED_BACKUP = "QMC3407", 20, "A backup of the console's data is not scheduled: it leaves the console through a browser, under a passphrase.",
        "A backup is downloaded from Backup and restore, sealed under a passphrase typed there; the console sends it nowhere by itself.",
        "Take the backup from Backup and restore.";
    SCHEDD_DOWN = "QMC3409", 30, "The scheduler did not answer: &1.",
        "The console's service that keeps and runs scheduled operations is not running, or is starting.",
        "Wait a moment and show the page again. If it goes on, see the reference code history of the console.";
    FUNCTION_ACCEPTED = "QMC5003", 0, "Function &1 selected for partition &2; the managed system accepted the request.",
        "The host carried out the control panel function; the result shows what it displays and the code before and after.",
        "None.";
    FUNCTION_REFUSED = "QMC5010", 30, "Function &1 for partition &2 not performed. The managed system's own messages follow.",
        "The host refused the control panel function: it does not have it, or its keylock or the partition's state does not allow it.",
        "Act on the system's messages below this one and try again.";

    // Reference codes.
    NO_EXPLANATION = "QMC1201", 10, "This release has no explanation for reference code &1. Its text is shown as the system posted it.",
        "The code is in neither the QS/VS catalogue nor the QS/400 table this console was built with: a system of a later release may post codes this console does not know.",
        "See the system's own documentation for the code, or update the console.";
    DATE_NOT_VALID = "QMC1202", 30, "&1 is not a date in this console's format.",
        "A date filter was typed in another format than the one on Language, locale and session.",
        "Type the date as the console shows dates, or leave the field empty.";

    // Power.
    POWER_ON_NOT_HERE = "QMC2201", 20, "Power on of &1 is not in this release: a powered-off machine does not answer QSI, and this console has no way to its BMC.",
        "A managed system answers QSI only while it runs; powering one on takes its BMC, which this console is not given.",
        "Power the machine on at its BMC or at the machine. The console reconnects when it answers.";
    POWER_OFF_STARTED = "QMC2202", 0, "Power off of &1 started. Its partitions are shut down first unless Immediate was chosen.",
        "The managed system accepted the power off; its state and codes follow on its pages.",
        "None.";
    RESTART_STARTED_SYSTEM = "QMC2203", 0, "Restart of &1 started. The console reconnects when it answers again.",
        "The managed system accepted the restart; it is shown as not connected until it answers.",
        "None.";
    POWER_REFUSED = "QMC2210", 30, "&1 request for &2 not performed. The managed system's own messages follow.",
        "The managed system refused the request.",
        "See the system's messages below this one, act on them, and try again.";

    SYSTEM_DESCRIBED = "QMC1007", 0, "Description of managed system &1 changed.",
        "The console keeps its own note about the system; nothing on the system is changed.",
        "None.";
    NAME_IS_THE_SYSTEMS = "QMC1008", 30, "The name of managed system &1 is the system's own and is changed at the system.",
        "QSI does not change a managed system's name; the console shows the name the system gives.",
        "Change the name at the managed system; the console shows the new name once it is paired again with Reset connection.";
    POWER_ON_PARAMETERS = "QMC2220", 20, "The IPL side, keylock and boot mode of &1 are set at the system.",
        "A managed system reports its slot and keylock over QSI and takes no change of them: its IPL side moves only when a code update is activated, its keylock and boot mode at its Service Tools.",
        "Set them at the managed system's Service Tools.";
    POWER_ON_SET = "QMC2221", 0, "Power-on settings of &1 changed: the partitions to start and the default power off.",
        "The managed system took the settings; they apply at its next power on and power off.",
        "None.";

    SELECT_SYSTEM = "QMC1006", 30, "Select a managed system for &1.",
        "The action acts on the systems selected in the list, and none was.",
        "Select the system's box in the list, then choose the action again.";

    SYSTEM_ADDED = "QMC1005", 0, "Managed system &1 added. The console keeps its session open and shows its state.",
        "The managed system was paired and is in the Servers list.",
        "None.";

    // The managed-systems service itself.
    MGMT_DOWN = "QMC1120", 30, "The managed-systems service did not answer: &1.",
        "The console's service that keeps the managed systems' sessions is not running, or is starting.",
        "Wait a moment and show the page again. If it goes on, see the reference code history of the console.";

    // Terminal windows.
    TOO_MANY_WINDOWS = "QMC3003", 30, "&1 terminal windows are open, the most at once.",
        "The console shares each partition's console among the windows that have it open, up to a limit across every system.",
        "Close a terminal window that is no longer needed, then open this one again.";

    // Managed systems and their connections.
    SYSTEM_CONNECTED = "QMC1001", 0, "Connection to managed system &1 established. Service processor version &2.",
        "The console paired with the managed system and opened its session; the version is the release the system says it runs.",
        "None.";
    SYSTEM_REMOVED = "QMC1002", 0, "Managed system &1 removed.",
        "The console forgot the system and its pinned certificate, and closed its session.",
        "To manage it again, add it with a new pairing password from the system.";
    SYSTEM_PAIRED_AGAIN = "QMC1003", 0, "Managed system &1 paired again.",
        "The console paired with the system with a new password and pinned the certificate it showed.",
        "None.";
    SYSTEM_TESTED = "QMC1004", 0, "Connection to managed system &1 tested: &2.",
        "Each step of a new connection to the system was timed.",
        "None.";
    NO_ANSWER = "QMC1101", 30, "Managed system at &1 did not answer: &2.",
        "The console could not open a connection to the address and port, or the connection was closed before QSI began.",
        "Check the address and the port, that the system is powered on and has its QSI service, and that the console's network reaches it.";
    TURNED_AWAY = "QMC1102", 30, "Managed system at &1 turned the console away: no pairing password is out there, or it expired, or its attempts were spent.",
        "A managed system lets a console it has not paired in only while a pairing password made there is out.",
        "At the managed system, make a new pairing password and type it here within its time.";
    NOT_PAIRED_HERE = "QMC1103", 30, "Managed system &1 is already managed by this console.",
        "A system of that name or address is in the list.",
        "Use Reset connection to pair it again.";
    NOT_QSI = "QMC1104", 30, "Managed system at &1 does not answer QSI: &2.",
        "Only a system that answers QSI can be managed: a QS/VS host, or a standalone QS/400 with its QSI server started.",
        "Check the address and port, and that the system's QSI server is started (on a standalone QS/400, the *QSI server).";
    CERTIFICATE_CHANGED = "QMC1105", 30, "Managed system &1 answered with a certificate other than the one pinned when it was paired. It is not trusted.",
        "The system's certificate is not the one the console pinned: it was installed again, or something else answers at its address.",
        "Find out why at the system. When it is the system, use Reset connection with a new pairing password.";
    NOT_CONNECTED = "QMC1106", 30, "Managed system &1 is not connected: &2.",
        "The console has no session with the system now; it tries again by itself.",
        "See the reason, and Connectivity for when it last answered.";
    TOO_MANY_SYSTEMS = "QMC1107", 30, "This console manages &1 systems, the most it may.",
        "The limit of managed systems was reached.",
        "Remove a system it no longer needs to manage.";
    NO_SUCH_SYSTEM = "QMC1108", 30, "No managed system &1.",
        "The system is not in the list, or was removed.",
        "Choose a system from the Servers list.";
    SERIAL_CHANGED = "QMC1109", 20, "Managed system &1 says it is serial number &2, not &3 as when it was paired.",
        "The system answers with the pinned certificate but another serial number.",
        "Find out at the system why its serial number changed; use Reset connection when it is right.";
    NO_REPLY = "QMC1110", 30, "Managed system &1 did not answer in time.",
        "The system took longer than the console waits.",
        "Try again; see Connectivity for the system's connection.";
    NOT_CONFIRMED = "QMC1111", 30, "Pairing with the managed system at &1 did not confirm: the password is not the one it showed.",
        "The system's confirmation did not hold for the password typed; the system counted the attempt.",
        "Type the password the system shows again, or make a new one there.";
    SETUPS_BUSY = "QMC1112", 30, "The console is setting up as many connections as it may at once. Try again shortly.",
        "Too many connections were being set up at the same time.",
        "Wait a moment and try again.";
    PASSWORD_NEEDED = "QMC1113", 30, "The system's pairing password is required.",
        "The form was sent without the pairing password shown at the managed system.",
        "Type the password the managed system shows.";
    LACKS = "QMC1114", 30, "Managed system &1 does not offer &2.",
        "The system's QSI service, by what its hello said, does not answer this request: it is of an older release, or of a kind that has no such function.",
        "Do it at the managed system, or update it to a release that offers it.";
    NOT_CARRIED = "QMC1115", 30, "The console does not send this request to managed system &1.",
        "The managed-systems service carries only the requests the console's pages send.",
        "Use the page that does it.";

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
        "A managed system is added with the pairing password its operator makes on it.",
        "Choose Add managed system on the Servers or Connectivity page.";

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

    // Code updates: the console's and its hosts', and the fleet's levels.
    CHECKED = "QMC7001", 0, "Checked &1 at &2. &3",
        "The console read each product stream's index or catalog from the update server and verified it against the keys built into its image.",
        "None.";
    AT_NEWEST = "QMC7002", 10, "The &2 stream offers nothing newer than &1.",
        "The newest release the stream names, that updates from what runs, is what runs.",
        "None; check again later.";
    DOWNLOAD_FIRST = "QMC7003", 30, "Download &1 before it is installed: it is not on the console.",
        "An update is installed from the image the console downloaded and checked against the stream's index.",
        "Choose Download, then Install when its status says it is downloaded.";
    PARTITIONS_RUN = "QMC7004", 30, "Partitions run on &1: &2. Shut them down, or install saying they are to be shut down.",
        "Activating a host's update restarts the host, which ends every partition.",
        "Shut the partitions down, or choose on the confirmation that they are shut down through their operating systems first.";
    NOT_CHECKED = "QMC7005", 30, "The &1 stream has not been checked, or nothing of it verified.",
        "The console knows what a stream offers only from an index that verified.",
        "Choose Check now on Code updates.";
    NO_QSI_UPDATE = "QMC7006", 20, "&1 takes its fixes by its own means: the console updates no standalone system.",
        "A standalone QS/400 is fixed by its own PTF commands; QSI gives it no code update.",
        "Use the system's own fixes page; Updates links to it.";
    DOWNLOAD_BEGUN = "QMC7007", 0, "Download of &1 begun.",
        "The download service fetches the file and checks it against the stream's index before anything may read it.",
        "Show the page again to see when it is downloaded.";
    DOWNLOAD_GONE = "QMC7008", 0, "&1 removed from the console.",
        "The downloaded file was deleted.",
        "None.";
    UPDATE_SETTINGS = "QMC7009", 0, "Update settings changed: server &1, check &2, download &3.",
        "The download service keeps the settings and uses them from now on.",
        "None.";
    INSTALLING = "QMC7010", 0, "Installing &1 on &2. This page reloads every 10 seconds.",
        "The image is brought to the system, verified there, written to its other slot and activated.",
        "Wait for the steps to end; the page shows each.";
    UPDATE_CONFIRMED = "QMC7011", 0, "&1 confirmed &2.",
        "The system restarted into its other slot, which started and became its default.",
        "None.";
    UPDATE_ROLLED_BACK = "QMC7012", 30, "&1 rolled back to its other slot: &2.",
        "The slot the update was written to did not confirm; the system came back on the slot it ran before, as it was.",
        "See the system's serviceable events and its dump; install the update again when the cause is known.";
    NO_PLAN_SAVED = "QMC7013", 20, "No system plan is saved before the install: system plans come with a later release.",
        "The design saves a plan of the host first; this release has no system plans.",
        "None.";
    CONSOLE_RESTARTING = "QMC7014", 0, "The console is restarting into &1. Sign on again when it is back.",
        "The update is installed in the console's other slot; it confirms when its web server answers, or the console comes back as it was.",
        "Sign on again in a few minutes; Code updates then says how the update ended.";
    SERVICE_DOWN = "QMC7015", 30, "&1 does not answer: &2.",
        "A service of the console that the page or the schedule needs is not running, or is starting.",
        "Wait a moment and try again; if it goes on, see the console's serviceable events.";
    NO_NOTES = "QMC7016", 30, "The release notes of &1 are not on the console.",
        "The notes are fetched when a stream is checked and its index names them.",
        "Choose Check now on Code updates.";
    INSTALL_BEGUN = "QMC7017", 0, "Install of &1 on &2 begun.",
        "The image is on its way to the system's other slot.",
        "Follow it on the progress page.";
    UPDATE_ACTIVATED = "QMC7018", 0, "&1 is restarting into &2.",
        "The update was activated: the system restarts into its other slot.",
        "Follow it on the progress page; it confirms or rolls back by itself.";
    UPDATE_CANCELLED = "QMC7019", 0, "The pending update of &1 was cancelled.",
        "The slot it was written to is no longer pending; nothing else changed.",
        "None.";

    // Serviceable events and dumps.
    EVENT_CLOSED = "QMC7401", 0, "Serviceable event &1 closed.",
        "The system that owns the problem closed it, with your user name.",
        "None.";
    EVENT_NOTED = "QMC7402", 0, "Note added to serviceable event &1.",
        "The system that owns the problem added it to the analysis log, with your user name.",
        "None.";
    DUMP_DELETED = "QMC7501", 0, "Dump &1 deleted from &2.",
        "The system that held the dump deleted it.",
        "None.";
    SELECT_DUMP = "QMC7502", 30, "Select a dump for &1.",
        "The action needs a dump and none was chosen.",
        "Check the box of a dump and choose the action again.";
    NO_REMOTE_COPY = "QMC7503", 20, "Copying a dump to a remote host is not in this release. Download the dump instead.",
        "The console sends nothing to a host it does not manage; a dump leaves it through a browser.",
        "Download the dump and copy it where it is needed.";
    NO_SUCH_DUMP = "QMC7504", 30, "No dump &1 on &2.",
        "The dump is not in what the console keeps of the system: it was deleted, or put out for the system's limits.",
        "Show Manage dumps again.";
    DUMP_TAKEN = "QMC7505", 0, "Console dump &1 begun.",
        "PID 1 is writing the dump of the console's service data.",
        "Show the page again to see it listed.";
    ONE_DUMP = "QMC7506", 30, "Select one dump to download.",
        "A download is one file.",
        "Check the box of one dump and choose Download again.";

    // The console's own code update (`base/update`'s refusals).
    UPD_SIGNATURE = "QMC7101", 30, "Update image &1 is not signed by a key this console trusts.",
        "An image is installed only if its manifest is signed by a key the running release trusts, and nothing turns that off.",
        "Get the image from the qsmc product stream again, or from media of a QS/MC build.";
    UPD_DAMAGED = "QMC7102", 30, "Update image &1 is damaged: &2.",
        "The image does not read, or a size or a hash in it is not its manifest's.",
        "Download or copy it again; if it is refused again, get it from the product stream again.";
    UPD_PRODUCT = "QMC7103", 30, "Update image &1 is for &2, and this console is &3.",
        "An update image updates the product it was built for; a test image is updated only by a test image.",
        "Use an update image of QS/MC.";
    UPD_OLDER = "QMC7104", 30, "Update image &1 holds &2, which is older than &3, which runs.",
        "An older release is installed only when the stage allows it.",
        "Stage the image again allowing an older release, if that is what is wanted.";
    UPD_STATE = "QMC7105", 30, "Update image &1 holds &2, which cannot read the console's state: &3.",
        "The release in the image reads an older form of that file than the console has. It is never installed: the console would not start with its state.",
        "Install a later release of QS/MC.";
    UPD_NOTHING_STAGED = "QMC7106", 30, "No update image is staged.",
        "Install needs a staged, verified image.",
        "Stage one first: from a downloaded update on Code updates, or from a removable volume at the local console's option 8.";
    UPD_STAGED_ALREADY = "QMC7107", 30, "Update image &1 is staged already.",
        "One image is staged at a time.",
        "Install it, or discard it first.";
    UPD_NOT_VERIFIED = "QMC7108", 30, "Update image &1 has not verified: &2.",
        "Only an image that passed every check is installed.",
        "Discard it and stage another.";
    UPD_CUT = "QMC7109", 30, "Update image &1 ended after &2 of its &3 bytes.",
        "The image did not arrive whole; nothing of it was kept.",
        "Stage it again.";
    UPD_BUSY = "QMC7110", 30, "An update is &1.",
        "One install or activation at a time.",
        "Wait for it to end.";
    UPD_NOTHING_PENDING = "QMC7111", 30, "No code slot is pending.",
        "Activate and cancel need a slot an install wrote.",
        "Install a verified image first: the slot it is written to is then pending.";
    UPD_PARTITIONS_RUN = "QMC7112", 30, "Work is running: &1.",
        "An activation restarts the console.",
        "Wait for it to end, then activate again.";
    UPD_PENDING = "QMC7113", 30, "Code slot &1 is pending already.",
        "An install while a slot is pending would write over it.",
        "Activate the pending slot, or cancel it, before another image is installed.";
    UPD_NOT_WRITTEN = "QMC7114", 30, "Code slot &1 could not be written: &2.",
        "The slot is not pending, and the slot that runs is as it was.",
        "Install the image again; if it fails again the load-source disk named may be failing.";
    UPD_RECORD = "QMC7115", 30, "The A/B record could not be changed: &1.",
        "PID 1 could not write the record of the load source; the slots are as they were.",
        "Display the code levels at the local console's option 8 and try again.";
    UPD_NOT_KEPT = "QMC7116", 30, "Update image &1 could not be kept on the data file system: &2.",
        "Nothing of it was kept.",
        "Make room on the data file system, then stage it again.";
    UPD_NAME = "QMC7117", 30, "&1 is not a name and size an update image is staged by.",
        "A stage names the image in 1 to 64 printable characters and gives its size, more than 0.",
        "Stage a file of the qsmc stream, or one an update build made.";

    // The console's own problem log (`base/problem`'s refusals).
    PRB_NO_PROBLEM = "QMC7301", 30, "There is no problem &1.",
        "The console's problems are numbered from 1 and no number is used twice; the log keeps 500.",
        "Choose a problem from Serviceable events.";
    PRB_CLOSED = "QMC7302", 30, "Problem &1 was closed already, by &2.",
        "A closed problem stays closed. When its condition is seen again a new problem is opened.",
        "None.";
    PRB_HOLDS = "QMC7303", 30, "The condition of problem &1 still holds.",
        "A problem is closed once its condition no longer holds, which its analysis log says.",
        "Close it anyway, through the confirmation, when the condition is understood and will be dealt with later.";
    PRB_NOT_KEPT = "QMC7304", 30, "The problem log could not be written.",
        "The data file system did not take the change; nothing changed.",
        "Make room on the data file system and try again.";
    PRB_NOTE_INVALID = "QMC7305", 30, "A note of &1 bytes is not valid.",
        "A note is one line of 1 to &2 bytes, without control characters.",
        "Type a shorter note on one line.";

    // The console's own problems: each condition's description, what
    // to do its second level.
    PRB_PANICKED = "QMC7311", 30, "The console restarted after its kernel panicked: &1.",
        "The kernel stopped the machine. What it kept of the panic is in /data/log/panics and in the dump taken at the next IPL.",
        "Report this problem with the console dump of the same time.";
    PRB_NOT_MOUNTED = "QMC7312", 30, "Kernel file systems were not mounted: &1.",
        "PID 1 could not mount a file system the console needs; it runs without it.",
        "Report this problem with a console dump.";
    PRB_PROCESSOR = "QMC7313", 30, "The processor lacks what the cryptography needs: &1.",
        "The web server and QSI need AVX2, ADX, AES and PCLMULQDQ.",
        "Run the console on a processor with them, or give its virtual machine the host's processor model.";
    PRB_RECORD = "QMC7314", 30, "The A/B record of the load source was not valid: &1.",
        "The record that says which code slot is the default could not be read or written.",
        "If this is seen at every IPL, the load-source disk may be failing: replace it.";
    PRB_SLOT_FELL_BACK = "QMC7315", 30, "Code slot &1 did not confirm, and the console went back to the other slot: &2.",
        "The update in the slot did not start its web server; the state is as it was before the update, and a console dump was taken.",
        "Report this problem with the console dump of the same time before installing the update again.";
    PRB_DATA_NOT_MOUNTED = "QMC7316", 30, "The data file system was not mounted: &1.",
        "Nothing is kept across a restart while it is not.",
        "Check the load-source disks and their connections; the console never reformats the data file system.";
    PRB_RESTORE_TAKEN_BACK = "QMC7317", 30, "A restore was taken back: &1.",
        "The restored console did not start its web server, or the restore was cut short; the state before it is back.",
        "Restore the backup again; if it is taken back again, report this problem with a console dump.";
    PRB_CLOCK = "QMC7318", 30, "The clock could not be set from the hardware clock: &1.",
        "The console's time is wrong until a time server sets it.",
        "Configure a time server; if the machine has a real-time clock, check its battery.";
    PRB_STATE = "QMC7319", 30, "A state file was not valid, and its defaults are in use: &1.",
        "The file named was refused when it was read.",
        "Set those settings again.";
    PRB_IDENTITY = "QMC7320", 30, "The console's identity could not be made: &1.",
        "The key pair and certificate the console is known by could not be written.",
        "Check that the data file system has room; it is made again at the next IPL.";
    PRB_MGMT_NOT_SHARED = "QMC7321", 30, "The managed-systems service's files could not be given to it: &1.",
        "The managed-systems service does not start, and the console manages no system.",
        "Check the data file system, then report this problem with a console dump.";
    PRB_SERVICE_STOPPED = "QMC7322", 30, "Service &1 failed five times in ten minutes and is not started again.",
        "The service ended again and again; PID 1 took a console dump.",
        "Restart the console; if the service stops again, report this problem with the dump.";
    PRB_BOOT_ORDER = "QMC7323", 30, "The firmware's boot order was not written for code slot &1: &2.",
        "The record holds the slot; the firmware may start the other first.",
        "In the firmware's setup, put the confirmed slot's entry first.";
    PRB_NO_INTERFACE = "QMC7324", 30, "No management interface was found: &1.",
        "The console has no Ethernet link with carrier, or not the device configured.",
        "Connect a cable to a supported port, or correct the device in the network settings.";
    PRB_NO_CARRIER = "QMC7325", 30, "Management interface &1 has no carrier.",
        "The interface has had no carrier for 30 seconds.",
        "Check the cable and the switch port of the interface.";
    PRB_NO_ADDRESS = "QMC7326", 30, "Management interface &1 has no address.",
        "The interface has carrier but no address.",
        "Check that a DHCP server answers on its network, or set a static address.";
    PRB_SET = "QMC7327", 30, "The mirrored load source was not assembled: &1.",
        "The console could not assemble its two load-source disks.",
        "Check that both disks are present and connected, then restart.";
    PRB_UNIT_FAILED = "QMC7328", 30, "Load-source unit &1 is missing or failed.",
        "The console runs on the other unit without protection.",
        "Return or replace the disk of the unit, then rebuild it at the local console's option 6.";
    PRB_REBUILD = "QMC7329", 30, "The rebuild of load-source unit &1 did not finish: &2.",
        "The unit is still out of step; it is tried again at the next IPL.",
        "If it fails again, replace its disk.";
    PRB_APART = "QMC7330", 30, "The load-source units each ran without the other: &1.",
        "Each disk ran without the other; one of them must be chosen.",
        "Choose the unit to keep at the local console; the other is written again from it.";
    PRB_BOOT_ENTRIES = "QMC7331", 30, "The firmware took no boot entry for a new load-source unit: &1.",
        "The new unit holds both slots but the firmware has no entry for it.",
        "Add one in the firmware's setup.";
    PRB_MGMT_STATE = "QMC7332", 30, "The managed systems were not read: &1.",
        "The console manages none of the systems the file held.",
        "Restore a backup of the console, or add the systems again with new pairing passwords.";
    PRB_CERT_CHANGED = "QMC7333", 30, "Managed system &1 answered with another certificate: &2.",
        "The system's certificate is not the one pinned when it was paired; the console does not trust it.",
        "Find out at the system why it changed; when it is the system, pair it again with Reset connection.";
    PRB_NOT_REACHED = "QMC7334", 30, "Managed system &1 has not been reached for longer than its setting: &2.",
        "The console has tried and not reached the system.",
        "Check the system and the network between it and the console; Connectivity shows why the connection fails.";
    PRB_DATA_FULL = "QMC7335", 30, "The data file system is nearly full: &1.",
        "The console keeps its state, logs, downloads and dumps there.",
        "Delete the downloaded updates and dumps it no longer needs.";
    PRB_DUMP_FAILED = "QMC7336", 30, "A console dump could not be written: &1.",
        "Nothing of it was kept.",
        "Make room on the data file system, then take the dump again.";
    PRB_CERT_ENDING = "QMC7337", 30, "The web certificate ends soon: &1.",
        "Browsers refuse the console's certificate once it has ended.",
        "Make a new certificate or import one on Certificates.";
    PRB_INSTALL_FAILED = "QMC7338", 30, "An update could not be installed into code slot &1: &2.",
        "The slot is not pending, and the slot that runs is as it was.",
        "Check the load-source disks named, then install the update again.";
    PRB_DOWNLOAD_BAD = "QMC7339", 30, "A downloaded file did not match its product stream's index: &1.",
        "Its size or SHA-256 is not the one the signed index names; it was removed.",
        "Download it again; if it fails again, the server or the path to it changes what it sends.";
    PRB_PID1 = "QMC7340", 30, "The console's first process failed: &1.",
        "PID 1 panicked and the console parked.",
        "Report this problem with the text after it and a console dump.";
    PRB_STORAGE_LOST = "QMC7341", 30, "The storage service of the mirrored load source stopped: &1.",
        "The data file system has no server and nothing more was written.",
        "Restart the console, then report this problem with a console dump.";

    // Product streams and `fetchd`.
    STREAM_NOT_SIGNED = "QMC7201", 30, "The &1 stream's index is not signed by a key this console trusts.",
        "An index or catalog is used only if its signature verifies against the keys built into the console's image.",
        "Check that the update server is the product stream's. Nothing was taken from it.";
    STREAM_OLDER = "QMC7202", 30, "The &1 stream's index has sequence &2, older than &3, which was seen already.",
        "A stream's sequence never goes back: an older index may hide an update that fixes something.",
        "Check that the update server is the product stream's, not an old copy of it.";
    STREAM_FORM = "QMC7203", 30, "The &1 stream's index does not read: &2.",
        "It is signed, but its contents are not an index of this format.",
        "Check that the update server is the product stream's.";
    STREAM_NOT_REACHED = "QMC7204", 30, "The update server &1 was not reached: &2.",
        "The console could not connect to it, it did not answer in time, or it answered with an error.",
        "Check the server's address and the console's route and name servers, then check again.";
    FILE_BAD = "QMC7205", 30, "&1 does not match the stream's index: &2. It was removed.",
        "The file's size or SHA-256 is not the one the signed index names.",
        "Download it again; if it fails again, the server or the path to it changes what it sends.";
    FILE_TOO_LARGE = "QMC7206", 30, "&1 is &2 bytes, more than the &3 the console downloads.",
        "The download area of the data file system has a limit.",
        "Delete downloaded updates the console no longer needs.";
    FETCH_DOWN = "QMC7207", 30, "The download service does not answer: &1.",
        "fetchd, the only service that reaches the update server, is not running.",
        "Restart the console; if it does not answer again, report this with a console dump.";
    SERVER_NOT_VALID = "QMC7208", 30, "&1 is not an update server's address.",
        "The address is https:// or http:// with a host name or address, and a path.",
        "Type the server's address, for example https://updates.quartz.systems.";
    NOT_IN_STREAM = "QMC7209", 30, "&1 is not in the &2 stream's index.",
        "Only a file the signed index names is downloaded.",
        "Check now, then download what the stream offers.";
    NOT_DOWNLOADED = "QMC7210", 30, "&1 is not a host update the console downloaded.",
        "A host is brought an update image the download service fetched from the qsvs stream and checked against its index.",
        "Download the update on Code updates first.";
}

/// `d` in `base/msg`'s form: its first-level text, and what to do as
/// the second.
pub const fn base_of(d: &Def) -> msg::Message {
    msg::Message {
        id: d.id,
        text: d.text,
        help: d.recovery,
    }
}

/// The catalogue's messages in `base/msg`'s form, which the base crates
/// refuse with: the first-level text, and what to do as the second.
pub mod base {
    use super::base_of as of;

    macro_rules! base {
        ($($name:ident),* $(,)?) => {
            $(
                #[doc = concat!("`", stringify!($name), "` as `base/msg` has it.")]
                pub const $name: msg::Message = of(&super::$name);
            )*
        };
    }

    base!(
        UPD_SIGNATURE,
        UPD_DAMAGED,
        UPD_PRODUCT,
        UPD_OLDER,
        UPD_STATE,
        UPD_NOTHING_STAGED,
        UPD_STAGED_ALREADY,
        UPD_NOT_VERIFIED,
        UPD_CUT,
        UPD_BUSY,
        UPD_NOTHING_PENDING,
        UPD_PARTITIONS_RUN,
        UPD_PENDING,
        UPD_NOT_WRITTEN,
        UPD_RECORD,
        UPD_NOT_KEPT,
        UPD_NAME,
        PRB_NO_PROBLEM,
        PRB_CLOSED,
        PRB_HOLDS,
        PRB_NOT_KEPT,
        PRB_NOTE_INVALID,
        PRB_PANICKED,
        PRB_NOT_MOUNTED,
        PRB_PROCESSOR,
        PRB_RECORD,
        PRB_SLOT_FELL_BACK,
        PRB_DATA_NOT_MOUNTED,
        PRB_RESTORE_TAKEN_BACK,
        PRB_CLOCK,
        PRB_STATE,
        PRB_IDENTITY,
        PRB_MGMT_NOT_SHARED,
        PRB_SERVICE_STOPPED,
        PRB_BOOT_ORDER,
        PRB_NO_INTERFACE,
        PRB_NO_CARRIER,
        PRB_NO_ADDRESS,
        PRB_SET,
        PRB_UNIT_FAILED,
        PRB_REBUILD,
        PRB_APART,
        PRB_BOOT_ENTRIES,
        PRB_MGMT_STATE,
        PRB_CERT_CHANGED,
        PRB_NOT_REACHED,
        PRB_DATA_FULL,
        PRB_DUMP_FAILED,
        PRB_CERT_ENDING,
        PRB_INSTALL_FAILED,
        PRB_DOWNLOAD_BAD,
        PRB_PID1,
        PRB_STORAGE_LOST,
    );
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
