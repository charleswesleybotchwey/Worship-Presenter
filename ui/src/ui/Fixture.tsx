import { Icon } from "../Icon";

// Control fixture (design brief §7 step 1): every control in default, hover, focus,
// selected, disabled and fault states, in both themes. Open #/fixture. Used for
// visual review and the screenshot baselines; never shown to operators.

function Panel({ theme }: { theme: "dark" | "light" }) {
  return (
    <section className="wp-control fixture__col" data-theme={theme} aria-label={`${theme} theme`}>
      <h2 className="fixture__h">{theme === "dark" ? "Dark" : "Light"}</h2>

      <h3 className="fixture__k">Buttons</h3>
      <div className="fixture__row">
        <button className="wp-btn">Default</button>
        <button className="wp-btn is-hover">Hover</button>
        <button className="wp-btn is-focus">Focus</button>
        <button className="wp-btn is-pressed" aria-pressed="true">
          <Icon name="check" /> Pressed
        </button>
        <button className="wp-btn" disabled>
          Disabled
        </button>
      </div>
      <div className="fixture__row">
        <button className="wp-btn wp-btn--quiet">Quiet</button>
        <button className="wp-btn wp-btn--quiet is-hover">Quiet hover</button>
        <button className="wp-btn wp-btn--primary">
          Go live <kbd>F5</kbd>
        </button>
        <button className="wp-btn wp-btn--primary is-focus">
          Go live <kbd>F5</kbd>
        </button>
        <button className="wp-btn wp-btn--primary" disabled>
          Go live <kbd>F5</kbd>
        </button>
      </div>
      <div className="fixture__row">
        <button className="wp-btn">
          <Icon name="blank" /> Blank
        </button>
        <button className="wp-btn is-pressed" aria-pressed="true">
          <Icon name="check" /> Logo
        </button>
        <button className="wp-btn wp-btn--resume">Resume output</button>
        <button className="wp-btn wp-btn--quiet is-on">
          <Icon name="timer" /> 4:59
        </button>
        <button className="wp-btn wp-btn--quiet is-over">
          <Icon name="timer" /> +0:12
        </button>
        <button className="wp-icon-btn" aria-label="More">
          <Icon name="more" />
        </button>
      </div>

      <h3 className="fixture__k">Tabs and chips</h3>
      <div className="fixture__row">
        <div className="wp-tabs" role="tablist">
          <button role="tab" aria-selected="true">
            Songs
          </button>
          <button role="tab" aria-selected="false">
            Scripture
          </button>
          <button role="tab" aria-selected="false" className="is-focus">
            Media
          </button>
        </div>
        <button className="wp-chip">V1</button>
        <button className="wp-chip is-selected">C</button>
        <button className="wp-chip">V2</button>
      </div>

      <h3 className="fixture__k">Fields</h3>
      <div className="fixture__row">
        <label className="wp-search">
          <Icon name="search" />
          <input placeholder="Search songs" />
        </label>
        <label className="wp-search is-focus-within">
          <Icon name="book" />
          <input defaultValue="Yohane 3:16" />
        </label>
      </div>

      <h3 className="fixture__k">Rows</h3>
      <ul className="wp-rows fixture__rows">
        <li className="wp-row">
          <span className="wp-row__num">01</span>
          <Icon name="note" />
          <span className="wp-row__title">Default row: MHB 50</span>
        </li>
        <li className="wp-row is-hover">
          <span className="wp-row__num">02</span>
          <Icon name="note" />
          <span className="wp-row__title">Hover: PAN 123</span>
        </li>
        <li className="wp-row is-selected" aria-selected="true">
          <span className="wp-row__num">03</span>
          <Icon name="book" />
          <span className="wp-row__title">Selected (staged): Yohane 3:16 · TK</span>
        </li>
        <li className="wp-row is-focus" tabIndex={0}>
          <span className="wp-row__num">04</span>
          <Icon name="note" />
          <span className="wp-row__title">Focused: Ɛ ɛ Ɔ ɔ Ŋ ŋ Ɔ́ a very long title that must ellipsise and not wrap</span>
          <span className="wp-live-tag">
            <Icon name="speaker" /> Live
          </span>
        </li>
        <li className="wp-row">
          <Icon name="video" />
          <span className="wp-row__title">Missing media</span>
          <span className="wp-row__warn">File missing</span>
          <button className="wp-btn wp-btn--quiet">Relink…</button>
        </li>
      </ul>

      <h3 className="fixture__k">Output states</h3>
      <div className="fixture__col2">
        {[
          ["on", "Live · Main"],
          ["idle", "Main: blank"],
          ["sending", "Main: sending…"],
          ["unconfirmed", "Main: not confirmed"],
          ["disconnected", "Main: disconnected"],
          ["rehearsal", "Rehearsal · not on the screens"],
        ].map(([c, t]) => (
          <span key={c} className={`wp-live-state wp-live-state--${c}`}>
            <span className="wp-dot" aria-hidden="true" />
            {t}
          </span>
        ))}
      </div>

      <h3 className="fixture__k">Banners</h3>
      <div className="wp-banner">
        <span>Service saved.</span>
      </div>
      <div className="wp-banner wp-banner--warn">
        <span>Main came back after being lost and is showing black.</span>
        <button className="wp-btn">Resume Main</button>
      </div>
      <div className="wp-banner wp-banner--bad">
        <span>Output is held. Choose Resume output first.</span>
      </div>

      <h3 className="fixture__k">Menu and palette row</h3>
      <div className="fixture__row">
        <div className="wp-menu fixture__static" role="menu">
          <button role="menuitem">Preview</button>
          <button role="menuitem" className="is-focus">
            Add to the service
          </button>
          <button role="menuitem">Edit song</button>
        </div>
        <div className="palette fixture__static">
          <div className="palette__row is-selected">
            <span>Search songs</span>
            <kbd>Ctrl+F</kbd>
          </div>
          <div className="palette__row">
            <span>Quick scripture</span>
            <kbd>Ctrl+G</kbd>
          </div>
        </div>
      </div>

      <h3 className="fixture__k">Status strip</h3>
      <footer className="wp-status fixture__status">
        <span className="wp-status__out wp-status__out--confirmed">Live · Main</span>
        <span className="wp-status__out wp-status__out--disconnected">Lower third: disconnected</span>
        <span className="wp-num">Slide 3 of 8</span>
        <span className="wp-spacer" />
        <span>Backup: 2026-11-01 09:00</span>
        <span className="wp-num">09:41</span>
      </footer>
    </section>
  );
}

export function Fixture() {
  return (
    <div className="fixture">
      <Panel theme="dark" />
      <Panel theme="light" />
    </div>
  );
}
