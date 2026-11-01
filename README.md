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

- wp-core: 25 tests pass (22 unit, 3 against your real files).
  - EasyWorship songs: 4,531 of 4,531 imported, 529 look-alike characters fixed and logged.
  - EasyWorship Bibles (.ewb): all 17 imported. Bible Show (.bib): NKK2011, TK2012, YGZP2020 imported.
  - Our own Access (Jet 4) reader matches mdbtools row for row on every table of all three .bib files.
- Tauri shell: compiles with no errors or warnings (checked on Linux with Rust 1.91).
- UI: builds clean; checked in a browser with sample data.

Not yet done: running the desktop app on Windows. Use the cloud build below.

## Getting the Windows installer without installing anything (recommended)

GitHub builds the installer for you on its own Windows machines.

1. Create a free account at https://github.com and a new **private** repository.
2. Upload this whole folder to it (GitHub Desktop is the easiest way: File, Add local
   repository, then Publish). The .gitignore keeps build files out.
3. Open the repository's **Actions** tab. The "Windows installer" run starts by itself
   and takes about 20–30 minutes the first time, less after that.
4. When it shows a green tick, open the run and download **worship-presenter-windows**
   under Artifacts. Unzip it and run the setup .exe.

Every time you upload a change, a new installer is built. If a run fails, copy the red
error text from the failed step and send it to me.

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

## What's next (Phase 0 remaining, then Phase 1)

- First Windows build; fix anything the compile turns up.
- Fault-injection tests for the output windows (spec failure model).
- NDI alpha-capture prototype.
- Import screen in the UI (the `import_file` command exists), schedule saving to the database.
