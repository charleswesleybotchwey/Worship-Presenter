import { defaultLook } from "./live/outputs";
import { PropsLayer } from "./PropsLayer";
import { Crossfade, useMediaFadeMs } from "./fade";
import { blendGradient, pinTransform } from "./live/geometry";
import type { StageLayout } from "./StageView";
import { useEffect, useRef, useState } from "react";
import { api, onCaption, onHeartbeat, onIdentify, onLive, onOutputConfigChanged, onSettingsChanged, type Broadcast, type MediaItem, type OutputConfig } from "./api";
import { SlideView } from "./SlideView";
import { MediaView } from "./MediaView";
import { NdiFeed } from "./NdiFeed";
import { useThemes } from "./theme/store";
import { StageView } from "./StageView";
import { AlertOverlay, TimerOverlay } from "./Cues";

// An output window. It draws only what the core sends, ignores anything older than
// what it already shows, acknowledges every frame (a silent output is rebuilt), and
// switches to its fallback picture when the heartbeat stops.
//
// Going on air plays each layer's in-animation; going blank plays the out-animations
// on the last picture and then clears; a slide change cross-fades the text only.

type Rule = { onLogo: "logo" | "blank" | "transparent"; lowerThird: boolean; keyed: boolean };
/** What each look of a screen shows (as ProPresenter's looks). */
const LOOK_RULES: Record<string, Rule> = {
  full: { onLogo: "logo", lowerThird: false, keyed: false },
  lower: { onLogo: "transparent", lowerThird: true, keyed: true },
  stage: { onLogo: "blank", lowerThird: false, keyed: false },
  board: { onLogo: "blank", lowerThird: false, keyed: false },
  words: { onLogo: "blank", lowerThird: false, keyed: false },
  media: { onLogo: "logo", lowerThird: false, keyed: false },
};
const RULES: Record<string, Rule> = {
  main: { onLogo: "logo", lowerThird: false, keyed: false },
  alternate: { onLogo: "transparent", lowerThird: true, keyed: true },
  foldback: { onLogo: "blank", lowerThird: false, keyed: false },
  // NDI lower third: drawn on the green key colour, captured and keyed to alpha.
  // Never a public output; its window lives off the visible desktop.
  ndi: { onLogo: "transparent", lowerThird: true, keyed: true },
};

export function Output({ role, keyed = false }: { role: string; keyed?: boolean }) {
  const fade = useMediaFadeMs();
  const [win, setWin] = useState({ w: window.innerWidth, h: window.innerHeight });
  useEffect(() => {
    const on = () => setWin({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, []);
  const themes = useThemes();
  const [b, setB] = useState<Broadcast | null>(null);
  const [lost, setLost] = useState(false);
  const [phase, setPhase] = useState<"in" | "out" | "off">("off");
  const [shown, setShown] = useState<Broadcast | null>(null); // what is drawn, held through the out-animation
  const [showKey, setShowKey] = useState(0);
  const rev = useRef(0);
  const beat = useRef(Date.now());
  const lastItem = useRef<string>("");
  const [lowSpec, setLowSpec] = useState(false);
  // this output's own options (EasyWorship's Options dialog)
  const [cfg, setCfg] = useState<OutputConfig | null>(null);
  // the NDI lower third follows the alternate output's options (its theme and so on)
  const cfgRole = role === "ndi" ? "alternate" : role;
  // the screen's look (as ProPresenter): full, lower third, stage, words only, media only
  const look = role === "ndi" ? "lower" : cfg?.look ?? defaultLook(role);
  const rule = LOOK_RULES[look] ?? RULES.main;
  const audience = look === "full" || look === "words" || look === "media";
  useEffect(() => {
    const load = () => api.outputConfig(cfgRole).then(setCfg).catch(() => {});
    load();
    let stop = () => {};
    onOutputConfigChanged((r) => r === cfgRole && load()).then((u) => (stop = u));
    return () => stop();
  }, [role]);
  const [identify, setIdentify] = useState(false);

  // low-spec projection: cuts instead of animations (core setting, brief §4)
  useEffect(() => {
    const load = () => api.getSettings().then((x) => setLowSpec(x.low_spec_mode === "true")).catch(() => {});
    load();
    let a = () => {},
      c = () => {};
    onSettingsChanged(load).then((u) => (a = u));
    // explicit identify from Output setup: a test pattern for a few seconds
    onIdentify((r) => {
      if (r !== role) return;
      setIdentify(true);
      setTimeout(() => setIdentify(false), 4000);
    }).then((u) => (c = u));
    return () => (a(), c());
  }, [role]);

  useEffect(() => {
    document.body.classList.add("output-body", `output-body--${role}`);
    let stop1 = () => {},
      stop2 = () => {};
    onLive((x) => {
      if (x.state.revision < rev.current) return; // stale
      rev.current = x.state.revision;
      beat.current = Date.now();
      setLost(false);
      setB(x);
      api.outputAck(role, x.state.revision).catch(() => {});
    }).then((u) => (stop1 = u));
    onHeartbeat(() => {
      beat.current = Date.now();
      setLost(false);
      api.outputAck(role, rev.current).catch(() => {});
    }).then((u) => (stop2 = u));
    api.resync().catch(() => {});
    const t = setInterval(() => setLost(Date.now() - beat.current > 3000), 500);
    return () => (stop1(), stop2(), clearInterval(t));
  }, [role]);

  // a display that came back after being lost stays black until the operator resumes it
  const isHeld = !!b?.held?.includes(role);
  // "ignore": this output keeps showing the slide when Blank or Logo is pressed
  const ignores = (b?.state.mode === "blank" && cfg?.on_blank === "ignore") || (b?.state.mode === "logo" && cfg?.on_logo === "ignore");
  const onAir = !!b && !lost && !isHeld && (b.state.mode === "live" || b.state.mode === "clear" || ignores) && b.slides.length > 0;

  useEffect(() => {
    if (onAir) {
      const itemKey = JSON.stringify(b!.item);
      if (phase !== "in" || itemKey !== lastItem.current) setShowKey((k) => k + 1); // play in-animations
      lastItem.current = itemKey;
      setShown(b);
      setPhase("in");
    } else if (phase === "in") {
      setPhase("out");
      const k = shown?.item?.kind;
      const theme = rule.lowerThird ? themes.lower_third : k === "scripture" ? themes.scripture : k === "announcement" ? themes.announcement : themes.song;
      const longest = shown?.item?.kind === "media" || lowSpec ? 0 : Math.max(0, ...theme.layers.map((l) => (l.animOut.kind === "none" ? 0 : l.animOut.ms + l.animOut.delay)));
      const t = setTimeout(() => setPhase("off"), longest + 30);
      return () => clearTimeout(t);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onAir, b]);

  const mode = b?.state.mode ?? "blank";
  const logoMedia = useLogoMedia();
  const showLogo = !lost && !isHeld && !onAir && mode === "logo" && (cfg ? cfg.on_logo === "logo" : rule.onLogo === "logo");
  // with the alpha channel on, nothing is drawn where the theme would show its background
  const seeThrough = !!cfg?.alpha || (!onAir && ((mode === "blank" && cfg?.on_blank === "transparent") || (mode === "logo" && cfg?.on_logo === "transparent")));
  useEffect(() => {
    // the window itself is see-through only when the alpha channel is on
    document.body.style.background = cfg?.alpha ? "transparent" : "";
  }, [cfg?.alpha]);
  const kindNow = shown?.item?.kind;
  const outputTheme = kindNow === "song" ? cfg?.song_theme_id : kindNow === "scripture" ? cfg?.scripture_theme_id : null;
  // margins correct a physical screen's edges: not for an NDI picture
  const m = role === "ndi" || keyed ? undefined : cfg?.margins;
  const cam = role === "main" && !lost && !isHeld && (mode === "live" || mode === "clear") ? b?.state.camera ?? null : null;
  // full-screen camera hides the slide's words
  const camFull = cam?.mode === "full";
  const s = shown?.state;
  const slide = phase === "off" ? undefined : shown?.slides[s?.slide_idx ?? 0];

  // corner pin (keystone) over the whole window, and this screen's part of a wall
  const pinCss = pinTransform(cfg?.pin, win.w, win.h);
  const pinStyle: React.CSSProperties = pinCss ? { transform: pinCss, transformOrigin: "0 0" } : {};
  const r = cfg?.region;
  const regionStyle: React.CSSProperties | undefined =
    r && (r.w < 100 || r.h < 100) ? { position: "absolute", left: `${(-r.x / r.w) * 100}%`, top: `${(-r.y / r.h) * 100}%`, width: `${(100 / r.w) * 100}%`, height: `${(100 / r.h) * 100}%` } : undefined;
  const blend = cfg?.blend;
  // a stage screen's own layout (else the shared one from Settings → Stage display)
  const ownStage = (() => {
    try {
      return cfg?.stage_layout ? (JSON.parse(cfg.stage_layout) as StageLayout) : undefined;
    } catch {
      return undefined;
    }
  })();
  if (look === "board")
    return (
      <div className="output output--stage" style={pinStyle}>
        <HymnBoard revision={b?.state.revision ?? 0} itemKey={JSON.stringify(b?.item ?? null)} />
      </div>
    );
  if (look === "stage")
    return (
      <div className="output output--stage" style={pinStyle}>
        <StageView b={b} lost={lost} layout={ownStage} />
        {cfg?.grid && <AlignGrid />}
      </div>
    );

  return (
    <div className={`output ${rule.keyed || seeThrough ? "output--keyed" : ""} ${cfg?.large_print ? "output--large" : ""}`} style={{ ...(m ? { padding: `${m.top}px ${m.right}px ${m.bottom}px ${m.left}px` } : {}), ...pinStyle }}>
      <div className={`output__frame ${seeThrough ? "output__frame--alpha" : ""}`}>
        {/* a projector wall: this screen shows its part of the whole picture */}
        <div className="output__region" style={regionStyle}>
        {cam && cam.mode !== "corner" && <CameraFeed cam={cam} />}
        {/* a media item and the slides crossfade into each other (as ProPresenter) */}
        <Crossfade k={shown?.item?.kind === "media" ? `media:${shown.item.media_id}` : "slides"} ms={lowSpec ? 0 : fade}>
        <SlideView
          slide={slide}
          allSlides={shown?.slides}
          sound={role === "main"}
          adjust={{ scale: (shown?.state.text_scale ?? 1) * (cfg?.large_print ? 1.35 : 1), shift: shown?.state.text_shift ?? 0 }}
          copyright={shown?.song_copyright}
          position={{ first: (shown?.state.slide_idx ?? 0) === 0, last: (shown?.state.slide_idx ?? 0) >= (shown?.slides.length ?? 1) - 1 }}
          kind={shown?.item?.kind ?? "song"}
          caption={shown?.caption}
          media={shown?.media}
          themeId={(shown?.item && (shown.item.kind === "song" || shown.item.kind === "scripture") ? shown.item.theme_id : null) ?? outputTheme ?? shown?.rule_theme_id ?? null}
          playing={b?.state.media_playing}
          restart={b?.state.media_restart}
          muted={role !== "main"}
          look={{ lowerThird: rule.lowerThird, wordsOnly: look === "words" || !!b?.state.media_off || !!cfg?.large_print }}
          blank={phase === "off"}
          hideText={b?.state.mode === "clear" || camFull || look === "media"}
          alpha={seeThrough || (!!cam && cam.mode !== "corner")}
          phase={lowSpec ? "static" : phase === "out" ? "out" : "in"}
          showKey={showKey}
          textKey={`${s?.slide_idx}-${s?.lines_per_slide}-${s?.lang_mode}-${shown?.slides.length}`}
        />
        </Crossfade>
        </div>
        {/* props: graphics over the slides, independently of them */}
        {!!b?.state.props?.length && <PropsLayer showing={b.state.props} stream={look === "lower"} />}
        {/* the soft edges where this projector overlaps its neighbours */}
        {blend && blend.left > 0 && <div className="output__blend" style={{ left: 0, top: 0, bottom: 0, width: `${blend.left}%`, background: blendGradient("left", blend.gamma) }} />}
        {blend && blend.right > 0 && <div className="output__blend" style={{ right: 0, top: 0, bottom: 0, width: `${blend.right}%`, background: blendGradient("right", blend.gamma) }} />}
        {blend && blend.top > 0 && <div className="output__blend" style={{ left: 0, right: 0, top: 0, height: `${blend.top}%`, background: blendGradient("top", blend.gamma) }} />}
        {blend && blend.bottom > 0 && <div className="output__blend" style={{ left: 0, right: 0, bottom: 0, height: `${blend.bottom}%`, background: blendGradient("bottom", blend.gamma) }} />}
        {cfg?.grid && <AlignGrid />}
      </div>
      {showLogo && (
        <div className="output__logo" aria-label="Logo">
          {logoMedia && <MediaView item={{ ...logoMedia, repeat: "loop", fit: logoMedia.fit ?? "fill" }} muted />}
        </div>
      )}
      {identify && (
        <div className="identify" aria-label="Test pattern">
          <div className="identify__bars">
            {["#c0c0c0", "#c0c000", "#00c0c0", "#00c000", "#c000c0", "#c00000", "#0000c0"].map((c) => (
              <span key={c} style={{ background: c }} />
            ))}
          </div>
          <div className="identify__name">{cfg?.name ?? (role === "main" ? "Main" : role === "alternate" ? "Lower third" : "Stage")}</div>
          <div className="identify__size">
            {window.screen.width}×{window.screen.height}
          </div>
        </div>
      )}
      {!lost && !isHeld && (
        <div className={`cue-layer ${b?.state.alert?.position === "bottom" && (role === "main" ? b.state.alert.on_screens : b.state.alert.on_stream) ? "has-bottom-alert" : ""}`}>
          {role === "main" && b?.state.timer && b.state.timer.on_screens !== "off" && <TimerOverlay t={b.state.timer} full={b.state.timer.on_screens === "full"} />}
          {cam?.mode === "corner" && <CameraFeed cam={cam} corner />}
          {b?.state.subtitles && ((audience && b.state.subtitles !== "stream") || (look === "lower" && b.state.subtitles !== "screens")) && <Subtitles />}
          {b?.state.alert && (audience ? b.state.alert.on_screens : look === "lower" && b.state.alert.on_stream) && <AlertOverlay a={b.state.alert} />}
        </div>
      )}
    </div>
  );
}

/** Live sermon subtitles: the last finished sentence and what is being said now, two
 *  lines at most; they fade when the preacher pauses. */
function Subtitles() {
  const [done, setDone] = useState("");
  const [now, setNow] = useState("");
  const [at, setAt] = useState(0);
  const [, tick] = useState(0);
  useEffect(() => {
    let stop = () => {};
    onCaption((c) => {
      setAt(Date.now());
      if (c.final) (setDone(c.text), setNow(""));
      else setNow(c.text);
    }).then((u) => (stop = u));
    const t = setInterval(() => tick((x) => x + 1), 1000);
    return () => (stop(), clearInterval(t));
  }, []);
  const quiet = Date.now() - at > 6000;
  const text = (now || done).split(" ").slice(-22).join(" ");
  if (!text) return null;
  return (
    <div className={`subtitles ${quiet ? "subtitles--quiet" : ""}`} aria-live="off">
      <span>{text}</span>
    </div>
  );
}

/** A live camera (USB, or a capture card that Windows shows as a camera). */
function CameraFeed({ cam, corner }: { cam: { device_id: string; label: string; mode: string }; corner?: boolean }) {
  if (cam.device_id.startsWith("ndi:")) return <NdiFeed id={cam.device_id} className={corner ? "camfeed camfeed--corner" : "camfeed"} />;
  return <DeviceCamera cam={cam} corner={corner} />;
}

function DeviceCamera({ cam, corner }: { cam: { device_id: string; label: string; mode: string }; corner?: boolean }) {
  const ref = useRef<HTMLVideoElement>(null);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    let stream: MediaStream | null = null;
    let stop = false;
    // camera ids can differ between windows: try the id, then the same name, then the default camera
    const open = (deviceId?: string) => navigator.mediaDevices.getUserMedia({ video: { deviceId: deviceId ? { exact: deviceId } : undefined, width: { ideal: 1920 }, height: { ideal: 1080 } }, audio: false });
    const byName = async () => {
      const list = await navigator.mediaDevices.enumerateDevices();
      const same = list.find((d) => d.kind === "videoinput" && d.label === cam.label);
      return open(same?.deviceId);
    };
    (async () => {
      try {
        let s: MediaStream;
        try {
          s = await open(cam.device_id || undefined);
        } catch {
          s = await byName();
        }
        if (stop) return s.getTracks().forEach((t) => t.stop());
        stream = s;
        if (ref.current) ref.current.srcObject = s;
        setProblem(null);
      } catch {
        setProblem(`${cam.label}: not available`);
      }
    })();
    return () => {
      stop = true;
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, [cam.device_id, cam.label]);
  return (
    <div className={corner ? "camfeed camfeed--corner" : "camfeed"}>
      <video ref={ref} autoPlay muted playsInline />
      {problem && <span className="camfeed__problem">{problem}</span>}
    </div>
  );
}

/** The picture or video chosen with "Use As Logo Background" (none: the plain logo). */
function useLogoMedia(): MediaItem | null {
  const [m, setM] = useState<MediaItem | null>(null);
  useEffect(() => {
    const load = () =>
      api
        .getSettings()
        .then(async (x) => {
          const id = Number(x.logo_media_id || 0);
          if (!id) return setM(null);
          const all = await api.listMedia(null, "");
          setM(all.find((i) => i.id === id && !i.missing) ?? null);
        })
        .catch(() => setM(null));
    load();
    let stop = () => {};
    onSettingsChanged(load).then((u) => (stop = u));
    return () => stop();
  }, []);
  return m;
}

/** A grid over the picture for lining up projectors: tenths, the centre, the edges. */
function AlignGrid() {
  return (
    <svg className="output__grid" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
      {Array.from({ length: 9 }, (_, i) => (i + 1) * 10).map((v) => (
        <g key={v}>
          <line x1={v} y1={0} x2={v} y2={100} />
          <line x1={0} y1={v} x2={100} y2={v} />
        </g>
      ))}
      <rect x={0.2} y={0.2} width={99.6} height={99.6} className="output__grid-edge" />
      <line x1={0} y1={0} x2={100} y2={100} className="output__grid-x" />
      <line x1={100} y1={0} x2={0} y2={100} className="output__grid-x" />
      <circle cx={50} cy={50} r={8} className="output__grid-edge" />
    </svg>
  );
}

/** The hymn board (like the boards on church walls): the day's hymn numbers, the one being
 *  sung lit. Asks again when the live item changes, and every few seconds for edits. */
function HymnBoard({ revision, itemKey }: { revision: number; itemKey: string }) {
  const [rows, setRows] = useState<{ number: string; title: string; live: boolean }[]>([]);
  useEffect(() => {
    const load = () => api.hymnBoard().then(setRows).catch(() => {});
    load();
    const t = setInterval(load, 4000);
    return () => clearInterval(t);
  }, [revision, itemKey]);
  return (
    <div className="hymn-board">
      <div className="hymn-board__head">Hymns</div>
      {rows.length === 0 && <div className="hymn-board__empty">No hymns in this service yet</div>}
      <ol className="hymn-board__list">
        {rows.map((r, i) => (
          <li key={i} className={r.live ? "is-live" : ""}>
            <span className="hymn-board__num">{r.number || r.title}</span>
            {r.number && <span className="hymn-board__title">{titleAfter(r.title, r.number)}</span>}
          </li>
        ))}
      </ol>
    </div>
  );
}

/** "MHB 1, ENG&FANTE" after its number "MHB 1" → "ENG&FANTE" (the number is shown large). */
function titleAfter(title: string, number: string): string {
  return title.toLowerCase().startsWith(number.toLowerCase()) ? title.slice(number.length).replace(/^[\s,:.-]+/, "") : title;
}
