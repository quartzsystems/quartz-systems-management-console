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
| `QMC9031` | 0 | No managed system has been added to this console. | Managed systems are added and reached over QSI, which arrives in a later release. | Use the functions under Console Management. |
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
