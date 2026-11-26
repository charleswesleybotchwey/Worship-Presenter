import { describe, expect, it } from "vitest";
import { isChord, pieces, transpose } from "../chords";
import { BRAND_DEFAULT, brandColor, isBrand, parseBrand } from "../theme/brand";
import { contrast } from "../theme/readability";
import { msUntil } from "../PreService";

describe("chords", () => {
  it("recognises and transposes chords", () => {
    expect(isChord("G")).toBe(true);
    expect(isChord("Amazing")).toBe(false);
    expect(transpose("G", 2)).toBe("A");
    expect(transpose("Bb", 2, true)).toBe("C");
  });
  it("places chords over the words", () => {
    const p = pieces("[G]O for a [C]thousand");
    expect(p.map((x) => x.chord)).toEqual(["G", "C"]);
    expect(p.map((x) => x.text).join("")).toBe("O for a thousand");
  });
});

describe("brand colours", () => {
  it("reads brand settings and resolves brand references", () => {
    const b = parseBrand({ brand_colors: JSON.stringify(["#112233", "#445566", "#ffffff", "#000000"]) });
    expect(b.colors[0]).toBe("#112233");
    expect(isBrand("brand:2")).toBe(true);
    expect(brandColor("brand:2", b)).toBe("#445566");
    expect(parseBrand({ brand_colors: "not json" }).colors).toEqual(BRAND_DEFAULT.colors);
    expect(brandColor("#abcdef", BRAND_DEFAULT)).toBe("#abcdef");
  });
});

describe("readability", () => {
  it("measures contrast like WCAG", () => {
    expect(contrast([255, 255, 255], [0, 0, 0])).toBeCloseTo(21, 0);
    expect(contrast([120, 120, 120], [128, 128, 128])).toBeLessThan(1.2);
  });
});

describe("pre-service loop", () => {
  it("counts down to today's start time", () => {
    const soon = new Date(Date.now() + 30 * 60_000);
    const hhmm = `${String(soon.getHours()).padStart(2, "0")}:${String(soon.getMinutes()).padStart(2, "0")}`;
    const ms = msUntil(hhmm);
    expect(ms).toBeGreaterThan(28 * 60_000);
    expect(ms).toBeLessThanOrEqual(30 * 60_000);
  });
});

import { capitalise } from "../theme/ThemeView";

describe("capitalisation (as drawn)", () => {
  it("capitalises all words, or the first word of each line", () => {
    expect(capitalise(["o for a thousand tongues", "my great redeemer's praise"], "words")).toEqual(["O For A Thousand Tongues", "My Great Redeemer's Praise"]);
    expect(capitalise(["o for a thousand tongues", "my great redeemer's praise"], "lines")).toEqual(["O for a thousand tongues", "My great redeemer's praise"]);
    expect(capitalise(["ɔdɔ wɔ hɔ"], "words")).toEqual(["Ɔdɔ Wɔ Hɔ"]);
    expect(capitalise(["as written"], "none")).toEqual(["as written"]);
  });
});

import { slideForVerse } from "../api";

describe("finding a verse in Preview", () => {
  it("finds a verse on its own slide or inside a slide of several", () => {
    const s = (label: string, a: number, b: number) => ({ label, occurrence: 0, first_key: a, last_key: b, primary: [], second: [] });
    const slides = [s("3:16", 3016, 3017), s("3:18", 3018, 3019)];
    expect(slideForVerse(slides, "3:18")).toBe(1);
    expect(slideForVerse(slides, "3:17")).toBe(0);
    expect(slideForVerse(slides, "3:19")).toBe(1);
    expect(slideForVerse(slides, "9:9")).toBe(0);
  });
});

import { capitaliseLines, dropBlankLines, tidySpaces, trimLineEnds } from "../songTidy";

describe("song Tidy tools", () => {
  const body = "[V1]\no for a thousand tongues,\n\n  my  great Redeemer's   praise;\n[C]\nɔdɔ wɔ hɔ!\n";
  it("capitalises lines (not section marks)", () => {
    expect(capitaliseLines(body)).toContain("O for a thousand tongues,");
    expect(capitaliseLines(body)).toContain("Ɔdɔ wɔ hɔ!");
    expect(capitaliseLines(body)).toContain("[V1]");
  });
  it("trims line ends but keeps ! and ?", () => {
    expect(trimLineEnds("a line,\nanother;\nwhy?\npraise!")).toBe("a line\nanother\nwhy?\npraise!");
  });
  it("tidies spaces and blank lines", () => {
    expect(tidySpaces(body)).toContain("my great Redeemer's praise;");
    expect(dropBlankLines("[V1]\none\n\ntwo\n\n\n[C]\nthree\n")).toBe("[V1]\none\ntwo\n\n[C]\nthree");
  });
});

import { parseRich, stripRich } from "../rich";

describe("rich text in lyrics", () => {
  it("reads bold, italic, underline and colour", () => {
    expect(parseRich("Praise **Jesus** now")).toEqual([{ text: "Praise " }, { text: "Jesus", b: true }, { text: " now" }]);
    expect(parseRich("{gold}Hal*le*lujah{/}!")).toEqual([{ text: "Hal", color: "gold" }, { text: "le", i: true, color: "gold" }, { text: "lujah", color: "gold" }, { text: "!" }]);
    expect(parseRich("__Amen__")).toEqual([{ text: "Amen", u: true }]);
  });
  it("gives the plain words everywhere else", () => {
    expect(stripRich("Praise **Jesus**, {#ffcc00}King{/} of *kings*")).toBe("Praise Jesus, King of kings");
    expect(stripRich("no markers {here}")).toBe("no markers {here}");
  });
});

import { PAIRED, pairLines } from "../theme/ThemeView";

describe("two languages line by line", () => {
  it("puts each translation under its line", () => {
    expect(pairLines(["O for a thousand", "My great Redeemer"], ["Meenya ngyiramá", "Me Pomfo"])).toEqual(["O for a thousand", PAIRED + "Meenya ngyiramá", "My great Redeemer", PAIRED + "Me Pomfo"]);
    expect(pairLines(["one", "two", "three"], ["baako"])).toEqual(["one", PAIRED + "baako", "two", "three"]);
  });
});

import { blendAlpha, mapPoint, pinMatrix, pinTransform } from "../live/geometry";

describe("screen geometry", () => {
  it("corner pin moves each corner where it is told and keeps straight lines", () => {
    const pin = [5, 2, 3, 0, 0, 4, 2, 1];
    const m = pinMatrix(pin, 1920, 1080)!;
    const near = (p: number[], q: number[]) => Math.hypot(p[0] - q[0], p[1] - q[1]) < 1e-6;
    expect(near(mapPoint(m, [0, 0]), [96, 21.6])).toBe(true);
    expect(near(mapPoint(m, [1920, 0]), [1920 - 57.6, 0])).toBe(true);
    expect(near(mapPoint(m, [1920, 1080]), [1920, 1080 - 43.2])).toBe(true);
    expect(near(mapPoint(m, [0, 1080]), [38.4, 1080 - 10.8])).toBe(true);
    // the middle of the top edge stays on the line between the two top corners
    const [a, b, c] = [mapPoint(m, [0, 0]), mapPoint(m, [960, 0]), mapPoint(m, [1920, 0])];
    expect(Math.abs((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))).toBeLessThan(1e-6);
    expect(pinTransform([0, 0, 0, 0, 0, 0, 0, 0], 1920, 1080)).toBeNull();
    expect(pinTransform(pin, 1920, 1080)).toMatch(/^matrix3d\(/);
  });
  it("soft edges of two projectors add up to even brightness", () => {
    const g = 2.2;
    for (const t of [0, 0.25, 0.5, 0.75, 1]) {
      // what each projector puts out, back in real brightness, across the overlap
      const left = Math.pow(1 - blendAlpha(t, g), g);
      const right = Math.pow(1 - blendAlpha(1 - t, g), g);
      expect(left + right).toBeCloseTo(1, 6);
    }
  });
});

import { chorusAfterEach, groupColor, parseOrder, sectionsOf } from "../groups";

describe("slide groups and arrangements", () => {
  it("colours groups by kind", () => {
    expect(groupColor("V1")).toBe(groupColor("V3"));
    expect(groupColor("C")).not.toBe(groupColor("V1"));
    expect(groupColor("Ending")).toBe("#8e8a83");
  });
  it("reads a song's sections and arranges them", () => {
    const body = "[V1]\na\n[C]\nb\n[V2]\nc\n[B]\nd\n[V3]\ne";
    expect(sectionsOf(body)).toEqual(["V1", "C", "V2", "B", "V3"]);
    expect(chorusAfterEach(sectionsOf(body))).toEqual(["V1", "C", "V2", "C", "B", "C", "V3", "C"]);
    expect(chorusAfterEach(["V1", "V2"])).toEqual(["V1", "V2"]);
    expect(parseOrder(" V1  C V2 ")).toEqual(["V1", "C", "V2"]);
  });
});

import { responsive } from "../rich";

describe("responsive readings", () => {
  it("hides the prefix and knows who reads", () => {
    expect(responsive("Leader: The earth is the Lord's")).toEqual({ who: "leader", text: "The earth is the Lord's" });
    expect(responsive("P: and the fulness thereof")).toEqual({ who: "people", text: "and the fulness thereof" });
    expect(responsive("All: Amen.")).toEqual({ who: "people", text: "Amen." });
    expect(responsive("Psalm 24: a psalm of David")).toEqual({ who: null, text: "Psalm 24: a psalm of David" });
  });
});

import { cleanReading, readingParts } from "../LectionaryPanel";

describe("lectionary readings as references", () => {
  it("tidies a reading for the reference reader", () => {
    expect(cleanReading("Isaiah 64:1-64:9")).toBe("Isaiah 64:1-9");
    expect(cleanReading("Psalm 80:1-80:7, 80:16-80:18")).toBe("Psalm 80:1-7, 16-18");
    expect(cleanReading("Psalm 126 or Canticle 15")).toBe("Psalm 126");
    expect(cleanReading("Luke 2:1-2:14, (2:15-2:20)")).toBe("Luke 2:1-14");
    expect(cleanReading("2 Peter 3:8-3:15a")).toBe("2 Peter 3:8-15");
    expect(cleanReading("Isaiah 61:10-62:3")).toBe("Isaiah 61:10-62:3");
  });
  it("splits a reading with several ranges into references the app reads", () => {
    expect(readingParts("Psalm 80:1-80:7, 80:16-80:18")).toEqual(["Psalm 80:1-7", "Psalm 80:16-18"]);
    expect(readingParts("Galatians 3:23-3:25, 4:4-4:7")).toEqual(["Galatians 3:23-25", "Galatians 4:4-7"]);
    expect(readingParts("1 Corinthians 1:3-9")).toEqual(["1 Corinthians 1:3-9"]);
  });
});

import { stanzas } from "../features/service/TuneBar";

describe("hymn tunes: verses as runs of slides", () => {
  const s = (labels: string[]) => labels.map((label, i) => ({ label, occurrence: 0, i }));
  it("each verse with its refrain, a title slide left out", () => {
    expect(stanzas(s(["Title", "V1", "V1", "C", "V2", "V2", "C"]))).toEqual([
      { start: 1, end: 3 },
      { start: 4, end: 6 },
    ]);
  });
  it("a song of choruses only sings each section to the tune", () => {
    expect(stanzas(s(["C", "C", "B"]))).toEqual([
      { start: 0, end: 1 },
      { start: 2, end: 2 },
    ]);
  });
});
