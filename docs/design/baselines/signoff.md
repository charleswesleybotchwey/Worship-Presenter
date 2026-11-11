# Design sign-off record (design brief §8)

Do not mark a check passed without evidence. "Browser" means checked in the
development preview (Chromium, sample data); Windows evidence is still required.

| ID | Status | Evidence | Still needed |
|---|---|---|---|
| V01 Fidelity | Not started | Fixture page (#/fixture) and baselines exist | R1–R4 reference crops in docs/design/references, paired review by the designer |
| V02 Regression | Browser baselines approved for development | `python tests/ui/baselines.py check`: 17 images (8 service sizes × 2 themes, fixture) identical on repeat capture | Windows client captures at 100/125/150% |
| W01 Sizing | Browser pass | 1440×900, 1280×720, 1092×614, 800×560: no page-wide horizontal scroll; Live state, Blank and Go live visible | 1366×768 laptop at 125% Windows scale |
| W02 Window behaviour | Partly | Native caption kept; closing during a service asks first and cancel leaves output | Snap, maximise, monitor moves with mixed DPI on Windows |
| A01 Accessible chrome | Partly | Brief's token contrast; forced-colours rules; focus ring on every control in the fixture; named controls | Narrator pass, 200% text size, Windows high contrast |
| S01 No accidental transmission | Browser pass | Typing "C L 123", spaces, "Yohane 3:16", Enter and Esc in fields: zero output changes; rehearsal never reaches outputs; song save never changes the live slide | Repeat on Windows with the Rust core and a projector |
| S02 Failure and state | Partly | Disconnect shows the last confirmed picture and time; returning display held black until Resume; Blank/Logo/Clear latch; recovery restores into Preview | Automated delayed/out-of-order acknowledgement fixtures |
| G01 Ghanaian service | Partly | MHB 50, Yohane 3:16, English/Fante with unequal lines (pairing view flags them), Ɛ ɛ Ɔ ɔ Ŋ ŋ Ɔ́ in rows and slides | 4:3 projector and 16:9 alternate rehearsal |
| P01 Low-spec | Not started | Low-spec interface and low-spec projection switches exist (Settings) | Measure on the 4 GB pilot laptop with a 5,000-song library |

Deviations from the brief, with reasons:
- At Charles's request (EasyWorship behaviour): arrow and page keys drive Live from anywhere outside text fields and lists (Blank/Logo/Clear/number keys still need the Live monitor); schedule double-click goes live; Go live works with no main display (shown in Live only); the main output opens on a secondary monitor automatically on first run.
- 3D look by default, at Charles's request (10 Nov 2026): raised buttons, card sections, framed monitors, gradient bars, built from the brief's tokens so Dark and Light both work. Flat (the brief's look) stays one click away in Settings, Appearance, Look. Text selection is switched off outside text fields (double-click to go live was painting words blue). A sun/moon button switches Light and Dark.
- Layout follows EasyWorship, at Charles's request (10 Nov 2026): Schedule, Preview and Live across the top (each with a text slide list and a small output monitor), the library across the bottom with a folder tree and sortable columns. Tokens, focus, safety and keyboard rules from the brief are unchanged. The screenshot baselines must be recaptured after review.
- Double-clicking a Preview slide (or the Preview picture) goes live, as in EasyWorship, at Charles's request. It runs the same checks as Go live (held output, no main display, rehearsal). Double-click in the Library and Schedule still never transmits, as the brief requires.
- Linear reference captures are not in the repository: they must be taken on a Windows machine with permission; the manifest lists what to capture.
- The brief names PMKY Worship Centre as the first acceptance site; the app shows the profile name rather than a hard-coded church, as agreed for multi-church use.
