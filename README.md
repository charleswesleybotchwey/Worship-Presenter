# Worship Presenter

Church presentation software for Windows: songs, scriptures, media and announcements on
the projector, the stream and the stage, in English and Ghanaian languages.
Works like EasyWorship, with extras such as automatic scripture recognition.

## What it does

- **Moving from EasyWorship**: its songs, Bibles and themes import (Settings → Import), and
  File → Open EasyWorship schedule opens a saved EasyWorship 7 schedule (.ewsx).
- **Songs** with two languages on one slide (e.g. English and Fante), chords for the band,
  and quick hymn numbers ("555", "MHB 1"). Find a song's words online by title and artist
  (Songs → right-click → Find lyrics online): Hymnary.org for hymns, LRCLIB for others.
- **Scriptures** from any Bible you add, including free downloads (Scriptures → More
  available…). English references work in every Bible.
- **Scripture recognition**: listens to the preacher and suggests the verse being
  announced or read. Nothing goes on screen until you choose.
- **Web pages** in the service (🌐 in the Schedule): shown over the main screen when live.
- **Media, PowerPoint and PDF**, announcements, timers, alerts and background music. Free
  pictures and videos from Pixabay and Unsplash (Media → Online; each needs a free key).
- **Several outputs**: projector, lower third for the livestream, stage display, and NDI
  for OBS or vMix. As in FreeShow, NDI sends the real output page: moving video, text
  animations and transitions exactly as on the screens (the lower third keyed to transparency).
- **Themes** with a gallery, brand colours, and import from EasyWorship and ProPresenter.
- **Phones**: a remote control for the media desk, and the words on the congregation's
  phones.
- **Recording and going live** on YouTube, Facebook or any RTMP address.
- **Sync** between church computers through a shared folder.
- **Printing** of song sheets, chord charts and the service order.

## Installing

1. Open the repository's **Releases** page and download the latest installer:
   - `…-setup.exe`: the usual installer.
   - `…-setup-offline.exe`: for a computer without internet.
2. Run it. If Windows says "Windows protected your PC", choose **More info → Run anyway**.
   The installer is not code-signed yet.

You need:

- Windows 10 or 11 (64-bit).
- For Whisper scripture recognition, a processor from 2013 or later.
- For NDI, the free NDI Runtime (Settings → Outputs → NDI).

Internet is only needed for downloads, Deepgram, streaming and updates.

## First steps

1. **Settings → Outputs**: choose the projector's monitor (or NDI Stream).
2. **Settings → Import**: bring in your songs and Bibles (EasyWorship, Bible Show,
   OpenLyrics, OpenSong, ProPresenter, ChordPro and others).
3. Build the service: drag songs, scriptures and media into the **Schedule**.
4. Click an item to see it in **Preview**, then press **F5** or double-click to go live.

## Keys

| Key | Action |
| --- | --- |
| F5 | Go live with what is in Preview |
| Space, Enter, → | Next slide |
| ← | Previous slide |
| Page Down / Page Up | Next / previous item |
| 1–9 | Jump to verse or section |
| Esc / L / C | Blank / Logo / Clear the words (press again to bring them back) |
| Ctrl+K | Search for any command |
| Ctrl+P | Print the service order |

Keys can be changed in **Settings → Keyboard**.

## Scripture recognition

Choose the engine in **Settings → Scripture recognition**. Open the **Transcript** panel
(beside Listen) to see the words, the microphone level and the verses detected.

| Engine | Internet | Notes |
| --- | --- | --- |
| Whisper | No (one download) | Recommended. Accurate, works offline. |
| Deepgram | Yes | Very accurate. Needs a Deepgram account (paid per minute); Whisper takes over if the internet drops. |
| Vosk | No (one download) | Light, less accurate. |
| Windows Speech | No | Built into Windows, least accurate. |

For the best results, feed the computer from the mixing desk through a USB audio
interface rather than a laptop microphone.

## Updates

The app checks for a new version once a day and offers **Install and restart**. It never
installs while something is live. You can also use **Settings → About → Check for updates**.

## Before the service

The dots beside Blank / Logo / Clear show every output at a glance (green live, blue
blank, amber held, red off or not answering). Click them for the Outputs panel and
**Check system**: screens, NDI, disk space, backup, Bibles, scripture recognition and
recording, each problem with a Fix button.

## When something goes wrong

Go to **Settings → About → Copy a diagnostic report**, and send it with a description of
what happened. The app keeps a log of errors; it records no song words or notes.

---

## For maintainers

**Builds.** Every push builds the installers on GitHub (**Actions** → "Windows installer"
→ Artifacts). Test that build before releasing it.

**Releasing.** Actions → **Windows installer** → **Run workflow** → type the new version
(e.g. `2.2`) → Run. That run builds, signs and publishes the release (pushing a tag such
as `v2.2` does the same). Use a higher number each time (`2.2.1` for fixes, `2.3` for new
features). Church computers then update themselves. Ordinary pushes only make test builds
(Artifacts) and skip the release step.

**Update signing (one time).** Run `npx tauri signer generate -w worship-presenter.key`.
Then, in GitHub → Settings → Secrets and variables → Actions, add:

- Secret `TAURI_SIGNING_PRIVATE_KEY`: the key file's contents.
- Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: its password.
- Variable `TAURI_UPDATER_PUBKEY`: the contents of the `.pub` file.

Keep the key and password safe: anyone with them could publish updates.

**Where updates come from.** Computers download updates without logging in to GitHub, so
the releases must be public. Either make this repository public, or keep it private and
publish to a separate public repository:

1. Create a public repository, e.g. `Worship-Presenter-Releases`, with a README so it has
   a `main` branch.
2. Create a fine-grained token with "Contents: read and write" on that repository only.
3. Here, under Settings → Secrets and variables → Actions, add the secret
   `RELEASES_TOKEN` (the token) and the variable `RELEASES_REPO`
   (e.g. `charleswesleybotchwey/Worship-Presenter-Releases`).

**Building locally** (Node 22, Rust, and LLVM/CMake for Whisper): `npm run setup`, then
`npm run dev`. Tests: `cargo test -p wp-core -p wp-remote -p wp-audio -p wp-render`, and
for the interface `npm --prefix ui test` (unit) and `npm --prefix ui run e2e` (operator
workflows in a browser; run `npm --prefix ui run build` first). Every push runs them all.
A GitHub Codespace has everything installed; `npm run preview` shows the interface in a
browser.

**More detail**: the design brief, the security audit and the performance audit are in `docs/`.

**Third-party parts**: Noto Sans fonts (SIL Open Font License). Downloaded by the app when
first needed: FFmpeg (recording), the Whisper and Vosk speech models, and Bibles from
eBible.org.
