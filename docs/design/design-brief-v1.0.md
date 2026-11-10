# Worship Presenter · Windows interface design

**Implementation brief v1.0** · Companion to the Worship Presenter product specification of 30 October 2026.  
**Target:** Tauri 2 / React / TypeScript Windows operator application. **First acceptance screen:** a volunteer running a Sunday service at PMKY Worship Centre.  
**Reference:** Linear’s desktop **product**, not its marketing website. The reference baseline is its March 2026 interface refresh and June 2026 desktop tab behaviour. This document does not claim access to Linear’s private design tokens. [1] · [2]

## 1. The design decision

Build a console that looks and behaves like Linear wherever the operator is navigating, searching, editing or choosing a command. Keep the parts that perform a service unmistakably Worship Presenter: **Schedule → Library → Preview → Live**. Do not build an issue tracker, and do not put a dashboard in front of the service.

The key layout decision is that **Schedule is the sidebar**. Do not bolt a permanent Linear-style navigation rail onto the four existing panes. The schedule supplies the dim left edge; Library supplies the dense list; Preview and Live occupy the brighter work surface. A volunteer must be able to read what is on air while searching for the next hymn.

| Match the reference closely | Adapt deliberately | Never inherit |
|---|---|---|
| Relative emphasis of sidebar and content, compact tabs, header alignment, list density, menus, keyboard navigation and restrained selection. | Service order instead of issues; content-type switcher instead of project filters; an explicit Go live action; confirmed output state instead of an ordinary selected row. | Linear’s logo, illustrations, proprietary icon artwork, source code, project-management terminology, or marketing effects. Use the Worship Presenter name and original assets. |

**Order of precedence:** public-output safety → keyboard and accessibility requirements → fidelity to the approved reference → convenience. The visual rules below replace the product spec’s generic references to Raycast and Figma. They do not change feature phases. A future tab or control appears when its feature works, not as a decorative placeholder.

## 2. Freeze the reference before styling screens

Linear’s published refresh shows a dimmer sidebar, compact desktop tabs, quieter dividers, smaller icons and a warmer neutral palette. Its desktop tab update separates the navigation history of each tab. Use those observable relationships, not an assumed signature purple or a dark-mode preset. [1] · [2] Linear’s earlier design notes explain the relationship between sidebar and top chrome and identify Inter as its UI text face. [3]

| ID | Reference to freeze | Where the comparison applies |
|---|---|---|
| R1 | Sidebar, tab strip and border comparisons in Linear’s [2026 refresh][1] | Schedule sidebar, workspace tabs, row insets, pane boundaries |
| R2 | A populated list with its view controls; see [Display options][4] | Library toolbar, list headers, icon placement, selection |
| R3 | Command bar with a selected item; see [Select issues][5] for the interaction model | Command palette, scoped results, shortcut hints, arrow-key selection |
| R4 | A settings page and light-theme list; see [settings redesign][6] and [UI design notes][3] | Preference rows, narrow content column, light palette |

**Reference pack:** Before visual sign-off, put dated, permitted reference crops in `docs/design/references/` (review material only, never runtime assets). Prefer same-scale Windows client captures, record build/date, theme, viewport and display scale in `manifest.md`, and label public article crops as such. For each reference, save the matching Worship Presenter view beside it. Mask product-specific text and artwork; compare chrome, rhythm and alignment. Do not label this a pixel-for-pixel match to the Windows client until compared with actual Windows client captures. A future Linear change does not silently change the approved baseline.

## 3. The service window

The diagram shows ownership and reading order, not literal column widths. Bracketed elements are controls. The native Windows caption sits above this client area.

```text
[Service] [Song editor]                     Worship Presenter                 [Commands]
PMKY Worship Centre / Sunday service      [Quick scripture]  [Blank] [Logo] [Clear] [Go live]
┌──────────────────┬─────────────────────┬──────────────────────┬────────────────────────┐
│ Schedule     [+] │ Library       [···] │ Preview              │ ● Live · Main          │
│ Sunday service   │ [Songs ▾]  [Filter] │ MHB 50 · Verse 1    │ PAN 123 · Chorus       │
│                  │ [Search songs…    ] │                      │                        │
│ 01  Opening      │                     │    Staged slide      │    On-air slide        │
│ 02  PAN 123      │ MHB 50   Song title │    (not public)      │    (acknowledged)      │
│ 03  Scripture    │ PAN 123  Song title │                      │                        │
│ 04  Offering     │                     │ [V1] [C] [V2]        │ [V1] [C] [V2]          │
│                  │                     │ [Language ▾]         │ [Previous] [Next]      │
└──────────────────┴─────────────────────┴──────────────────────┴────────────────────────┘
Main: live  ·  Slide 3 of 8                          Backup: saved                09:41
```

### Geometry and resizing

All dimensions are logical CSS pixels at default text size, not physical display pixels. Pane widths below **include** their one-pixel inset divider. Only the pane bodies scroll. The caption, workspace tabs, service bar, pane headers and status strip stay fixed.

| Client width | Schedule | Library | Preview | Live |
|---|---:|---:|---:|---:|
| 1440 px reference | 224 | 320 | 448 | 448 |
| 1280 px reference | 200 | 264 | 408 | 408 |
| Above 1440 | Start at 224 | Start at 320 | Split remaining space equally with Live | Same as Preview |
| 960–1279 | Combine Schedule and Library in one tabbed source pane, 232 px by default | Same pane | Half the remainder | Half the remainder |
| Below 960 | One task at a time: Schedule, Library or Preview | Same task surface | Same task surface | Persistent compact Live monitor, state label and service actions |

Interpolate the first two columns between the reference sizes. At full width, drag separators to redistribute space; minima are 176 / 224 / 280 / 280. A separator has a 1 px visible rule, an 8 px hit area, keyboard arrow adjustments of 8 px, and a “Reset pane sizes” command. Save proportions per profile and width band. Clamp invalid saved values on launch or DPI change. Restore the last source tab on return from compact mode. **Never hide a live-state warning or the Blank action to make the columns fit.**

**Vertical rhythm:** workspace tab strip 32 px; service bar 44 px; pane header 36 px; status strip 24 px. In short windows, scroll the thumbnail lists before shrinking a labelled control. Below 960 px use a two-row service bar; the output state and Blank, Logo, Clear and Go live stay visible. Move quick scripture to a labelled search trigger, not offscreen. Below the preferred 1280 × 720 workspace, adapt rather than forcing a minimum window that cannot fit a scaled Windows desktop.

**Windows frame:** Start with native caption controls. Do not draw macOS traffic lights or a second row of fake minimize/maximize/close buttons. An integrated title/tab bar is an upgrade only after drag, double-click maximize, system menu, `Alt+F4`, keyboard snapping and Windows 11 Snap Layouts pass on applicable systems. Drag areas must exclude interactive controls. During a service, a close request asks whether to end output; cancelling leaves it unchanged. [8] · [9]

**Chrome ownership:** The profile and service name belong on the left of the service bar. The primary action owns the far right. The status strip reports all enabled output roles, clock, and backup condition; a fault also appears at its affected pane. Selecting an output status opens details; it does not reroute a display. The Service workspace tab is pinned and non-closable. Editors can open beside it; switching tabs never changes what is on air. Outside Service, keep a compact persistent service strip with output state, Blank and Return to service. Modal dialogs preserve normal focus rules; an operator can dismiss them to reach the strip.

## 4. Visual tokens

These are **Worship Presenter build values**, not extracted Linear values. Put them in one scoped token file. Dark is the initial theme; provide Light, Dark and Follow Windows thereafter. The operator theme never changes a projected slide.

| CSS token (`--wp-…`) | Dark | Light | Use |
|---|---|---|---|
| `shell` | `#191918` | `#F5F4F2` | Sidebar, tabs and surrounding chrome |
| `canvas` | `#201F1E` | `#FCFBFA` | Library, Preview, Live and editor surfaces |
| `overlay` | `#292827` | `#FFFFFF` | Menus, palette and modal surfaces |
| `hover` | `#2C2B2A` | `#F0EFED` | Pointer hover |
| `selected` | `#343331` | `#E7E6E3` | Selected row, active workspace tab, pressed neutral control |
| `line` | `#383735` | `#DEDDDA` | Decorative dividers; never the sole indicator of a state |
| `edge` | `#85827C` | `#75736D` | Functional boundaries that must be discernible |
| `ink` | `#F1F0EE` | `#22211F` | Primary text |
| `secondary` | `#C4C2BE` | `#4E4C49` | Secondary text and informative icons |
| `muted` | `#A5A29D` | `#62605C` | Metadata, keyboard hints, inactive navigation |
| `accent` / `on-accent` | `#AEC3F4` / `#182238` | `#39558D` / `#FFFFFF` | One primary action in a scope; not a sidebar wash |
| `focus` | `#B4CAFA` | `#385790` | 2 px keyboard focus ring, 2 px offset |
| `live` | `#8AD6AA` | `#216542` | Confirmed live state, always with a label |
| `warn` | `#F0C07A` | `#80500C` | Pending, stale, missing media or recoverable fault |
| `danger` | `#FFA39F` | `#973A38` | Failed output, destructive action or unrecoverable error |

**Hierarchy:** Only **Go live** is accent-filled in the service bar. Blank, Logo and Clear are neutral buttons with visible labels and distinct pressed states. Use text plus an icon to indicate a pressed state; do not rely on the subtle selected fill. A neutral row selection is not an on-air indication. Reserve semantic colours for meaning, never for categorising all content types. No gradients, glass, shadows, oversized headings, nested cards, or decorative illustration in operator chrome. A slide author may use gradients or shadows *inside a slide*.

| Element | Size / leading | Weight and application |
|---|---|---|
| Main UI | 13 / 20 px | Inter 400; 500 for controls and selected labels |
| Pane / toolbar title | 14 / 20 px | 600; sentence case |
| Standalone editor title | 18 / 24 px | 600; not used as a banner |
| Metadata / keyboard hint | 12 / 16 px | 400 / 500; never use small text for lyrics on projection |
| Numerals | Inherit | Tabular figures for clock, timings, counters and service positions |

Use local font assets, with `Inter, "Noto Sans", "Segoe UI", sans-serif` for operator text. License and package approved fonts for offline use. Preserve case, marks and original text in displayed titles and lyrics. Use a separate font chain in the output theme; Inter is not a compulsory congregation font. Test `Ɛ ɛ Ɔ ɔ Ŋ ŋ`, `Ɔ́` and canonically equivalent sequences in rows, the editor and the projector renderer.

**Spacing and shape:** an 8 px layout grid, with 4 px optical increments. Use 8 px icon-to-label, 12 px row inset, 16 px pane inset, 24 px section gap. Row/control radius 4 px; attached content surface or menu 6 px; dialog 8 px. No card corners between adjacent service panes. Use 14 px secondary icons, 16 px standard icons, an original consistent icon family on a 16-unit grid, and a 1.5-unit optical stroke. Pointer hit areas are larger than the glyphs: 28 px desktop buttons, 32 px list rows, 40 px in touch mode. Highlight the whole row, not a small pill around its name.

**Motion:** 100 ms hover/press; 120 ms menu or palette opacity; no layout bounce or animated selection travel. Render keyboard focus and live/fault state immediately. With reduced motion or low-spec mode, cut UI transitions. Projection transitions are separate and follow the service theme or its low-spec override.

## 5. Component and screen recipes

| Component | Build and interaction contract |
|---|---|
| Service row | 32 px single line, 48 px only when a second line is enabled. Reserve columns for order, type glyph, title and state. Ellipsise only the title; reveal the full title on focus or hover. Selection stages the item. A different row may retain a speaker glyph and the word “Live”. Drag handle also has keyboard Move up/down actions. |
| Library row | Leading hymnal number or type, title, trailing metadata. All rows share column guides. Right-click or `Shift+F10` opens the same context menu. Double-click opens Preview, never Go live. Filters show their active count and a Clear action. Missing results retain the query and offer Clear filters. |
| Pane header | Title and optional count left; at most two common actions right; the rest in an ellipsis menu. Library types are a compact tab row when they fit, otherwise a named type switcher. Keep the current type visible. |
| Slide surface | Preserve source aspect ratio using contain, not crop. Letterbox outside the artwork. Caption and controls stay outside the slide. Cache thumbnails from the same layout engine as the output; never approximate bilingual line breaks with an unrelated UI text box. |
| Slide grid | Section tag and position visible as text. Preview thumbnails select the cue; Live thumbnails are direct output controls and are labelled as such. Mark the confirmed on-air slide with glyph + “Live”. Do not outline an entire pane in red or green. |
| Command palette | Centred horizontally, top edge about 18% of the client height; 520 px max width, viewport minus 32 px on narrow windows. Show one search field, grouped 32 px rows, keyboard hints and an empty state. Up/Down moves selection; Enter executes the named **safe** action; Esc dismisses and restores focus. Output-changing commands open the relevant control for explicit activation rather than firing from search. [5] |
| Menu / dialog | Menu rows 28 px; align icons and key hints in columns. Context menu includes only relevant actions. A dialog traps focus, names its destination, and returns focus to its invoker. Default a destructive confirmation to Cancel, not Delete. Clipped menus flip or scroll within the client area. |
| Feedback | Save success is a quiet status-line update, not a toast on every edit. A temporary message can offer Undo for a library action. A public-output fault stays visible until resolved. Never obscure Go live or send a toast to an output. |

| Screen | Layout and content rules |
|---|---|
| Service / empty start | Open directly to Service. Empty Schedule: “No items in this service” and **Add from library**. Preview: “Select an item to preview.” Main unassigned: “No main display” and **Set up output**; disable Go live. No welcome dashboard. |
| Song editor | Split local editor and preview. Language tabs above lyrics; section order beside the text. Show English/Fante segment pairing explicitly, flag unpaired segments, and let the editor fix them. Separate **Save to library** from **Apply to this service**. Neither silently changes an on-air slide. |
| Scripture picker | Version → reference → results → Preview. Accept a fixture such as `Yohane 3:16` and show the resolved book, version and range before projection. Keep Add to schedule separate from Go live. Parallel scripture exposes the two version names, not just text colours. |
| Theme / announcement editors | Layer or slide list left, true-aspect canvas centre, properties right. Use the same rows, menus and inputs as Library. Contextual guides and resize handles appear only when needed; never in audience output. Save a theme without applying it live. |
| Media manager | Dense list by default; optional thumbnails. Show type, duration, missing-file state and Relink. Preview playback is muted by default; indicate the audio destination before enabling sound. |
| Settings / outputs | Compact category sidebar, one 680 px max-width settings column, labelled rows and short explanations. Output setup names the role, physical destination and connection state. Identify/test pattern requires an explicit action and warning for an active main destination. Never auto-assign a lost public output to the operator display. |
| Import / recovery | Import flow: choose source → review recognised, ambiguous, rejected and duplicate records → commit → show report. After interruption, restore the service into Preview; public output remains safe until the operator chooses to resume. An unavailable Bible or media file gets a specific message, not an empty black preview. |
| Stage / stream boundary | Main, foldback and alternate outputs render their role-specific content only. No sidebars, keycaps, window caption, fault banners, debug overlays or status chips on public outputs. The remote may share tokens and terminology, not the four-pane desktop layout. |

## 6. Keyboard and live-output rules

**Separate inspection from transmission.** Selection, hover, tab changes, typing, resizing, filtering and editing must never issue a public-output command. Only labelled service controls, their scoped shortcuts, and explicit actions inside the Live pane may do so. The Rust core owns live state; React owns navigation and draft state. The “Live” label means the renderer acknowledged the displayed revision, not that the physical projector lamp was independently verified.

| Input | In a service-driving context | In a field, list, palette or modal |
|---|---|---|
| `Ctrl+K` | Open commands | Open only when the active editor has not claimed it; never send a live command. |
| `Ctrl+F` / `Ctrl+G` | Focus library search / quick scripture | Keep an editor’s local Find when appropriate. Quick scripture stages a result. |
| `F5` | Send the valid preview cue to enabled outputs using their role policies | Suppress in editors, overlays, and text entry; prevent WebView reload in the packaged operator app. |
| `Space`, `Enter`, `←`, `→`, `Page Up/Down`, `1`–`9` | In the *Live driving surface*, advance, step back or jump by the configured mapping | Native focused control wins. A space types a space, Enter selects a search result, arrows move a caret or list focus. No output side effect. |
| `Esc`, `L`, `C` | In the Live driving surface: Blank, Logo, Clear respectively | Esc closes the top layer or exits the field. Letters remain text; a single key event never both dismisses a layer and blanks the projector. |
| `Ctrl+Z` | Restore the previous live cue only in Live driving scope and only when that action is available | Undo text or local edits normally. Never recall a public slide from an editor. |
| `Tab`, `Shift+Tab`, `F6` | Traverse controls; F6 cycles work regions in reading order | Overlays contain their own focus; close restores it. Support an on-screen alternative for every service hotkey. |

After a successful Go live from the Service view, return keyboard focus to the Live driving surface. `Space`, `Enter` and `→` advance one slide; `←` goes back one slide; `Page Down` / `Page Up` change service item; `1`–`9` select a numbered section of the current item. At the end of an item, keep the last slide unless an explicit advance policy was configured. A held key must not step through an entire hymn.

Consume a handled key once. Ignore IME composition, repeat keydown for one-shot commands and unintended modifiers. Rebinding must detect conflicts; include a preference to disable single-character presentation shortcuts. Do not install process-wide or OS-wide typing hooks. Windows focus, not a stale “last clicked Live” flag, determines the scope.

### What the operator sees when output changes

| Condition | Display and allowed action |
|---|---|
| Ready | Preview identifies the staged item, section, language mode and main target. Go live is enabled only for a valid cue and available main target in the pilot configuration. An explicitly selected rehearsal mode operates on a simulator labelled **Rehearsal**, never on public output. |
| Requested | Send once with command ID and target revision. Keep the last acknowledged live image and show “Sending…” until the main renderer acknowledges. Ignore stale acknowledgements and duplicate commits. |
| Live | Show “Live · Main”, confirmed item, section and position. Each other enabled output has its own status; successful main output does not imply successful lower-third delivery. |
| Blank / Logo / Clear | Name the applied mode: “Main: blank”, “Main: logo”, or “Main: text cleared”. Blank suppresses content; Logo applies the configured role policy; Clear removes the text but retains the background. In the pilot these modes latch: Preview navigation remains available; Go live and direct Live advancing are disabled until **Resume output**. Resume restores the previously confirmed cue, not an unsent Preview selection. Later role policies must report their individual result. |
| Late or disconnected | Initial alert threshold: 1 s without a requested acknowledgement. Show “Main: not confirmed” and the **last confirmed** thumbnail/time, not a green Live badge. On connection loss show “Main: disconnected”. Offer Reconnect / Output setup; never replay queued commands on reconnect. Reconcile against a fresh snapshot and require an explicit resume before revealing content on a returned public display. |
| UI reload / power recovery | Recreated control UI requests a fresh core snapshot; it does not push default values on mount. A cold restart shows the recovered cue in Preview and names the recovery time. Changing a UI theme, navigating Back, or closing an editor must not touch output. |

**Acceptance example:** While `PAN 123 · Chorus` is live, type `Yohane 3:16` in quick scripture and press Enter. The verse appears in Preview; Main still shows the chorus. Press Esc to leave the field: Main still shows the chorus. Use Go live: the Live monitor changes only after acknowledgement. Disconnect Main: the monitor identifies its last confirmed frame instead of pretending that it is still on air.

## 7. Implementation order

This is a UI work order, not a new product release plan. Build one complete path before styling all the secondary screens. Keep theme-editor, remote and NDI controls behind their actual feature phase.

| Step | Deliverable | Exit test |
|---|---|---|
| 1. Reference and tokens | Freeze R1–R4. Implement named dark/light tokens, font files, original glyphs, focus and forced-colour styles. | A single fixture page shows every control in default, hover, focus, selected, disabled and fault states in both themes. |
| 2. Service shell | Build native-framed workspace, fixed service bar, four resizable panes, compact layouts and status strip using synthetic data. | Reproduce the reference pane widths; no clipped controls or horizontal page scroll at the specified sizes. |
| 3. First vertical slice | Load a fixture schedule and library; search `MHB 50`, stage a slide, Go live, Blank and Resume. Wire to a test output adapter, then the Rust core and one real projector. | A collected command log proves that only intended actions changed output. Operator can tell staged, confirmed, blank and disconnected apart. |
| 4. Editing and commands | Implement command palette, context menus, song editor, scripture picker, Settings and Output setup; reuse the same primitives. | Complete search, edit, reorder and display assignment using keyboard only. Repeat with text-entry focus and a disconnected display. |
| 5. Pilot and lock | Compare reference pairs, record Windows build/DPI, run failure and accessibility checks, then rehearse at PMKY. Reuse approved patterns for later screens. | Sign-off record includes paired images, test results, remaining deviations and operator approval; no unresolved public-output safety issue. |

### Suggested repository boundaries

```text
docs/design/references/         # Dated comparison manifest; review-only material
docs/design/baselines/          # Approved Worship Presenter screenshots and fixture IDs
src/ui/tokens.css               # Sole operator colours, sizes, typography and motion tokens
src/ui/primitives/              # Button, Icon, Row, Tabs, Field, Menu, Dialog, Splitter, Status
src/ui/shell/                   # WorkspaceTabs, ServiceBar, ServiceLayout, StatusStrip
src/features/service/           # SchedulePane, LibraryPane, PreviewPane, LivePane
src/features/commands/          # Command registry, focus scopes, palette, keyboard routing
src/features/editors/           # Song, scripture, media, themes and settings surfaces
src/live/                       # Typed core adapter, acknowledgements, per-output selectors
src/output/                     # Projected scene; imports no operator chrome or token reset
src-tauri/                      # Actual command validation and authoritative live state
tests/ui/                       # Visual baselines, focus tests, command traces and failure fixtures
```

Tailwind utilities should resolve to the same CSS variables; do not introduce a parallel palette with arbitrary colour classes. Scope global styles to `.wp-control`, not `body` across every webview. Store drafts and panel sizes in UI state; do not make a local store a second authority on what is public.

**Starter layout contract** (defaults before saved user sizes are applied):

```css
.wp-service-grid {
  --schedule-default: clamp(200px, calc(15vw + 8px), 224px);
  --library-default: clamp(264px, calc(35vw - 184px), 320px);
  display: grid;
  grid-template-columns:
    var(--schedule-width, var(--schedule-default))
    var(--library-width, var(--library-default))
    minmax(0, var(--preview-share, 1fr)) minmax(0, var(--live-share, 1fr));
  min-block-size: 0;
  block-size: 100%;
}
.wp-pane { box-sizing: border-box; min-width: 0; min-height: 0; }
.wp-pane + .wp-pane { border-inline-start: 1px solid var(--wp-line); }
.wp-control :focus-visible { outline: 2px solid var(--wp-focus); outline-offset: 2px; }
@media (prefers-reduced-motion: reduce) {
  .wp-control { --wp-hover-time: 0ms; --wp-overlay-time: 0ms; }
}
```

Use the shell’s **available client width** to choose the compact component tree; the CSS above is for the full-width shell only. If embedded in a smaller host, derive the clamps from measured container width rather than `vw`. Implement viewport changes without unmounting the output adapter or resetting the focused item ID.

## 8. Release checklist

These are pass/fail tests for the implementation, not claims that an app has already been built or measured.

| ID | Procedure | Pass condition |
|---|---|---|
| V01 · Fidelity | Review dark/light Service, Library selection, command palette, editor and Settings against their frozen counterparts. Compare like components, not entire unrelated screens. | Sidebar recedes; controls align to common guides; tab, row and header heights are within 2 CSS px of the approved Worship Presenter baseline. Any intentional difference is recorded. |
| V02 · Regression | Capture fixed-size images with deterministic content, fonts loaded, clock frozen, animations off and known scale. | No unreviewed change in the operator chrome. Mask only variable slide art or native OS regions. A screenshot change is not approved merely by updating its baseline. |
| W01 · Sizing | Check client fixtures at 1440×900, 1280×720, 1092×614 and 800×560; also use a 1366×768 physical laptop at 125% Windows scale. Test 100/125/150% on supported Windows configurations. | No page-wide horizontal scroll, inaccessible dialog buttons, overlapping controls, or clipped Live/Blank labels. Compact mode keeps last selected cue and public state. |
| W02 · Window behaviour | Drag, maximize, restore, snap, move between monitors with different DPI, and cancel a live close request. | Frame behaves like a Windows app; cancel leaves output unchanged. Disconnecting the projector never exposes the operator interface on the returning output. |
| A01 · Accessible chrome | Check both themes, forced colours, 200% UI text size, Narrator, keyboard-only navigation and reduced motion. | Normal text ≥4.5:1, meaningful non-text boundaries/focus cues ≥3:1 against adjacent colour, visible focus, named controls, no colour-only meaning. At 200%, rows grow or reflow rather than clipping; critical actions stay reachable. Decorative lines are not accessibility boundaries. [7] |
| S01 · No accidental transmission | With a cue live, type `C L 123`, spaces and `Yohane 3:16`; use arrows, Enter and Escape in fields, palette and modals. Switch tabs and save an edit. | Zero output-changing commands. The previous live cue remains public. A deliberate F5 from a valid service context sends exactly one command. |
| S02 · Failure and state | Simulate delayed/out-of-order acknowledgements, a duplicate command, main unplug, alternate-only failure, UI reload, then cold restart. | No false on-air indicator, no automatic reveal on reconnect, no broadcast of stale pending actions. The service strip accurately distinguishes main and alternate state. |
| G01 · Ghanaian service | Rehearse `MHB 50`, `PAN 123`, `Yohane 3:16`, English/Fante pairs with unequal line counts, and combining marks. Test a 4:3 projector and 16:9 alternate fixture. | Correct song/passage, no lost marks, aligned translations, no clipped text. Operator colours and controls never leak into either output. |
| P01 · Low-spec operation | Run on the designated 4 GB pilot laptop with a 5,000-song fixture and one projector; record CPU/GPU, scaling and WebView version. | Search and navigation meet the separately agreed product performance gates. No visual delay is introduced merely to complete a UI animation. Expensive thumbnails are deferred; low-spec UI uses cuts. |

**Token check:** Using the specified opaque sRGB pairs, muted text on the selected-row background computes to **4.96:1 dark / 5.03:1 light**; the Go live label on its accent computes to **8.99:1 / 7.35:1**. These are palette calculations, not a substitute for checking rendered states, system contrast mode, or projected text over media. [7]

**Sign-off:** The designer approves the reference comparison; the Windows engineer approves sizing, focus and state traces; a PMKY operator runs the acceptance example without coaching and identifies both the current and next item. Keep results and justified deviations in `docs/design/baselines/signoff.md`. Do not mark a check passed without evidence.

## Reference key

1. [Linear — A calmer interface for a product in motion (March 2026)][1]. Sidebar, tabs, borders and palette comparisons.
2. [Linear — Desktop navigation history and pinned tabs (June 2026)][2]. Reference for tab identity and history, not a mandate to build a browser.
3. [Linear — How we redesigned the Linear UI (March 2024)][3]. Sidebar-to-header structure, typography and theme testing.
4. [Linear Docs — Display options][4]. Scoped view controls and list presentation.
5. [Linear Docs — Select issues][5]. Selection, command bar and contextual actions.
6. [Linear — Personalized sidebar and new settings pages][6]. Settings and navigation reference.
7. [W3C — Web Content Accessibility Guidelines 2.2][7]. Criteria for contrast, keyboard, reflow, focus and target size.
8. [Microsoft Learn — Windows app title bar][8]. Native caption behaviour and layout guidance.
9. [Tauri — Window customization][9]. Title-bar implementation and drag-region constraints.

[1]: https://linear.app/now/behind-the-latest-design-refresh
[2]: https://linear.app/changelog/2026-06-18-agent-assisted-project-updates
[3]: https://linear.app/now/how-we-redesigned-the-linear-ui
[4]: https://linear.app/docs/display-options
[5]: https://linear.app/docs/select-issues
[6]: https://linear.app/changelog/2024-12-18-personalized-sidebar
[7]: https://www.w3.org/TR/WCAG22/
[8]: https://learn.microsoft.com/en-us/windows/apps/design/basics/titlebar-design
[9]: https://v2.tauri.app/learn/window-customization/
