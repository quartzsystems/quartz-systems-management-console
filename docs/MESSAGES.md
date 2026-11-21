# Messages

Every message a page of the web interface shows is a `QMC` message of
the catalogue in `web/msgs` (`qmcmsg`). A page shows its first-level
text in the message area; its identifier links to the help window,
which shows the second-level text: the message again, its cause and
what to do. `&1`, `&2` are the values a message is shown with. A test
holds this table to the catalogue.

Severity: 0 is information or a change made, 10 something to know, 20
a function the design shows that this release does not provide, 30 a
request refused.

| Range | What |
| --- | --- |
| `QMC1001`-`QMC1008` | managed systems added, paired again, tested, removed, described, a name that is the system's |
| `QMC1101`-`QMC1120` | what a managed system's connection and pairing failed with, what it does not offer, and the managed-systems service not answering |
| `QMC1201`-`QMC1202` | reference codes: one this release cannot explain, a date filter |
| `QMC2201`-`QMC2221` | power on, off and restart of a managed system, and its power-on parameters |
| `QMC2401`-`QMC2441` | a host's virtual resources: uploads, images, virtual disks, pools, switches, tape, processor pools |
| `QMC3002` | a partition waiting for a console |
| `QMC3003` | terminal windows: too many open |
| `QMC3101`-`QMC3133` | partitions: activated, shut down, changed, made, deleted, and what a host refuses of them |
| `QMC3201`-`QMC3205` | partition profiles |
| `QMC3301`-`QMC3303` | custom groups |
| `QMC3401`-`QMC3409` | scheduled operations |
| `QMC3002` | 0 | Partition &1 is waiting for a console (&2). Open a terminal window to continue the IPL. | The partition's IPL stopped until a console connects to it. | Open a terminal window to the partition. |
| `QMC3101` | 0 | Activation of partition &1 started. | The host took the request; the partition's codes follow its IPL. | None. |
| `QMC3102` | 0 | Shut down of partition &1 started. | The host, or the partition's system, took the request. | None. |
| `QMC3103` | 0 | Restart of partition &1 started: it is shut down and activated again. | The host took the request. | None. |
| `QMC3104` | 30 | Select a partition for &1. | The action needs a partition and none was chosen. | Check the box of a partition and choose the action again. |
| `QMC3105` | 30 | No partition &1 on managed system &2. | The partition is not in what the console keeps of the host: it was deleted, or the host has not said it yet. | Choose a partition from the Partitions list. |
| `QMC3110` | 30 | Activate request for &1 failed: &2. | The host refused to activate the partition; its messages follow this one. | Act on the host's messages, then activate the partition again. |
| `QMC3120` | 0 | Partition &1 changed. | The host took the change. | None. |
| `QMC3121` | 10 | The changes to the profile of running partition &1 take effect at its next activation. | A host gives a partition its profile's resources when it activates it. | Restart the partition when the change is wanted now. |
| `QMC3122` | 0 | The processing units, sharing and weight of running partition &1 are changed now. | A host changes a running shared partition's entitlement at once. | None. |
| `QMC3123` | 20 | Open firmware is not a boot mode of &1. | A QS/VS host has no open firmware: a partition starts normally or in service mode with a manual IPL. | Choose Normal or Service - manual IPL. |
| `QMC3124` | 20 | The partition's console is the terminal window. | A QS/VS host gives a partition no physical console adapter: its console is the virtual serial adapter 0. | Open a terminal window to reach the partition's console. |
| `QMC3125` | 20 | The console's serial adapters are fixed. | Adapters 0 and 1 are the partition's console, the terminal window; they are not added, changed or removed. | None. |
| `QMC3126` | 20 | Partition &1 has not said its address. | The partition's system says its addresses over its service channel; it has not, or the channel is down. | Reach Navigator at the partition's own address. |
| `QMC3130` | 0 | Partition &1 (ID &2) created, Not activated. | The host made the partition and its first profile. | Activate it when it is wanted. |
| `QMC3131` | 30 | Partition ID &1 is in use: partition &2 has it. | Each partition of a host has an ID of its own. | Choose a free ID; the step shows the next one. |
| `QMC3132` | 30 | No managed system can have a partition made from this console. | A partition is made on a connected QS/VS host that offers it. | Add a QS/VS host, or wait until it is connected. |
| `QMC3133` | 0 | Partition &1 deleted. | The host deleted the partition and its profiles, and its virtual disks when chosen. | None. |
| `QMC3201` | 30 | Profile &1 was changed since this page was read. | Another session changed the profile, so the change was refused. The page shows the profile as it is now, with your entries. | Check your entries against the profile and send them again. |
| `QMC3202` | 0 | The running configuration saved as profile &1. | The host saved what the partition holds now as the profile. | None. |
| `QMC3203` | 0 | Profile &1 is the default. | An activation uses it unless another is chosen. | None. |
| `QMC3204` | 0 | Profile &1 copied as &2. | The host made the new profile. | None. |
| `QMC3205` | 0 | Profile &1 deleted. | The host deleted the profile. | None. |
| `QMC3301` | 0 | Custom group &1 saved. | The console keeps the group and its members. | None. |
| `QMC3302` | 30 | A custom group named &1 exists. | Each group has a name of its own. | Choose another name, or edit that group. |
| `QMC3303` | 0 | Custom group &1 deleted. | The group is gone; its systems and partitions are as they were. | None. |
| `QMC3401` | 0 | &1 scheduled, first at &2. | The scheduler keeps the operation and runs it at its times in your name. | None. |
| `QMC3402` | 0 | Scheduled operation &1 changed. | The scheduler runs it as changed, in your name from now on. | None. |
| `QMC3403` | 0 | Scheduled operation &1 removed. | It is not run again; its runs stay in the list. | None. |
| `QMC3404` | 0 | Scheduled operation &1 run now: &2. | The scheduler ran it in its owner's name. | See the run's status in the list. |
| `QMC3405` | 30 | The operation was not scheduled: &1 | The scheduler refused it. | Correct the operation and add it again. |
| `QMC3406` | 30 | User &1 may no longer &2: the scheduled run was refused. | A scheduled operation runs in its owner's name, and the owner's role is checked when it runs: it no longer has the task, or the user is disabled or gone. | Change the operation to an owner who may do it, or give the role the task. |
| `QMC3407` | 20 | A backup of the console's data is not scheduled: it leaves the console through a browser, under a passphrase. | A backup is downloaded from Backup and restore, sealed under a passphrase typed there; the console sends it nowhere by itself. | Take the backup from Backup and restore. |
| `QMC3408` | 20 | Installing a code update is not scheduled in this release. | Code updates arrive with a later release of this console. | None. |
| `QMC3409` | 30 | The scheduler did not answer: &1. | The console's service that keeps and runs scheduled operations is not running, or is starting. | Wait a moment and show the page again. If it goes on, see the reference code history of the console. |
| `QMC4001`-`QMC4002` | a terminal window's console closed or disconnected |
| `QMC5003`, `QMC5010` | control panel functions carried out and refused |
| `QMC8001`-`QMC8011` | users, roles and form values |
| `QMC8100`-`QMC8103` | Network settings |
| `QMC8200`-`QMC8201` | Date and time |
| `QMC8300`-`QMC8305` | Certificates |
| `QMC8400`-`QMC8406` | Backup and restore |
| `QMC8500`-`QMC8502` | Shut down or restart |
| `QMC8600` | Language, locale and session |
| `QMC8700` | what the design shows and V1 does not do |
| `QMC9001`-`QMC9031` | sign-on, sessions, the frame's own answers |

| Message | Severity | Text | Cause | Recovery |
| --- | --- | --- | --- | --- |
| `QMC4001` | 30 | Console connection to partition &1 closed: &2. | The console went: the managed system closed it, another console took it over, the partition ended, or the session to the system was lost. | Choose Connect to open it again; with Take over on Open terminal window to take it from another console. |
| `QMC4002` | 0 | Terminal window to &1 disconnected. | You disconnected the window; the console stays with the managed system and any other window that has it open. | Choose Connect to open it again. |
| `QMC5003` | 0 | Function &1 selected for partition &2; the managed system accepted the request. | The host carried out the control panel function; the result shows what it displays and the code before and after. | None. |
| `QMC5010` | 30 | Function &1 for partition &2 not performed. The managed system's own messages follow. | The host refused the control panel function: it does not have it, or its keylock or the partition's state does not allow it. | Act on the system's messages below this one and try again. |
| `QMC1201` | 10 | This release has no explanation for reference code &1. Its text is shown as the system posted it. | The code is in neither the QS/VS catalogue nor the QS/400 table this console was built with: a system of a later release may post codes this console does not know. | See the system's own documentation for the code, or update the console. |
| `QMC1202` | 30 | &1 is not a date in this console's format. | A date filter was typed in another format than the one on Language, locale and session. | Type the date as the console shows dates, or leave the field empty. |
| `QMC2201` | 20 | Power on of &1 is not in this release: a powered-off machine does not answer QSI, and this console has no way to its BMC. | A managed system answers QSI only while it runs; powering one on takes its BMC, which this console is not given. | Power the machine on at its BMC or at the machine. The console reconnects when it answers. |
| `QMC2202` | 0 | Power off of &1 started. Its partitions are shut down first unless Immediate was chosen. | The managed system accepted the power off; its state and codes follow on its pages. | None. |
| `QMC2203` | 0 | Restart of &1 started. The console reconnects when it answers again. | The managed system accepted the restart; it is shown as not connected until it answers. | None. |
| `QMC2210` | 30 | &1 request for &2 not performed. The managed system's own messages follow. | The managed system refused the request. | See the system's messages below this one, act on them, and try again. |
| `QMC1007` | 0 | Description of managed system &1 changed. | The console keeps its own note about the system; nothing on the system is changed. | None. |
| `QMC1008` | 30 | The name of managed system &1 is the system's own and is changed at the system. | QSI does not change a managed system's name; the console shows the name the system gives. | Change the name at the managed system; the console shows the new name once it is paired again with Reset connection. |
| `QMC2220` | 20 | The IPL side, keylock and boot mode of &1 are set at the system. | A managed system reports its slot and keylock over QSI and takes no change of them: its IPL side moves only when a code update is activated, its keylock and boot mode at its Service Tools. | Set them at the managed system's Service Tools. |
| `QMC2221` | 0 | Power-on settings of &1 changed: the partitions to start and the default power off. | The managed system took the settings; they apply at its next power on and power off. | None. |
| `QMC2401` | 0 | Image &1 uploaded to &2's media library. | The host took the whole image and checked it. | None. |
| `QMC2402` | 30 | Image &1 was not uploaded to &2. The messages that follow say why. | The upload was refused or ended before the host had the whole image; the host keeps nothing of it. | Act on the messages and upload the image again. |
| `QMC2403` | 30 | The upload was not taken: &1. | The browser's form did not arrive as the console's page sends it, or it was cut. | Upload the image again from Virtual Optical, with the page's own form. |
| `QMC2404` | 30 | Image &1 is larger than the &2 GB an upload may be. | The console refuses an upload past its limit before it reads it. | Copy the image to the host another way, or use a smaller one. |
| `QMC2405` | 20 | The console does not fetch an image from an address. | The console reaches only its managed systems and its product stream; it fetches nothing from an address typed into a page. | Download the image to your workstation and upload it. |
| `QMC2406` | 0 | Image &1 loaded for partition &2. | The host loaded the image into the partition's virtual optical device. | None. |
| `QMC2407` | 0 | Image &1 unloaded from partition &2. | The host unloaded the image from the partition's device. | None. |
| `QMC2408` | 0 | Image &1 deleted from &2's media library. | The host deleted the image. | None. |
| `QMC2410` | 0 | Virtual disk &1 created in pool &2. | The host made the virtual disk. | None. |
| `QMC2411` | 0 | Virtual disk &1 assigned to partition &2 in its default profile. | The disk is a LUN on a client adapter of the partition's default profile; a running partition has it at its next activation. | None. |
| `QMC2412` | 30 | Virtual disk &1 was made, but not assigned to partition &2. The messages that follow say why. | The disk was made; giving it to the partition's profile was refused. The disk stays, unassigned. | Act on the messages, then assign the disk on the partition's Virtual adapters page. |
| `QMC2413` | 0 | Virtual disk &1 is now &2 GB. | The host grew the disk. | None. |
| `QMC2414` | 0 | Virtual disk &1 deleted from &2. | The host deleted the disk and everything on it. | None. |
| `QMC2415` | 0 | Virtual disk &1 taken off the profiles of &2's partitions. | The disk is no longer a LUN of any profile that named it; it stays in its pool. | None. |
| `QMC2420` | 0 | Storage pool &1 created on &2. | The host made the pool from the disks chosen. | None. |
| `QMC2421` | 0 | Storage pool &1 on &2 changed. | The host took the change. | None. |
| `QMC2425` | 0 | Virtual switch &1 created on &2. | The host made the switch and its uplink. | None. |
| `QMC2426` | 0 | Virtual switch &1 on &2 changed. | The host took the change. | None. |
| `QMC2427` | 0 | Virtual switch &1 deleted from &2. | The host deleted the switch. | None. |
| `QMC2430` | 0 | Tape volume &1 created on &2. | The host made the volume in its media library. | None. |
| `QMC2431` | 0 | Tape volume &1 on &2 changed. | The host took the change. | None. |
| `QMC2432` | 0 | Tape volume &1 deleted from &2. | The host deleted the volume. | None. |
| `QMC2433` | 0 | Tape volume &1 given to &2's tape device. | The volume is the device's medium in the partition's default profile. | None. |
| `QMC2434` | 0 | Tape device &1 taken off partition &2's default profile. | The device is no longer in the profile; its volume stays. | None. |
| `QMC2435` | 20 | A host's own tape drive is given to a partition on the partition's I/O page. | A drive passed through to a partition is released for partitions at the host first; the partition's profile then names it. | Release the drive at the host's Service Tools, then add it on the partition's I/O page. |
| `QMC2440` | 0 | Shared processor pool &1 on &2 made or changed. | The host took the pool. | None. |
| `QMC2441` | 0 | Shared processor pool &1 deleted from &2. | The host deleted the pool. | None. |
| `QMC1006` | 30 | Select a managed system for &1. | The action acts on the systems selected in the list, and none was. | Select the system's box in the list, then choose the action again. |
| `QMC1005` | 0 | Managed system &1 added. The console keeps its session open and shows its state. | The managed system was paired and is in the Servers list. | None. |
| `QMC1120` | 30 | The managed-systems service did not answer: &1. | The console's service that keeps the managed systems' sessions is not running, or is starting. | Wait a moment and show the page again. If it goes on, see the reference code history of the console. |
| `QMC3003` | 30 | &1 terminal windows are open, the most at once. | The console shares each partition's console among the windows that have it open, up to a limit across every system. | Close a terminal window that is no longer needed, then open this one again. |
| `QMC1001` | 0 | Connection to managed system &1 established. Service processor version &2. | The console paired with the managed system and opened its session; the version is the release the system says it runs. | None. |
| `QMC1002` | 0 | Managed system &1 removed. | The console forgot the system and its pinned certificate, and closed its session. | To manage it again, add it with a new pairing password from the system. |
| `QMC1003` | 0 | Managed system &1 paired again. | The console paired with the system with a new password and pinned the certificate it showed. | None. |
| `QMC1004` | 0 | Connection to managed system &1 tested: &2. | Each step of a new connection to the system was timed. | None. |
| `QMC1101` | 30 | Managed system at &1 did not answer: &2. | The console could not open a connection to the address and port, or the connection was closed before QSI began. | Check the address and the port, that the system is powered on and has its QSI service, and that the console's network reaches it. |
| `QMC1102` | 30 | Managed system at &1 turned the console away: no pairing password is out there, or it expired, or its attempts were spent. | A managed system lets a console it has not paired in only while a pairing password made there is out. | At the managed system, make a new pairing password and type it here within its time. |
| `QMC1103` | 30 | Managed system &1 is already managed by this console. | A system of that name or address is in the list. | Use Reset connection to pair it again. |
| `QMC1104` | 30 | Managed system at &1 does not answer QSI: &2. | Only a system that answers QSI can be managed: a QS/VS host, or a standalone QS/400 with its QSI server started. | Check the address and port, and that the system's QSI server is started (on a standalone QS/400, the *QSI server). |
| `QMC1105` | 30 | Managed system &1 answered with a certificate other than the one pinned when it was paired. It is not trusted. | The system's certificate is not the one the console pinned: it was installed again, or something else answers at its address. | Find out why at the system. When it is the system, use Reset connection with a new pairing password. |
| `QMC1106` | 30 | Managed system &1 is not connected: &2. | The console has no session with the system now; it tries again by itself. | See the reason, and Connectivity for when it last answered. |
| `QMC1107` | 30 | This console manages &1 systems, the most it may. | The limit of managed systems was reached. | Remove a system it no longer needs to manage. |
| `QMC1108` | 30 | No managed system &1. | The system is not in the list, or was removed. | Choose a system from the Servers list. |
| `QMC1109` | 20 | Managed system &1 says it is serial number &2, not &3 as when it was paired. | The system answers with the pinned certificate but another serial number. | Find out at the system why its serial number changed; use Reset connection when it is right. |
| `QMC1110` | 30 | Managed system &1 did not answer in time. | The system took longer than the console waits. | Try again; see Connectivity for the system's connection. |
| `QMC1111` | 30 | Pairing with the managed system at &1 did not confirm: the password is not the one it showed. | The system's confirmation did not hold for the password typed; the system counted the attempt. | Type the password the system shows again, or make a new one there. |
| `QMC1112` | 30 | The console is setting up as many connections as it may at once. Try again shortly. | Too many connections were being set up at the same time. | Wait a moment and try again. |
| `QMC1113` | 30 | The system's pairing password is required. | The form was sent without the pairing password shown at the managed system. | Type the password the managed system shows. |
| `QMC1114` | 30 | Managed system &1 does not offer &2. | The system's QSI service, by what its hello said, does not answer this request: it is of an older release, or of a kind that has no such function. | Do it at the managed system, or update it to a release that offers it. |
| `QMC1115` | 30 | The console does not send this request to managed system &1. | The managed-systems service carries only the requests the console's pages send. | Use the page that does it. |
| `QMC9001` | 30 | User ID or password not correct. | The user ID is not a user of this console, or the password is not its password. | Type the user ID and the password again. After five wrong passwords in a row the user ID is disabled for five minutes. |
| `QMC9002` | 30 | User ID &1 is disabled for &2 more seconds. | Five wrong passwords in a row were typed for the user ID. | Wait until the time has passed, or ask an Administrator to set a new password, which enables it at once. |
| `QMC9003` | 30 | User ID &1 is disabled. | An Administrator disabled the user ID. | Ask an Administrator of this console to enable it. |
| `QMC9004` | 30 | Too many sign-on attempts from this address. Try again in &1 seconds. | More sign-on attempts came from this address in the last minute than the console takes. | Wait the time given and sign on again. |
| `QMC9005` | 0 | You are signed off. | You signed off, or your session ended. | Sign on to go on. |
| `QMC9006` | 10 | Your session ended. Sign on again. | The session was idle longer than the idle timeout, reached the maximum session length, or its user ID was disabled or removed. | Sign on again. The timeouts are on Language, locale and session. |
| `QMC9010` | 30 | User &1 is not authorized to &2. | The user's role does not have the task the page needs. Roles and their tasks are on Users and roles. | Ask an Administrator of this console for a role with the task. |
| `QMC9011` | 30 | The form has expired. Nothing was changed. | The form was not sent with the current session's token: it was opened in an earlier session, or did not come from this console. | Open the page again and send the form from it. |
| `QMC9020` | 20 | There is no page &1. | The address does not name a page of this console. | Go to the page from the navigation tree. |
| `QMC9030` | 0 | &1 arrives in a later release of this console. | This function of the console is not available yet. | Use the functions under Console Management. |
| `QMC9031` | 0 | No managed system has been added to this console. | A managed system is added with the pairing password its operator makes on it. | Choose Add managed system on the Servers or Connectivity page. |
| `QMC8001` | 0 | User &1 created with role &2. | An Administrator created the user. | None. |
| `QMC8002` | 0 | User &1 changed. | An Administrator changed the user's properties. | None. |
| `QMC8003` | 0 | Password of user &1 changed. | The user's password was set. | None. |
| `QMC8004` | 0 | User &1 deleted. | An Administrator deleted the user. | None. |
| `QMC8005` | 0 | User &1 enabled. | An Administrator enabled the user. | None. |
| `QMC8006` | 0 | User &1 disabled. Its sessions ended. | An Administrator disabled the user; any session of the user ended. | None. |
| `QMC8007` | 0 | Roles changed. They apply at each user's next sign-on. | An Administrator changed which roles have which tasks. | None. |
| `QMC8010` | 30 | The last enabled Administrator cannot be removed, disabled or given another role. | The console must keep one enabled Administrator. | Make or enable another Administrator first. |
| `QMC8011` | 30 | &1 | A value on the form is not valid. | Correct the value and send the form again. |
| `QMC8100` | 10 | Network change on trial: it is taken back in &1 seconds unless it is kept. | The change was applied on trial. | A change of the address the browser came in on is kept when you sign on at the new address within the time; any other change when its interfaces work. |
| `QMC8101` | 0 | Network change kept. | The console was reached within the trial, and the change was written. | None. |
| `QMC8102` | 20 | Network change taken back: &1 | The console was not reached within the trial, or the change did not work. | Check the values and apply them again. |
| `QMC8103` | 0 | Network settings changed. | The change does not touch the address the browser came in on, and was applied. | None. |
| `QMC8200` | 0 | Date and time settings changed. | The time servers or the display offset were changed. | None. |
| `QMC8201` | 0 | Clock set to &1. | The clock was set by hand. | None. |
| `QMC8300` | 0 | A new self-signed certificate is in use: SHA-256 &1. | An Administrator made a new self-signed certificate. New connections are served with it. | Accept the new certificate in the browser; compare its fingerprint with the one shown. |
| `QMC8301` | 0 | The imported certificate is in use: SHA-256 &1. | An Administrator imported a certificate and its key. New connections are served with it. | None. |
| `QMC8302` | 30 | The certificate was not imported: &1 | The file is not a certificate with its key, or the key does not belong to it. | Import the certificate chain and its private key together, as PEM or as PKCS #12 with its password. |
| `QMC8303` | 0 | Certificate authority &1 is trusted. | An Administrator imported a certificate authority's certificate without a key: it is added to the trusted authorities. | None. |
| `QMC8304` | 0 | Certificate authority &1 is no longer trusted. | An Administrator removed a certificate authority from the trusted authorities. | None. |
| `QMC8305` | 0 | Choose the file to import under Import certificate. | Import... asks for a file: the form under the trusted authorities takes it. | Choose a PEM file with the chain and its key, or a PKCS #12 file with its password. A certificate authority's certificate alone is added to the trusted authorities. |
| `QMC8400` | 0 | Backup made: &1. | A backup of the console was made and downloaded. | Keep the file and its passphrase apart. |
| `QMC8401` | 10 | The backup of &1 is restored when the console restarts. | The backup was checked and is waiting for the restart. | Restart the console from Shut down or restart. |
| `QMC8402` | 30 | The passphrase is not this backup's, or the file is damaged. | The backup could not be opened with the passphrase given. | Type the passphrase the backup was made with. |
| `QMC8403` | 30 | The file is not a backup of a QS/MC console. | The file is damaged or of another kind. | Choose the backup file the console made. |
| `QMC8404` | 30 | The backup is of release &1, newer than this console's &2. | A backup can be restored on its release or a later one. | Update the console to the backup's release first. |
| `QMC8405` | 20 | Copying a backup to a remote host is not in this release. Download the backup instead. | A backup holds the console's identity and users; in this release it leaves the console only through the browser. | Choose Download as file. |
| `QMC8406` | 30 | The passphrase must be 12 to 128 characters. | The passphrase does not follow the rules: a backup holds the console's identity and users. | Type a passphrase of 12 to 128 characters, and keep it apart from the file. |
| `QMC8500` | 0 | The console is powering off. | An Administrator asked the console to power off. | Start it at the machine. |
| `QMC8501` | 0 | The console is restarting. | An Administrator asked the console to restart. | Sign on again when it is back. |
| `QMC8502` | 0 | Message from &1: &2 | A user shutting down or restarting the console left a message for everyone signed on. | None. |
| `QMC8600` | 0 | Settings changed. | The settings were saved. | None. |
| `QMC8700` | 20 | &1 is not in this release. | The function is shown by the design but not provided by this release of the console. | See the console's documentation for what this release does. |
