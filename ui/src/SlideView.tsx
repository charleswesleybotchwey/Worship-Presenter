import type { Slide } from "./api";

// One renderer for thumbnails, Preview, Live and the output windows, so what the
// operator sees in a thumbnail is what the projector draws. Sizes are in container
// query units, so the same slide scales from a 160px thumbnail to a 4K wall.

export interface SlideLook {
  /** Lower-third band instead of full-screen text. */
  lowerThird?: boolean;
  /** Transparent background, for keyed outputs. */
  transparent?: boolean;
}

export function SlideView({ slide, caption, look, blank }: { slide?: Slide; caption?: string | null; look?: SlideLook; blank?: boolean }) {
  const cls = ["slide", look?.lowerThird ? "slide--lower" : "", look?.transparent ? "slide--keyed" : ""].join(" ");
  return (
    <div className={cls}>
      {!blank && slide && (
        <div className="slide__text">
          <div className="slide__primary">
            {slide.primary.map((l, i) => (
              <p key={i}>{l}</p>
            ))}
          </div>
          {slide.second.length > 0 && (
            <div className="slide__second" lang="fat">
              {slide.second.map((l, i) => (
                <p key={i}>{l}</p>
              ))}
            </div>
          )}
          {caption && <div className="slide__caption">{caption}</div>}
        </div>
      )}
    </div>
  );
}
