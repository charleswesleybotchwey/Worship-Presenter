import { useEffect, useRef, useState } from "react";
import { api, onCaption, onHeartbeat, onIdentify, onLive, onOutputConfigChanged, onSettingsChanged, type Broadcast, type OutputConfig } from "./api";
import { SlideView } from "./SlideView";
import { useThemes } from "./theme/store";
import { StageView } from "./StageView";
import { AlertOverlay, TimerOverlay } from "./Cues";

// An output window. It draws only what the core sends, ignores anything older than
// what it already shows, acknowledges every frame (a silent output is rebuilt), and
// switches to its fallback picture when the heartbeat stops.
//
// Going on air plays each layer's in-animation; going blank plays the out-animations
// on the last picture and then clears; a slide change cross-fades the text only.

const RULES: Record<string, { onLogo: "logo" | "blank" | "transparent"; lowerThird: boolean; keyed: boolean }> = {
  main: { onLogo: "logo", lowerThird: false, keyed: false },
  alternate: { onLogo: "transparent", lowerThird: true, keyed: true },
  foldback: { onLogo: "blank", lowerThird: false, keyed: false },
  // NDI lower third: drawn on the green key colour, captured and keyed to alpha.
  // Never a public output; its window lives off the visible desktop.
  ndi: { onLogo: "transparent", lowerThird: true, keyed: true },
};

export function Output({ role }: { role: string }) {
  const rule = RULES[role] ?? RULES.main;
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
  useEffect(() => {
    if (role === "ndi") return;
    const load = () => api.outputConfig(role).then(setCfg).catch(() => {});
    load();
    let stop = () => {};
    onOutputConfigChanged((r) => r === role && load()).then((u) => (stop = u));
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
  const showLogo = !lost && !isHeld && !onAir && mode === "logo" && (cfg ? cfg.on_logo === "logo" : rule.onLogo === "logo");
  // with the alpha channel on, nothing is drawn where the theme would show its background
  const seeThrough = !!cfg?.alpha || (!onAir && ((mode === "blank" && cfg?.on_blank === "transparent") || (mode === "logo" && cfg?.on_logo === "transparent")));
  useEffect(() => {
    // the window itself is see-through only when the alpha channel is on
    document.body.style.background = cfg?.alpha ? "transparent" : "";
  }, [cfg?.alpha]);
  const kindNow = shown?.item?.kind;
  const outputTheme = kindNow === "song" ? cfg?.song_theme_id : kindNow === "scripture" ? cfg?.scripture_theme_id : null;
  const m = cfg?.margins;
  const cam = role === "main" && !lost && !isHeld && (mode === "live" || mode === "clear") ? b?.state.camera ?? null : null;
  // full-screen camera hides the slide's words
  const camFull = cam?.mode === "full";
  const s = shown?.state;
  const slide = phase === "off" ? undefined : shown?.slides[s?.slide_idx ?? 0];

  if (role === "foldback") return <StageView b={b} lost={lost} />;

  return (
    <div className={`output ${rule.keyed || seeThrough ? "output--keyed" : ""}`} style={m ? { padding: `${m.top}px ${m.right}px ${m.bottom}px ${m.left}px` } : undefined}>
      <div className={`output__frame ${seeThrough ? "output__frame--alpha" : ""}`}>
        {cam && cam.mode !== "corner" && <CameraFeed cam={cam} />}
        <SlideView
          slide={slide}
          kind={shown?.item?.kind ?? "song"}
          caption={shown?.caption}
          media={shown?.media}
          themeId={(shown?.item && (shown.item.kind === "song" || shown.item.kind === "scripture") ? shown.item.theme_id : null) ?? outputTheme ?? null}
          playing={b?.state.media_playing}
          restart={b?.state.media_restart}
          muted={role !== "main"}
          look={{ lowerThird: rule.lowerThird }}
          blank={phase === "off"}
          hideText={b?.state.mode === "clear" || camFull}
          alpha={seeThrough || (!!cam && cam.mode !== "corner")}
          phase={lowSpec ? "static" : phase === "out" ? "out" : "in"}
          showKey={showKey}
          textKey={`${s?.slide_idx}-${s?.lines_per_slide}-${s?.lang_mode}-${shown?.slides.length}`}
        />
      </div>
      {showLogo && <div className="output__logo" aria-label="Logo" />}
      {identify && (
        <div className="identify" aria-label="Test pattern">
          <div className="identify__bars">
            {["#c0c0c0", "#c0c000", "#00c0c0", "#00c000", "#c000c0", "#c00000", "#0000c0"].map((c) => (
              <span key={c} style={{ background: c }} />
            ))}
          </div>
          <div className="identify__name">{role === "main" ? "Main" : role === "alternate" ? "Lower third" : "Stage"}</div>
          <div className="identify__size">
            {window.screen.width}×{window.screen.height}
          </div>
        </div>
      )}
      {!lost && !isHeld && (
        <div className={`cue-layer ${b?.state.alert?.position === "bottom" && (role === "main" ? b.state.alert.on_screens : b.state.alert.on_stream) ? "has-bottom-alert" : ""}`}>
          {role === "main" && b?.state.timer && b.state.timer.on_screens !== "off" && <TimerOverlay t={b.state.timer} full={b.state.timer.on_screens === "full"} />}
          {cam?.mode === "corner" && <CameraFeed cam={cam} corner />}
          {b?.state.subtitles && ((role === "main" && b.state.subtitles !== "stream") || ((role === "alternate" || role === "ndi") && b.state.subtitles !== "screens")) && <Subtitles />}
          {b?.state.alert && (role === "main" ? b.state.alert.on_screens : (role === "alternate" || role === "ndi") && b.state.alert.on_stream) && <AlertOverlay a={b.state.alert} />}
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
