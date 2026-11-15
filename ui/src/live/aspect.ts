import { useEffect } from "react";
import { api, onOutputConfigChanged } from "../api";

// Themes take the shape of the main output's screen: a 4:3 projector gets 4:3 slides
// (layers are placed in percentages, text sized to the height), not a letterboxed 16:9.
// Preview, Live, the theme editor and thumbnails use the same shape (--wp-aspect).

export function useMainAspect() {
  useEffect(() => {
    const apply = async () => {
      try {
        const c = await api.outputConfig("main");
        let w = 16;
        let h = 9;
        if (c.ndi) [w, h] = [c.ndi.width, c.ndi.height];
        else if (c.custom && c.custom.w > 0 && c.custom.h > 0) [w, h] = [c.custom.w, c.custom.h];
        else if (c.display_index != null) {
          const d = (await api.displays()).find((x) => x.index === c.display_index);
          if (d && d.width > 0 && d.height > 0) [w, h] = [d.width, d.height];
        }
        document.documentElement.style.setProperty("--wp-aspect", `${w} / ${h}`);
      } catch {
        /* 16:9 */
      }
    };
    void apply();
    let stop = () => {};
    onOutputConfigChanged(() => void apply()).then((u) => (stop = u));
    return () => stop();
  }, []);
}
