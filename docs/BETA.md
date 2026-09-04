# Wind — beta tester guide

Thank you for testing Wind. This guide covers installation, the two
warnings you may run into (they are expected, and honestly explained),
and how to give feedback.

Wind is an email client for Windows and macOS: fast, sober, local.
Your messages stay on your machine; the application only contacts
your email providers and the update page. No network telemetry.

## 1. Install

1. Open the releases page:
   <https://github.com/smonchamps/wind/releases/latest>
2. Download the file that matches your machine:
   - `Wind_<version>_x64-setup.exe` — Intel/AMD PC (the most common
     case);
   - `Wind_<version>_arm64-setup.exe` — ARM PC (Surface Pro X,
     Snapdragon-based Copilot+ PCs…);
   - `Wind_<version>_x64.dmg` — Intel Mac (Apple Silicon Macs can run
     it through Rosetta; a native build will come with demand).
   If in doubt on a PC: Windows Settings > System > About, "System
   type" line. On a Mac: Apple menu > About This Mac.
3. Windows: run the installer and follow it. Mac: open the dmg and
   drag Wind to Applications.

### If Windows shows "Windows protected your PC"

This is SmartScreen: Wind isn't signed by a commercial certificate yet
(publisher validation is under way — it's currently closed outside
the USA/Canada, and we're waiting for it to open). Click "More info"
then "Run anyway".

### If the installation is refused with no way around it

On some recent PCs, **Smart App Control** (Windows Security Settings >
App & browser control) blocks unsigned programs **without offering a
way to override it** — and its verdict can vary from one Wind version
to another. This is known limitation #1 of this beta. If this happens
to you: **tell us** (see §5) — it's valuable feedback, not a mistake
on your part. We will never ask you to disable Smart App Control (it
can't be turned back on once disabled, and it's a real protection).

### On a Mac: "Wind" can't be opened

macOS blocks apps that aren't notarized by Apple — Wind isn't yet
(same reason as the Windows warnings above: certification is pending,
the beta comes first). Once, at first launch:

1. Double-click Wind; macOS refuses. Open **System Settings >
   Privacy & Security**, scroll down to *"Wind" was blocked*, click
   **Open Anyway**, then launch Wind again.
2. On macOS 14 (Sonoma) or older, a shortcut works instead:
   right-click Wind in Applications, choose "Open", then **Open** in
   the dialog. (macOS 15 removed this shortcut — use step 1.)

Updates installed by Wind itself don't need this gesture again.

## 2. Connect your mailbox

On first launch, Wind guides you through five steps: account, layout,
theme, a word about the beta (and the "Feedback" button in the
header), summary.

### Gmail: the "Google hasn't verified this app" screen

Google's verification of Wind is a lengthy audit (several months),
currently under way. Until it's done, Google shows a warning screen
when you connect a Gmail account. To continue: "Advanced" then "Go to
Wind (unsafe)". What Wind does with this access: read and send YOUR
emails from YOUR machine, nothing else — no third-party server ever
sees your credentials or your messages; the access token stays
encrypted on your machine.

Outlook/Hotmail and standard IMAP accounts connect without this
screen.

## 3. The "Organized" mode — what we'd like you to try

At the top of the window, to the right of the search box, an
**"Organized"** toggle. This is the novelty of this beta, and the
point we're most interested in your feedback on.

Turned on, it opens three destinations instead of one, and a place
where you decide:

- **Inbox** — what's written to you personally, and nothing else;
- **Feed** — what you subscribed to: newsletters and informational
  mail, in cards you scroll through;
- **Paper trail** — sends that are meant to be checked rather than
  read (receipts, alerts, confirmations), grouped by sender;
- **Screener** — senders writing to you for the first time. They are
  never told about your decision.

This isn't an automatic sort: **you're the one who sorts**, one
sender at a time, once — Wind then applies your decision to
everything they send afterward. Three things to know before you try
it:

1. **The Screener's "No" acts at your provider.** By default, messages
   arriving from that sender AFTERWARD go **to your mailbox's trash**
   (never a permanent deletion; what already arrived is untouched).
   You can choose a different rule at the moment you decide (junk,
   archiving, or "Screened out without moving", which touches
   nothing), and change the default in Settings > Screener. "Yes" and
   the three destinations, on the other hand, move nothing: they are
   views within Wind, your Gmail or Outlook folders stay untouched.
2. **The Screener only looks at new arrivals.** It doesn't judge your
   mailbox's past: only new senders, from the moment you turn the mode
   on, go through it.
3. **Until you've decided**, pending mail stays visible in your
   thread, marked "Awaiting the Screener" — nothing disappears
   without your decision.

The toggle can be switched back at any time, and your decisions are
kept (Settings > Screener lists them, and lets you change them). Tell
us what you missed, what was sorted wrong, and whether you kept the
mode on — that last point is what teaches us the most.

## 4. Updates

Automatic and signed: Wind checks on launch, installs with your
consent, restarts. You can check manually in Settings > About. If an
update fails (Smart App Control, again), Wind tells you and lets you
retry — please report it.

## 5. Giving feedback

**Click the "Feedback" button, at the top right of the window**: write
your message, it's sent by email from your account, along with Wind's
version. (If Wind itself is blocked — installation refused, for
example — write directly to <feedback-wind@fcts.io>.)

Everything counts: a bug, slowness, unclear text, a gesture you miss,
a habit from your current client that Wind breaks. Every piece of
feedback is read and acted on.

The most useful feedback fits in three lines:

1. **What you were doing** (the gesture, the screen).
2. **What you expected.**
3. **What happened** (with the time, if it's about slowness).

The installed version is shown in Settings > About — mention it.

## 6. What the beta isn't yet

- No commercial signature on the installer (the warnings of §1 —
  pending the opening of publisher validation).
- Windows and macOS (Intel-native) only, no web or mobile version.
- Fully catching up a very large mailbox (hundreds of thousands of
  messages) spreads over the first hours of use — search gets deeper
  as it goes.
