# Worship Presenter

Desktop church presentation app. Tauri 2 shell, Rust core, React UI, SQLite library.
Spec: the "Worship Presenter — Product Specification" doc.

## Layout

```text
crates/wp-core/   Rust core, no UI: schema, importers, Access reader, slide splitter, scripture parser, live state
  migrations/     SQL schema (spec section 5)
  tests/          real-file import test (needs your EasyWorship and Bible Show files)
src-tauri/        Tauri shell: commands, live-state broadcast, heartbeat, output windows
ui/               React + TypeScript control window and output windows
```

## Status

Built and tested in the dev sandbox (Linux):

- wp-core, wp-remote and wp-audio: 84 tests pass (81 unit, 3 against your real files).
  - EasyWorship songs: 4,531 of 4,531 imported, 529 look-alike characters fixed and logged.
  - EasyWorship Bibles (.ewb): all 17 imported. Bible Show (.bib): NKK2011, TK2012, YGZP2020 imported.
  - Our own Access (Jet 4) reader matches mdbtools row for row on every table of all three .bib files.
- Tauri shell: compiles with no errors or warnings (checked on Linux with Rust 1.91).
- UI: builds clean; checked in a browser with sample data.

Not yet done: running the desktop app on Windows. Use the cloud build below.

## Getting the Windows installer (.exe)

GitHub builds it on its own Windows machines; nothing needs installing on your PC.

1. Upload this folder to your GitHub repository (GitHub Desktop: Commit, then Push).
2. Open the repository's **Actions** tab. The "Windows installer" run takes 20–40 minutes.
3. When it has a green tick, open it and download **Worship-Presenter-…-Windows** under
   Artifacts. Unzip it to find:
   - `Worship Presenter_0.1.0_x64-setup.exe`: the usual installer (about 10 MB;
     downloads Microsoft WebView2 only if the PC lacks it; Windows 11 has it).
   - `Worship Presenter_0.1.0_x64-setup-offline.exe`: includes WebView2, for church PCs
     without internet.
4. Run the installer. It asks whether to install for you only (no administrator needed)
   or for everyone on the PC, and adds Worship Presenter to the Start menu.

To publish a version: on GitHub, Releases → "Create a new release", tag `v0.1.0` (or the
next number), Publish. The run builds that version and attaches both installers to the
release, ready to share as a link.

The installers are not code-signed yet, so Windows may say "Windows protected your PC":
choose More info, then Run anyway. Signing needs a code-signing certificate (about
US$200–400 a year); with one, the warning goes away. Tell me when you have one.

If a run goes red, open the failed step and send me the red error text.

## Building on your own PC (optional)

Only needed if you want live-reload development. In PowerShell, from this folder:

```text
powershell -ExecutionPolicy Bypass -File setup-windows.ps1
```

That installs Visual Studio C++ Build Tools, Rust, Node.js and WebView2 with winget.
Then open a new PowerShell here and run:

```text
rustup default stable-msvc
npm run setup
npm run dev
```

No separate Tauri CLI install is needed; it comes with `npm run setup`. Rust 1.90 or
newer is required (some Tauri dependencies need it).

## GitHub Codespaces

A Codespace is a Linux server without a screen, so the desktop window cannot open there
(`npm run dev` / `tauri dev` will not work). Use it for:

- the browser preview: `npm run preview`, then open the forwarded port 5173
- the core tests: `npm test` (runs `cargo test -p wp-core`)
- editing code; the Windows installer still comes from the GitHub Actions build

The .devcontainer folder installs Rust, Node and the Linux libraries automatically when a
new Codespace is created. For an existing Codespace: Ctrl+Shift+P, "Codespaces: Rebuild
Container".

## Trying the UI without Rust

```text
cd ui
npm install
npm run dev
```

Open http://localhost:5173/#/control. It runs on built-in sample data (MHB 1 bilingual,
John 3:16 in NKJV, Twi and Ga). Open http://localhost:5173/#/output/main and
http://localhost:5173/#/output/alternate in other tabs to see the outputs follow along.

## Running the real-file tests

```text
set WP_SAMPLES=C:\path\to\FILES        (Songs.db, SongWords.db and the .ewb files)
set WP_BIB=C:\path\to\bibleshow        (the .bib files)
cargo test -p wp-core --release --test real_files -- --nocapture
```

## Keys

Double-click a Preview slide or the Preview picture: go live with that slide (same checks as Go live). Double-click in the Library or Schedule only previews.

| Key | Action |
| --- | --- |
| F5 | Go live with the slide selected in Preview (works from anywhere) |
| Space, →, ↓ | Next slide (not while typing in a field) |
| ←, ↑ | Previous slide |
| Page Down / Page Up | Next / previous service item into Preview |
| 1–9 | Jump to verse or section n |
| Esc | Blank (when no field or dialog is open) |
| L / C | Logo / Clear text |
| Ctrl+Shift+B, Ctrl+Shift+L | Blank / Logo from anywhere, even while typing |
| Ctrl+F / Ctrl+G | Song search / scripture reference |
| Ctrl+Z | Undo go live |

Enter never puts anything on screen; it only acts on the focused field or button.

## Bible Show files

Bible Show .bib files are Microsoft Access (Jet 4) databases. They are read by our own
reader (crates/wp-core/src/jet.rs), so no Access drivers or helper programs are needed.

## Done in Phase 1 so far

- Service orders: saved by date, templates you reuse every week, auto-save on every change,
  the last open order reopens after a restart or power cut. Song use is logged for CCLI reports.
- Import screen (top bar, Import): EasyWorship songs folder, .ewb and .bib Bibles, with counts
  of what was read, imported, skipped and corrected. Imports run in the background so outputs
  keep drawing.
- Self-healing outputs: every output answers the heartbeat; one that goes silent for 4 seconds
  (crashed, hung, or closed by accident) is rebuilt on the same display. If that display was
  unplugged, it comes back as a window on the control screen.

- Song editor: pencil on any song, or New beside the search box. Lyrics per language with
  [V1] / [C] labels, a second language paired part by part, play order, lines per slide,
  hymn numbers, and buttons for ɛ ɔ Ɛ Ɔ ŋ Ŋ ɣ ʒ. The preview shows the real slides as you type.
  Editing a song that is live updates the projector in place. Ctrl+S saves.
- Search works from a plain keyboard: "hyen" finds "hyɛn", "Nyame ye" finds "Nyame yɛ".
  Libraries imported earlier are re-indexed once, automatically, the next time the app opens.
- Backups (top bar): back up now, restore from the list or from a file (e.g. a USB stick).
  A restore shows what the backup holds, and saves the current library first so it can be
  undone. Automatic backups: weekly and before every import (latest 12 kept).

- Profiles (status bar, bottom left): one library per church, branch or venue, each with its own
  songs, Bibles, service orders and backups. New profiles start empty or as a copy of the current
  one. Switching asks first if something is on screen, then blanks the outputs. Deleting moves the
  profile to profiles\\.deleted so it can be recovered.
- Fixed: the app no longer creates an empty "Untitled service" every time it opens; empty ones
  left by the earlier version are removed at start-up.

## Phase 2 so far: themes

- Theme editor (top bar, Themes). A theme is a background, a slide-change transition and a
  stack of layers: text (lyrics, second language, reference, song title, copyright, clock or
  fixed text), shape, image and ticker. Drag to move, handles to resize, snapping to the centre,
  edges and other layers (Alt places freely), arrow keys nudge, Ctrl+Z undoes, Ctrl+S saves.
- Each layer has its own "coming on" and "going off" motion (fade, slide, wipe, grow) with
  length and delay, so a lower third can slide its band in, wipe an accent line, then fade the text.
- Text shrinks to fit its box; a lyrics box can take more room when a song has no second language.
- Sample content (bilingual hymn, single language, two-version scripture, very long verse) to
  check a theme before using it; Play in / Play out previews the motion.
- One theme per kind is "in use": songs, scripture and the lower third. Saving updates open
  outputs at once. Built-in themes: Classic lyrics, Scripture, Lower third.
- Outputs now animate: going on air plays each layer's motion, going blank plays the
  going-off motion before clearing, and slide changes cross-fade only the text.
- Pictures in themes are stored inside the theme (scaled to at most 1920 px wide), so a theme
  is self-contained.

## Phase 2 so far: media

- Media tab in the library: import pictures, videos and sound (copied into the profile's
  media folder, so backups, profile copies and themes carry them). Thumbnails and video
  length are made automatically. Search and filter by pictures / videos / sound.
- Pictures and videos go live full screen and can sit in the service order. Live videos have
  Play/Pause and Restart; only the main output plays sound; the lower third stays clear.
- Moving backgrounds: a theme's background can be a library video (silent, looping, never
  restarted by slide changes) or a library picture, with darkening. Quickest route: ⋯ on a
  media card, "Background for songs" or "Background for scripture".
- Blank is now true black with no background video; keyed outputs go see-through.
- Thumbnails and Preview show a video background as its still picture, so a long service
  order does not decode dozens of videos at once.
- Windows note: videos must be in a format the Windows webview plays: MP4 (H.264) or WebM.
  Unsupported files import but show "cannot be played"; convert them with HandBrake.

## Phase 2 so far: looks, sharing, stage

- Per-item looks: in Preview, "Look" picks a theme for that song or reading only. It is saved
  with the item in the service order and goes live with it; the next item goes back to the
  theme in use. If the look is later deleted, the item quietly falls back to the theme in use.
- Theme export/import (Themes, Export… / Import a theme…): a .wptheme file holds the theme and
  every library picture and video it uses. Importing copies those into the media library,
  never overwrites a different file of the same name, and reuses identical files.
- Stage display (Outputs, "Stage display"): current words large, next slide dimmed, song or
  reading title, slide count, clock, and a red notice when the screens are blank or showing
  the logo. The Live pane has a "Message to the stage display" box ("Two minutes"), shown in a
  yellow banner on the stage display only, never on the projector.

## Phase 3 so far: timers and alerts

- Timer (top bar): start a saved timer or a new one, counting down from a length ("5:00",
  "1:30:00") or to a time of day ("09:00", "9:30 am"). Pause, resume, +1/−1 minute, stop.
  Show it on the projector in the corner or full screen, and on the stage display. After zero
  it keeps counting up in red ("+0:12") so everyone knows how far over it is. The top-bar
  button shows the time left while it runs.
- Alert (top bar): saved alerts with blanks in braces, e.g. "The owner of car {registration}
  is asked to see an usher". The blank must be filled before it shows. Band or scrolling, top
  or bottom, for 30 s to 2 min or until cleared, on any of projector / stream / stage.
  Time-limited alerts clear themselves even if nobody touches the computer.
- Starter presets: 5-minute countdown, 30-minute sermon, Service starts 09:00; children's room,
  car registration, and a welcome ticker. Add your own with "Save as preset".

## Interface: design brief v1.0 (docs/design/design-brief-v1.0.md)

The operator interface follows the design brief. Done so far (brief steps 1–3):

- Tokens (src/ui/tokens.css): the brief's dark and light palettes, Follow Windows, focus
  ring, sizes, radii, motion, forced-colours rules. Ctrl+K, "Appearance" switches theme.
- Shell: workspace tabs (Service pinned; Themes and the song editor open as tabs), 44 px
  service bar (profile / service on the left, quick scripture, Timer, Alert, Blank, Logo,
  Clear, then Go live as the only accent button), Schedule as the sidebar, Library,
  Preview and Live with draggable splitters (sizes saved per profile and width band),
  24 px status strip with every output's state, backup and clock. 960–1279 px combines
  Schedule and Library; below 960 px one task at a time with a compact Live strip.
- Command palette (Ctrl+K): safe actions only.
- Output safety:
  - "Live · Main" only after the main output acknowledges the revision; otherwise
    Sending…, Main: not confirmed (after 1 s) or Main: disconnected, and the Live
    monitor shows the last confirmed picture with its time.
  - Blank, Logo and Clear latch; Go live and slide changes are refused until Resume
    output, which restores the last confirmed cue.
  - Space, Enter, arrows, Page Up/Down, 1–9, Esc, L, C and Ctrl+Z drive output only when
    the Live surface itself has focus (focus moves there after Go live). In fields Esc
    just leaves the field. F5 never reloads the window and is ignored while typing or in
    a dialog. Held keys do not repeat output commands.
  - A lost display is never moved onto the operator's screen; a display that comes back
    stays black until "Resume Main" (or that output).
  - Recovery after restart puts the last cue in Preview; the screens stay as they are.
  - Closing the window while something is live asks first.

Also from the brief (round 2):

- Settings tab (Ctrl+K, Settings): Appearance (theme, low-spec interface), Keyboard (add or
  remove keys per action with conflict checks; switch off single-letter shortcuts),
  Projection (default lines per slide, show last slide after restart, low-spec projection
  with cuts instead of animations), Outputs (state and Identify), About.
- Song editor: "Save to library" never changes the screens; if the song is live you get
  "Update the screens". "Apply to this service" keeps changed words on that service item
  only ("Edited words" in the schedule). A Pairing view shows English/Fante line pairs per
  part, flags uneven parts and jumps to the text to fix them. Deleting a song or media
  that is on the screens is refused.
- Import: choose → review (recognised, changed on the way, rejected, already here) →
  import → report. Duplicate songs can be left out; a Bible code clash asks for a new code.
- Media: dense list by default (thumbnails optional), "File missing" with Relink.
- Rehearsal mode (service bar switch or Ctrl+K): Go live, Blank and the Live pane drive a
  simulator labelled Rehearsal; nothing reaches the screens.
- Options, outputs: "Identify screens" shows a big number and the monitor's name on every connected monitor for 4 s (as in EasyWorship), so you can pick the right Monitor; "Test pattern" shows colour bars on one open output. Both ask first if something is on the screens.
- Fixture page (#/fixture) and screenshot baselines (tests/ui/baselines.py).

Needs you, on Windows: the Linear reference captures (docs/design/references/manifest.md)
and the sign-off checks in docs/design/baselines/signoff.md.

## Phase 3: live production (complete in the browser; needs Windows testing)

- Announcements (Library, Notices): decks of title-and-text or picture slides, edited in their
  own tab (slides left, canvas centre, properties right). A deck moves on when you press Next,
  or by itself every N seconds (per-slide override), and can loop before the service. The Live
  pane shows "Next slide in N s" with Pause/Resume; nothing advances while Blank/Logo/Clear is
  on. Announcements stay off the stream's lower third.
- Offering pack (Notices, New): Offering and tithes (MoMo, bank details), Birthdays this week,
  Welcome before service (looping). Fill in your own numbers and names, then save.
- Chorus repeat counter (Live pane, songs): choose the section and how many times; each return
  to it counts, with −/+ to correct. The stage display shows it large, and "last time".
- Lining out (Live pane, songs): one line at a time; the stage display shows the next line
  large for the leader. Switching off restores the previous lines per slide.
- Live edits: "Edit words" opens the live song; saving offers "Update the screens".
  "Back to previous item" (or Ctrl+Z on the Live pane) returns to the item before.
- Also from earlier rounds: timers, alerts, stage messages.

## Phase 4: NDI (prototype, needs testing on Windows)

Settings → Outputs → NDI. Needs the free NDI Runtime (install "NDI Tools" from ndi.video);
the app finds it where the installer puts it. Nothing from NDI is bundled yet.

- Test pattern: moving colour bars with a see-through half for 30 s, named
  "Worship Presenter Test". Use it first to confirm OBS or vMix receives NDI at all.
- Lower third over NDI: an off-screen 1920×1080 window draws the lower third on pure
  green; Windows.Graphics.Capture grabs it; the core turns the green into real
  transparency (no green fringe; translucent bands keep their opacity) and sends it at
  25/30/50/60 fps, repeating the last frame between changes. No monitor needed.
- Main over NDI: the main output window as it is (opaque). Open the main output first.
- Live statistics and "Copy prototype results" for the Phase 0 gate (1080p30 with
  alpha, under 100 ms delay, under 25 % CPU on the reference laptop).
- Lower-third themes warn about strong greens, which would key partly.
- Checked here: the NDI structures match the C header; keying a 1080p frame takes about
  3 ms; a lower third rendered by Chromium keys cleanly. Not checked here: the Windows
  capture code compiles only on Windows, so the GitHub Actions build is its first compile.
- If capture cannot hold the targets on the reference laptop, the documented fallback is
  the alternate output on a spare HDMI port keyed by colour in vMix/OBS.

Fixed while testing: auto-fit in the built-in lower third could not shrink the second
language and reference lines (their smallest size was larger than their size), so long
Fante lines were cut at the bottom; the renderer now guards against that in any theme.

## Phase 4: phone remote and browser stage display

Settings → Phone remote → Start the remote. Off until started.

- Phones join at the address shown (or scan the QR code) with the 6-digit PIN, as:
  "Control the screens" (Previous/Next, Blank/Logo/Clear, Resume, tap a slide, send a
  service-order item live after a confirmation), "Stage display" (the pulpit tablet:
  words, next slide, clock, timer, notes, chorus count) or "Watch only".
- Every press goes through the same functions as the desk, so held output and the other
  rules apply; refused presses show the reason on the phone.
- Safety: 5 wrong PINs lock that phone out for a minute; pages from other websites are
  refused; one press per 150 ms; sessions end after 6 hours idle, can be signed out one
  by one, and "End of service" signs everyone out; every press is logged with the device.
- Timers on phones use the computer's clock, so a phone with the wrong time still counts
  down correctly.
- Windows asks once whether to allow Worship Presenter on the network: allow it for
  private networks (the church Wi-Fi).
- Try the pages without the desktop app: `cargo run -p wp-remote --example demo`, then
  open the address it prints (the PIN is printed too).

The remote server is its own crate (crates/wp-remote) and is tested here: joining, roles,
lockout, origin check, rate limit, desk rules and sign-out.

## Phase 4: background music

Service bar → Music (shows the track playing). The playlist comes from sound files in
the media library (MP3, WAV, FLAC, OGG, M4A/AAC).

- Play, pause, previous/next, fade out, volume; tap a track to play it now.
- Crossfade between tracks (0–8 s); start the list again after the last track (optional).
- Rules: when a song or reading goes live, fade the music out (default), play it quietly
  under the words, or keep playing; start the music when announcements go live (default
  on). "Raise music" brings it back up after it was lowered.
- Music plays from the core on its own thread, so a window reload never cuts it; a file
  that cannot be played is skipped and named; the playlist, volume and rules are saved
  with the profile. The player starts on first use, so a computer without speakers can
  still run a service.
- Tested here with a pretend sound card and clock (crates/wp-audio): fade-in, crossfade,
  the three go-live rules, skipping broken files, looping. Real playback needs Windows.

## EasyWorship layout (round 1 of 2)

- Schedule, Preview and Live across the top; drag the dividers (saved per profile).
  Preview and Live show the slides as a text list with section headers (Verse 1,
  Chorus…), or as pictures; under each, a small output monitor with "Slide 2 of 4"
  and ‹ › arrows. Double-click a Preview slide to go live.
- The library across the bottom: Songs, Scriptures, Media, Presentations, Themes.
  - Songs: folder tree (All songs; Collections = your hymnals: MHB, NCAN, GMHB, CAN,
    LALA; My collections you make) and a sortable table (Number, Title, First line,
    Author, Copyright) that scrolls smoothly through every song. Click previews, drag a
    row into the Schedule, right-click to add to a collection or edit.
  - Scriptures: type a book, chapter or verses; the whole chapter appears in a table
    with the typed verses marked. Click a verse to preview it, shift-click for several,
    choose a second version to show with it.
  - Themes: every theme as a picture; "Use for this item", "Use for all", "Edit themes".
- Video and sound in Preview can be played and scrubbed there, muted to start.
- Settings → Projection: "Advance the schedule after Go live". Settings → Library:
  "Rebuild search keys" and "Reset songs to the theme in use".
- Below 1100 px wide the Schedule moves into the library as its first tab.

## EasyWorship Options (round 2)

Click an output in the status strip, or Settings → Outputs.

- For each output (Main, Alternate/lower third, Foldback/stage):
  - Output monitor, remembered by name, so it is found again if Windows renumbers
    the monitors; if it is unplugged the output stays closed, never on your screen.
  - Output position: fill the monitor, or a custom left/top/width/height in pixels.
  - Screen margins in pixels: keeps words off edges the projector cuts, or clear of a TV
    frame. Works on 4:3 projectors too.
  - Alpha channel: see-through behind the words for keying hardware or capture.
  - Themes: this output's own song and scripture themes (an item's Look still wins).
  - Logo and Blank: show the logo / go black / go see-through / keep showing the slide.
  - Show this output when Worship Presenter starts.
  - Open, Identify, Close; Save applies to an open output at once.
- Service intervals (Settings): your service times; every song that goes live is
  recorded with its service. Song usage report by dates, saved as CSV for a spreadsheet
  or CCLI's website (a song shown several times in one service counts once).
- Font fallback list (Settings → Projection): fonts tried in order for letters a theme's
  font lacks; keep Noto Sans in it for ɛ ɔ ŋ and tone marks.

## PowerPoint and PDF (Library → Presentations → Import PowerPoint or PDF…)

PowerPoint (.pptx, .ppt, .pps, .ppsx, .odp), in three ways, best first:
1. PowerPoint on this computer: it exports every slide as a 1920×1080 picture, exactly as
   designed (hidden in the background; gives up after 3 minutes on a stuck file).
2. LibreOffice (free) on this computer: converts to PDF, then the pages are drawn as pictures.
3. Neither: the .pptx is read here; each slide keeps its title and text, and the report
   lists what each slide lost (pictures, charts, table layout, video, background).
   Older .ppt files need PowerPoint or LibreOffice (or save them as .pptx or PDF).

PDF: every page becomes a picture slide, drawn by pdf.js 4.10 inside the app (pdf.js 6 was
tried and needs a newer browser engine than some Windows 10 machines have).

The result is a deck in Presentations, used like announcements: preview, go live, add to
the schedule, auto-advance and loop if wanted. Animations inside PowerPoint are not kept.
Tested here: a sample PowerPoint (tests/make_sample_pptx.py) through LibreOffice and pdf.js
to the projector; the built-in reader on the same file (Akan text kept, hidden slide left
out, the picture reported). PowerPoint itself needs Windows to test.

## Song files from other programs (Ctrl+K, Import → Song files: Files… or Folder…)

One importer reads, telling formats apart by content as well as extension:
- OpenLyrics (.xml; OpenLP and others): titles, authors, copyright, CCLI, song books
  (as hymn numbers), verse order; verses in two languages come in as a bilingual song.
- OpenSong (files often without extension): sections, presentation order; chord lines,
  comments and line markers dropped.
- ProPresenter 6 (.pro6): groups (Verse 1, Chorus) with the words of each slide; CCLI details.
- ProPresenter 7 (.pro): best effort, since its format is not published: every slide's
  words come in (as V1, V2…), the title too; group names and arrangements do not.
- CCLI SongSelect: lyrics .txt (sections, CCLI number, authors, copyright) and older .usr.
- ChordPro (.cho, .chordpro, .crd): chords removed; {soc}/{start_of_chorus} and
  {comment: Verse 2} become sections; a chorus written out twice is kept once.
- Plain text (.txt): verses split by blank lines; "Chorus", "Verse 2" lines understood.

A folder is searched, sub-folders included. The review shows recognised songs, songs with
look-alike characters fixed (Greek ε → ɛ and so on), files that could not be read (with the
reason) and songs already in the library (left out by default). A backup is taken first.
ProPresenter's slide text is RTF; Akan letters written as RTF codes (\u596 → ɔ) come through.

## Scripture recognition (experimental)

Listen in the service bar; Settings → Scripture recognition. Listens to the preacher's
microphone and suggests the references it hears ("John three sixteen", "first
Corinthians thirteen verses four to seven", "Psalm twenty three", "Yohane three sixteen")
in a chip above Live: Preview, Go live (Enter on the button), Esc or × to dismiss, and
the earlier ones in a list. It never puts anything on the screens by itself; Go live
uses the usual checks (held output and so on).
- Vosk (default): light, offline; engine and English model downloaded once (~60 MB).
  "Bible references only" limits it to book names and numbers: much more accurate.
- Windows Speech: offline, built in, nothing to download; less accurate.
- Whisper and cloud services: listed, not available yet.
- English now; Twi and Fante when speech models for them exist. Book names include the
  Twi/Ga names from your Bibles; "a good job" is not Job (a chapter number must follow).
- Tested here: the reference finder on spoken and written forms; the real Vosk engine
  loads and refuses a missing model cleanly. Recognition itself needs a microphone and the
  model on Windows; please try it with a recording of a sermon.

## Fonts

The theme editor lists every font installed on this computer, each shown in its own face,
searchable, marked "ɛ ɔ ✓" when it has the Ghanaian letters (otherwise Noto Sans fills them in).

## Sunday features (11 Nov 2026)

- Bible word search: in Library → Scriptures, type words instead of a reference ("living
  water"); verses containing them appear with the words highlighted. "Quotes" for an exact
  phrase; "Search every Bible" to look beyond the chosen one. A plain keyboard finds Twi
  ("Onyankopon dɔ wiase" finds Yohane 3:16). Each Bible's search index is prepared the
  first time it is searched (under a second).
- Pastor's notes: on the phone remote, join as "Pastor's notes" and send points; each one
  becomes a slide in today's "Sermon notes" deck and waits in a chip above Preview
  (Preview / Go live / dismiss). Never shown without the operator.
- Live sermon subtitles (experimental): Subtitles in the service bar (stream, screens or
  both). Listens to everything the preacher says and shows two lines that fade when he
  pauses. English now; translation to Twi needs a translation model that does not exist
  offline yet.
- Camera on the screens: Camera in the service bar; choose the camera (USB or a capture
  card), see it, then Full screen, Behind the words or Corner. Main screen only; Blank,
  Logo and held outputs still win.
- Musicians' stage view: write chords in the lyrics in brackets before the syllable
  ("[G]Amazing [C]grace"); the congregation's screens, search and slide lists show the
  words only; the stage display (and the stage tablet) shows the chords above the words
  and the song's key (song editor → Key). Chords −/+ in the Live pane transposes them for
  the band; it resets when a new song goes live. ChordPro, OpenSong and OpenLyrics imports
  now keep chords and keys.

## Fixes and options (11 Nov 2026, later)

- Closing the control window works again and ends Worship Presenter cleanly (outputs,
  music, phone remote and the output watchdog stop with it). It used to hang: the window
  was not allowed to close itself after the "Something is on the screens" check.
- Themes → "Show verse and chorus labels" (song and lower-third themes): adds a small
  "Verse 1 / Chorus / Bridge" label above the words. It is an ordinary text layer: drag it
  to move it, click it to change its font, size and colour; untick to remove it.

## Themes from EasyWorship and ProPresenter

Themes tab → "From EasyWorship or ProPresenter…", then choose a file:
- EasyWorship theme export (.ewtx): background colour and picture (its opacity becomes
  dimming), the text box, font, size and colour, the verse label (it becomes a section-label
  layer), copyright line.
- ProPresenter 7 theme export (.proTheme, may hold many themes): every slide design becomes
  a theme of the right kind (songs, scripture, lower third, announcements) with its text
  boxes (position, font, size, colour, alignment), filled boxes and outlines, pictures from
  Assets, and the slide background when ProPresenter draws it. Countdown timers are left out.
The review shows each design as it will look, with its kind; songs, scripture and lower
thirds are ticked by suggestion, announcement designs are not. Hover a design to see what
was not carried over (masks, gradients, custom shapes, scrolling text, animations;
EasyWorship text effects and transitions). Fonts must be installed on this computer; a
missing one is replaced by Noto Sans. Tested with a real EasyWorship theme (BLACK) and a
ProPresenter pack of 17 themes (123 designs).

## NDI: built-in renderer (learned from FreeShow)

FreeShow draws its NDI output with Electron's offscreen rendering (a hidden window
painted into memory with real transparency) and sends it from a background worker.
Tauri's WebView2 has no offscreen rendering, so Worship Presenter does the same job
another way:
- The lower third is drawn by a built-in renderer (crates/wp-render) straight from the
  theme: exact transparency (no green keying), no window, about 23 ms a frame, and only
  when something changes. Same rules as the lower-third output: song and scripture words,
  section labels, alert band and live subtitles; nothing for media, announcements, Blank,
  Logo or held output. Noto Sans is built in (ɛ ɔ ŋ and tone marks); other fonts come
  from the computer. Video backgrounds, tickers and animations are not drawn.
- Window capture stays as a fallback (Settings → Outputs → NDI → Drawing), and is still
  how "Main over NDI" is sent.
- Like FreeShow: 1 frame a second while no receiver is connected, full rate once OBS or
  vMix connects; frames go out in NDI's own UYVA layout (BT.709 video levels, separate
  alpha); always the newest frame; NDI groups; "Get the NDI Runtime" when it is missing.
- Sound over NDI: tick "With the computer's sound" on a running NDI source (Settings →
  Outputs → NDI) to send everything this computer plays (music, video sound) with it.
  Windows records it as loopback from the default sound output; the room still hears it.
- NDI sources as inputs: Camera (service bar) → "Look for NDI sources" lists network
  cameras, other computers and OBS outputs next to the USB cameras; choose one and show it
  full screen, behind the words or in the corner, exactly like a USB camera. Screens fetch
  its newest picture from the app (ndi://, JPEG up to 1280 wide); a source is received
  only while a screen shows it, and stops 10 s after.
Noto Sans: SIL Open Font License (crates/wp-render/fonts/OFL.txt).

## EasyWorship-style operation (11 Nov 2026)

- On opening, a connected projector or TV is found: the first time, the main output goes
  on the first monitor that is not your own screen, is remembered, and shows at every
  start (Options → outputs to change it). With one screen only, nothing opens on it.
- Go live works with no projector too: the slide shows in Live (as in EasyWorship).
- Double-click a schedule item: it goes live. Double-click a Preview slide or the
  Preview picture: it goes live. Double-click in the library: it goes to Preview.
- Arrow keys drive Live from anywhere you are not typing: → or ↓ next slide, ← or ↑
  previous, Page Down/Up next/previous schedule item. Blank, Logo, Clear and the number
  keys still need the Live monitor selected, so a stray letter never changes the screens.
- Preview and Live each keep their own List/Pictures view.

## Feels like a Windows program

Worship Presenter is a Tauri app: the engine is native Rust, and the interface is drawn by
Microsoft WebView2 (the Edge engine built into Windows), like the new Teams and Outlook.
Browser habits are switched off so it behaves like a Windows program:
- No browser right-click menu (text fields keep Cut/Copy/Paste; the app's own menus stay),
  no reload/print/find/view-source/back keys, no Ctrl+wheel, Ctrl +/− or pinch zoom, and
  pictures cannot be dragged out (ui/src/nativeGuard.ts; --disable-pinch for WebView2).
- A Windows menu bar on the control window: File, Edit, Live, Profiles, View, Help, with
  each shortcut shown (the keys are handled once, by the app's keyboard code).
- The title bar follows Dark/Light; on Windows 11 the window gets the Mica backdrop.
- Segoe UI (Windows' own font) for the interface; slides keep their theme fonts.
- The window opens on a dark background and appears once drawn (no white flash; shown
  after 4 s at the latest).

## Drag to the schedule; no stalls (11 Nov 2026)

- Drag songs, scripture verses (or a chosen range), media (list or pictures) and
  presentations into the schedule. Tauri's own file-drop handling was taking all drags
  inside the window on Windows (dragDropEnabled is now off; the app never used it).
- Right-click any library item: Preview, Go live, Add to schedule (plus Edit for songs
  and presentations, and the collection items for songs).
- Responsiveness audit:
  - 123 commands ran on the window's main thread; anything slow among them (backups,
    profile switching, search rebuilds, big library queries) or any wait for the database
    during an import froze the whole window ("Not responding"). All now run off it.
  - Lock order checked across the app for deadlocks now that commands run side by side.
  - The service window redrew completely every 0.4–0.7 s (output status, pulpit notes,
    speech status) even when nothing changed: now 0 redraws while idle.
  - The NDI lower-third renderer re-read every theme 20 times a second: now cached.

## Right-click menus as in EasyWorship

Every library tab has EasyWorship's right-click menu (with submenus, check marks and
underlined access keys), followed by this app's extras (Preview, Go live, Add to schedule):
- Songs: New Song, Edit Song, Delete (backup first), Update items in Schedule (always up
  to date here, so greyed), Sort by (Number, Title, First line, Author, Copyright,
  Languages), Refresh; plus Add to my collection.
- Scriptures: View (translation / reference columns), Sort by (best match / translation,
  for word searches), Refresh, Copy Scripture to Clipboard.
- Media: Import New Video/Image/Audio File, Media Usage, Repeating (Off / On), Aspect
  Ratio (Fit / Fill / Stretch), Edit Properties, Copy to Theme (song / scripture /
  presentation), Use As Logo Background (the Logo screen shows it, looping), Delete,
  Open File Location, View (List / Pictures), Sort by (Title, File name, Date added,
  Size), Refresh. Repeating and Aspect Ratio are kept per item (migration 0006).
- Themes: New / Edit <kind> Theme, Import Theme File (Worship Presenter, EasyWorship or
  ProPresenter files; the last two open the design review), Export Theme File, Set As
  Default, Delete, Update Themes in Schedule (greyed), View, Sort by, Refresh, Duplicate.
  Right-click empty space for New / Import / View / Sort by / Refresh.
- The theme editor shows only the theme being edited (as in EasyWorship): its layers,
  the canvas and its properties. Choosing, importing, exporting, duplicating and deleting
  themes happen in the library's Themes tab.

## Box that grows with the words (as in ProPresenter)

Theme editor → a text layer → Box behind text:
- Whole text: one box around the words that grows and shrinks with them, gliding to the
  new size between slides (Glide to the new size), e.g. a white box with black words.
- Each line: a strip behind every line (news / lower-third look).
Colour (with transparency), side and top/bottom padding, and corner roundness are set per
layer. The padding comes out of the layer, so shrink-to-fit still keeps everything inside.
Works on every screen, Preview/Live, the stage display and the NDI renderer.

## Themes, round two (11 Nov 2026)

- Theme rules (Themes → Rules…): "For songs when the hymnbook is MHB, use …", by hymnbook,
  language, one or two languages, service interval, or Bible version for scripture. First
  match wins; an item's own theme and an output's theme still come first.
- Sections: song themes can make choruses, bridges and pre-choruses italic, bold or another
  colour. Any layer can show on every slide, the first only (hymn number and title) or the
  last only (copyright).
- Live pane → Words A− A+ ↑ ↓ ⟲: the words larger/smaller, higher/lower for the current item
  on every screen and NDI; reset on the next item.
- Brand kit (Settings): four colours and two fonts. Theme colours can link to brand swatches,
  fonts to the brand fonts; Themes → right-click → Re-colour to Brand converts a whole theme.
- Readability in the editor: a warning when words are hard to read (contrast under 3:1)
  with one-click fixes (darken the picture / box behind the words); Safe areas guides.
- Visuals: gradients (backgrounds, shapes, words), frosted blur behind the growing box, and
  text changes: fade, cut, slide up, wipe, zoom, push.
- Motion background sets: more videos that take turns, per song/reading or per service.
- Gallery (Themes → Gallery…): six ready-made themes in the brand colours: Methodist hymn
  English & Fante, hymn in a white growing box, scripture with a reference bar, two stream
  lower thirds, bold announcements.
The NDI renderer draws all of the above except transitions and videos.

## Go live always works; NDI as an output monitor (12 Nov 2026)

- Go live, double-click (Preview slide or schedule item) and clicking a Live slide always
  work. Earlier, Blank, Logo, Clear, or the black start after a restart "held" the output
  and Go live was refused until Resume (the screens stayed black). Now Go live shows the
  new item and ends Blank/Logo; clicking a Live slide changes it behind Blank/Logo;
  Resume returns without changing anything. Go live also releases outputs held after
  being rebuilt.
- As in EasyWorship: Settings → Outputs → Output monitor → "NDI Stream", with an output
  format (1920×1080, 1280×720, 1024×768 or 4K at 25/30/50/60 Hz). That output opens no
  window; the built-in renderer sends it as "Worship Presenter Main" (or Alternate, the
  see-through lower-third look). Same theme choice as the window (item → output → rules →
  default), Blank, Logo, Clear, pictures and presentation slides. Started again at every
  launch.
- Video over NDI, as FreeShow does it: FreeShow sends its real output page (rendered in a
  hidden window that is never throttled, autoplay allowed) frame by frame. Worship
  Presenter does the same with a real output window at the NDI size kept off the visible
  desktop and captured (WebView2 has no off-screen painting): videos, animations,
  transitions, camera and alerts all move. Every window now runs without background
  throttling or occlusion pausing (FreeShow's backgroundThrottling: false), so covered
  projectors keep playing too. The alternate (lower third) keeps the built-in renderer
  for exact transparency; off Windows, or if capture fails, the built-in renderer sends
  stills.

## More Bibles and sync (13 Nov 2026)

- More Bibles (Scriptures sidebar → More available…): eBible.org's freely shareable
  translations, searchable by language or title, downloaded into the library (a backup
  first). Verse-per-line format (text or XML); English book names, renameable.
- Sync (Settings → Sync) through a shared folder (OneDrive, Google Drive, network folder,
  USB drive): one computer shares its library (at start, every 30 minutes, Share now);
  the others follow (at start, or Update now when nothing is live; a backup first).
  Shared: songs, Bibles, themes, theme rules, media, presentations, services, collections.
  Each computer keeps its own settings and outputs.

## Buttons: Stream Deck, Companion and MIDI (13 Nov 2026)

Settings → Buttons and MIDI.
- Stream Deck through Bitfocus Companion (or any button box that can call a web address):
  make a control key, then give each Companion button an address such as
  http://PC:7300/api/v1/go-live?key=… (go-live, next, previous, next-item, previous-item,
  blank, logo, clear, resume, item/3). Needs the phone remote on. A wrong key does
  nothing and is logged; making a new key stops the old one.
- MIDI pads, keyboards and mixers: choose the device, press Learn beside an action, press
  a pad. Reconnects at every start.

## Preparing and after the service (13 Nov 2026)

- Sermon outline to slides (Presentations → New, or Ctrl+K): paste the outline; each line
  becomes a point slide and every reference in it (Romans 8:28, 1 Cor. 13:4-7, Ps 23)
  is followed by its passage from the chosen Bible. Checked, then saved as a presentation.
- Sermon transcript (Settings → Scripture recognition): everything heard since Listen,
  with times and the scriptures read, saved as a text file for notes or posts.
- Printing through Windows' print dialog (or Save as PDF): song words (every language)
  and chord charts (right-click a song → Print), the service order (Ctrl+P), and the
  song usage report.
- My collections for scripture and media (EasyWorship's sidebars): right-click a verse or
  media item → Add to collection; open a collection to preview, go live or drag items.

## Sunday helpers (13 Nov 2026)

- Quick box: "555", "MHB 1", "ncan 12" or "Yohane 3:16". A number alone uses the main
  hymnbook (Settings → Library); where a number has an English and a bilingual song, the
  bilingual one comes first.
- Pre-service loop (service bar → Pre-service): countdown to the start time, the
  announcements turning by themselves, background music; on time the first schedule
  item goes live (music fades as on any Go live), or the logo shows.
- Offering slide (Presentations → New, or Ctrl+K): MoMo / bank numbers and an optional
  QR code in the brand colours. The QR holds exactly what the church pastes (its GhQR
  text or a payment link); no payment codes are invented.
- Follow along on phones (Settings → Phone remote): a QR code opens the words on the
  screens on the congregation's phones (English, Fante or both; large print), read-only,
  no PIN, only while switched on. Phones see only the words on the screens.

## Scriptures tab (13 Nov 2026)

- Opens on a chapter straight away, as EasyWorship does: the last chapter you opened,
  or Genesis 1, with nothing typed.
- English works in every Bible: "John 3:16" with the Fante Bible chosen opens Yohane 3:16.
  English words ("for God so loved the world") with a Fante or Twi Bible chosen are
  found through your English Bibles and shown from the chosen Bible, marked "via NKJV";
  words in the chosen Bible's own language are searched in it directly.

## Live picture: switch outputs (13 Nov 2026)

As EasyWorship's Live Output menu: above the Live picture, choose Live Output (main
screen), Alternate Output (the lower third, see-through parts on the checkerboard,
blank for announcements, media, Blank and Logo, as the real output) or Foldback Output
(the stage display). Remembered between sessions.

## Settings is the home for setup (12 Nov 2026)

The title bar's Commands button is now Settings. Setup and management live there:
Appearance (incl. Reset pane sizes), Brand kit, Import, Backups, Profiles, Keyboard,
Projection, Outputs and NDI, Service intervals, Scripture recognition, Phone remote,
Library, About. Live-service tools (Timer, Alert, Music, Rehearsal, service orders) stay on
the service bar. Ctrl+K still searches everything; its setup entries open Settings pages.

## Smarter scripture recognition (12 Nov 2026)

- Whisper (OpenAI's model, run on this computer through whisper.cpp) is now the engine:
  far better with accents and a live room than Vosk or Windows Speech. Settings →
  Scripture recognition → Whisper → model Base (~150 MB) or Small (~490 MB, more
  accurate) → Download once. It hears whole phrases (transcribed at each pause, or every
  7 s), with Bible book names as a hint; its habitual "Thank you." on silence is dropped.
  whisper.cpp is compiled into the app (needs CMake, and LLVM/clang to generate its bindings;
  the Windows workflow sets LIBCLANG_PATH, the Codespace installs libclang-dev).
- It follows the preacher: "verse seventeen", "verses sixteen to eighteen", "chapter four
  verse two", "the next verse", "the next chapter", and a reference spread over two
  sentences ("turn to the book of Romans" … "chapter eight and verse one"). A lone book
  name that is also a word ("a good job") never counts.
- It recognises a verse read aloud without its reference: the last 15 s of speech are
  matched against the Bibles (at least six words in the verse's order, a misheard word
  allowed); everyday phrases ("the Lord is good") do not trigger it.
- Test bench: Settings → Scripture recognition → Test with a recording: runs a sermon
  recording through the same steps and lists every suggestion with its time, plus the
  transcript.
Suggestions still only appear above Live; the operator chooses Preview or Go live.

## Security

See docs/security/audit-2026-11-11.md: findings, fixes and what remains. Before each release run `cargo audit` and `npm audit` (in ui/).

## What's next

- NDI results from Windows; Windows sign-off checks.
