# Local build startup correction — 2.1.2

An unpublished local build deliberately omits SOURCE_REVISION, preserving an
honest distinction from its GitHub base commit. The checker incorrectly treated
this missing comparison metadata as a failed check and blocked every Start with
the GitHub failure dialog even after successful network responses.

After checking for a verified newer Android release, unknown source metadata now
returns a successful, nonblocking result identifying the locally built version.
It does not claim that the installed source matches GitHub. A verified newer APK
still produces an update prompt, and actual network/invalid-response failures
still offer Retry or Start installed. Every Start still performs fresh requests.

Verified November 16, 2026: five JVM tests, 19 updater contract checks on the
Galaxy S9, in-place upgrade, real successful GitHub check with failed=false and
news=false, and both public game routes matching the running phone runtime.
The desktop remains stopped. Screenshot:
`screenshots/go-host-2.1.2-automatic-start.png` (relative to android-host/docs).
