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
