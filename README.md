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
- Output setup and Settings: Identify shows a test pattern for 4 s; asks first if Main is live.
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

## What's next

- NDI results from Windows; Windows sign-off checks.
