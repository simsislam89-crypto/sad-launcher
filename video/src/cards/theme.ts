import { loadFont as loadOnest } from "@remotion/google-fonts/Onest";
import { loadFont as loadMono } from "@remotion/google-fonts/JetBrainsMono";
import { Easing, getInputProps, interpolate } from "remotion";

// Pure black background: the site plays these with `mix-blend-mode: screen`,
// so black disappears into the glass card and only the ink stays.
export const BG = "#000000";
/** Scene background: transparent for the alpha WebM (`--props='{"alpha":true}'`),
    black for the MP4 fallback that the page blends with `screen`. */
export const sceneBg = () => (getInputProps().alpha ? "transparent" : BG);
/** Fill of chips that must hide lines behind them (alpha render too). */
export const CHIP = "#0d0d10";
export const INK = "#f1f1ee";
export const DIM = "rgba(241, 241, 238, 0.42)";
export const FAINT = "rgba(241, 241, 238, 0.14)";
export const GREEN = "#82e6a5";

export const { fontFamily: SANS } = loadOnest("normal", {
  weights: ["500", "700"],
  subsets: ["cyrillic", "latin"],
});
export const { fontFamily: MONO } = loadMono("normal", {
  weights: ["500"],
  subsets: ["cyrillic", "latin"],
});

export const OUT = Easing.bezier(0.16, 1, 0.3, 1);

/** 0→1 between two frames, eased, clamped. */
export const ramp = (frame: number, from: number, to: number) =>
  interpolate(frame, [from, to], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
    easing: OUT,
  });

/** Fade the whole scene in at the start and out at the end: the loop seam is black → black. */
export const loopFade = (frame: number, total: number, edge = 12) =>
  interpolate(frame, [0, edge, total - edge, total], [0, 1, 1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
