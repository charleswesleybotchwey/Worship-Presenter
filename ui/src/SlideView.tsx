import type { MediaItem, Slide } from "./api";
import { sectionName } from "./features/service/SlideList";
import { MediaView } from "./MediaView";
import { Crossfade, useMediaFadeMs } from "./fade";
import { useTheme } from "./theme/store";
import { ThemeView, type Phase } from "./theme/ThemeView";
import { useMediaUrl } from "./theme/media";

function PictureSlide({ path }: { path: string }) {
  const url = useMediaUrl();
  return (
    <div className="tv tv--media">
      <img className="tv__bg" src={url(`media:${path}`)} alt="" style={{ objectFit: "contain" }} />
    </div>
  );
}

// A slide drawn with the default theme for its kind, or a picture/video item.
// Thumbnails, Preview, Live and the outputs all go through here, so they always match.

export interface SlideLook {
  /** Lower-third theme (alternate output) instead of the full-screen one. */
  lowerThird?: boolean;
  /** Words only (a screen's look): black, no backgrounds, pictures or video. */
  wordsOnly?: boolean;
}

const BLACK = { kind: "color" as const, color: "#000000", image: null, fit: "cover" as const, dim: 0 };
const CLEAR = { ...BLACK, kind: "transparent" as const };

export function SlideView({
  sound,
  allSlides,
  slide,
  kind = "song",
  caption,
  media,
  look,
  blank,
  hideText,
  still,
  phase = "static",
  showKey,
  textKey,
  playing,
  restart,
  muted = true,
  themeId,
  alpha,
  position,
  adjust,
  copyright,
}: {
  /** May play a background video's sound (the main output). */
  sound?: boolean;
  /** The item's other slides (for "the same size on every slide"). */
  allSlides?: Slide[];
  /** The song's copyright line, for a theme's copyright layer. */
  copyright?: string | null;
  /** The Live pane's quick change for this item (words larger/smaller, up/down). */
  adjust?: { scale: number; shift: number };
  slide?: Slide;
  /** Whether this is its item's first / last slide (layers shown only there). */
  position?: { first: boolean; last: boolean };
  kind?: "song" | "scripture" | "media" | "announcement" | "header" | "web";
  caption?: string | null;
  media?: MediaItem | null;
  look?: SlideLook;
  blank?: boolean;
  hideText?: boolean;
  still?: boolean;
  phase?: Phase;
  showKey?: number;
  textKey?: string;
  playing?: boolean;
  restart?: number;
  muted?: boolean;
  /** A per-item look; the lower third always uses its own theme. */
  themeId?: number | null;
  /** Alpha channel: the theme's background is left out so the output is see-through. */
  alpha?: boolean;
}) {
  const fade = useMediaFadeMs();
  const base = useTheme(look?.lowerThird ? "lower_third" : kind === "scripture" ? "scripture" : kind === "announcement" ? "announcement" : "song", themeId);
  const plain = look?.wordsOnly ? { ...base, background: BLACK, layers: base.layers.filter((l) => l.kind !== "image") } : base;
  const theme = alpha ? { ...plain, background: CLEAR } : plain;

  if (blank || !slide || (look?.wordsOnly && (kind === "media" || slide.image))) {
    // Blank: black on screens, see-through on keyed outputs. No background video.
    return <ThemeView theme={{ ...theme, background: look?.lowerThird || alpha ? CLEAR : BLACK, layers: [] }} data={{ primary: [], second: [] }} />;
  }
  if (kind === "announcement") {
    // announcements are for the screens; the stream's lower third stays clear
    if (look?.lowerThird) return <ThemeView theme={{ ...theme, background: CLEAR, layers: [] }} data={{ primary: [], second: [] }} />;
    if (slide.image) return <PictureSlide path={slide.image} />;
    return <ThemeView theme={theme} data={{ primary: slide.primary, second: [], title: slide.label }} phase={phase} showKey={showKey} textKey={textKey} hideText={hideText} still={still} />;
  }
  if (kind === "media") {
    // Pictures and videos are for the screens, not the lower third.
    if (look?.lowerThird || !media) return <ThemeView theme={{ ...theme, background: CLEAR, layers: [] }} data={{ primary: [], second: [] }} />;
    return (
      <Crossfade k={`m${media.id}`} ms={still ? 0 : fade}>
        <MediaView item={media} playing={playing} restart={restart} muted={muted} still={still} />
      </Crossfade>
    );
  }
  const data = { sound, allPrimary: allSlides?.map((x) => x.primary), allSecond: allSlides?.map((x) => x.second), primary: slide.primary, second: slide.second, reference: kind === "scripture" ? (slide.reference != null ? slide.reference || null : caption) : null, title: kind === "song" ? caption : null, section: kind === "song" ? sectionName(slide.label) : null, first: position?.first, last: position?.last, textScale: adjust?.scale, textShift: adjust?.shift, rotateKey: caption ?? slide.label, copyright: kind === "song" ? copyright ?? null : null };
  return <ThemeView theme={theme} data={data} phase={phase} showKey={showKey} textKey={textKey} hideText={hideText} still={still} />;
}
