import { AbsoluteFill, interpolate, useCurrentFrame, useVideoConfig } from "remotion";
import { sceneBg, FAINT, GREEN, INK, DIM, SANS, MONO, ramp, loopFade } from "./theme";

// Typed in the wrong keyboard layout, understood anyway (smart.rs).
const TYPED = "зкщьштутсу";
const FOUND = "Prominence";

export const Search = () => {
  const frame = useCurrentFrame();
  const { durationInFrames: total, fps } = useVideoConfig();
  const typeFrom = 0.4 * fps;
  const perChar = 3;
  const typed = TYPED.slice(0, Math.max(0, Math.min(TYPED.length, Math.floor((frame - typeFrom) / perChar))));
  const swapAt = typeFrom + TYPED.length * perChar + 0.35 * fps;
  const swap = ramp(frame, swapAt, swapAt + 0.4 * fps);
  const results = ramp(frame, swapAt + 0.3 * fps, swapAt + 0.8 * fps);
  const caretOn = Math.floor(frame / 8) % 2 === 0 && swap < 0.01;

  return (
    <AbsoluteFill style={{ backgroundColor: sceneBg(), opacity: loopFade(frame, total) }}>
      <div
        style={{
          position: "absolute",
          left: 96,
          right: 96,
          top: 110,
          height: 112,
          borderBottom: `2px solid ${INK}`,
          display: "flex",
          alignItems: "center",
          gap: 26,
        }}
      >
        <svg width="40" height="40" viewBox="0 0 20 20" fill="none">
          <circle cx="8.5" cy="8.5" r="6" stroke={INK} strokeWidth="1.6" />
          <path d="m13 13 5 5" stroke={INK} strokeWidth="1.6" strokeLinecap="round" />
        </svg>
        <div style={{ position: "relative", flex: 1, height: 70 }}>
          <div
            style={{
              position: "absolute",
              inset: 0,
              fontFamily: MONO,
              fontSize: 54,
              color: DIM,
              opacity: 1 - swap,
              filter: `blur(${swap * 10}px)`,
            }}
          >
            {typed}
            {caretOn && <span style={{ color: INK }}>|</span>}
          </div>
          <div
            style={{
              position: "absolute",
              inset: 0,
              fontFamily: SANS,
              fontSize: 62,
              fontWeight: 700,
              letterSpacing: -1.5,
              color: INK,
              opacity: swap,
              filter: `blur(${(1 - swap) * 10}px)`,
            }}
          >
            {FOUND}
            <span style={{ fontFamily: MONO, fontSize: 34, fontWeight: 500, color: GREEN, marginLeft: 24 }}>раскладка</span>
          </div>
        </div>
      </div>
      <div style={{ position: "absolute", left: 96, right: 96, top: 282, display: "flex", flexDirection: "column", gap: 26 }}>
        {[0.62, 0.44, 0.53].map((w, i) => {
          const r = ramp(frame, swapAt + 0.3 * fps + i * 4, swapAt + 0.8 * fps + i * 4);
          return (
            <div key={i} style={{ display: "flex", alignItems: "center", gap: 24, opacity: results * r, translate: `0px ${(1 - r) * 12}px` }}>
              <div style={{ width: 52, height: 52, borderRadius: 12, backgroundColor: i === 0 ? INK : FAINT }} />
              <div style={{ height: 14, borderRadius: 7, width: `${w * 100}%`, backgroundColor: i === 0 ? INK : FAINT, opacity: interpolate(i, [0, 2], [1, 0.8]) }} />
            </div>
          );
        })}
      </div>
    </AbsoluteFill>
  );
};
