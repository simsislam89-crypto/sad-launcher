import { AbsoluteFill, interpolate, useCurrentFrame, useVideoConfig } from "remotion";
import { BG, CHIP, sceneBg, FAINT, GREEN, INK, SANS, ramp, loopFade } from "./theme";

const STEPS = [
  { name: "Java 17", time: "скачана" },
  { name: "Forge 1.20.1", time: "1,8 с" },
  { name: "Моды пака", time: "54 из 54" },
  { name: "Зависимости", time: "готово" },
];

export const Pipeline = () => {
  const frame = useCurrentFrame();
  const { durationInFrames: total, fps } = useVideoConfig();
  const step = 0.62 * fps;
  const first = 0.5 * fps;
  const rail = ramp(frame, first, first + step * STEPS.length);

  return (
    <AbsoluteFill style={{ backgroundColor: sceneBg(), opacity: loopFade(frame, total) }}>
      <div style={{ position: "absolute", left: 120, top: 150, bottom: 150, width: 2, backgroundColor: FAINT }}>
        <div style={{ width: 2, height: `${rail * 100}%`, backgroundColor: GREEN }} />
      </div>
      <div style={{ position: "absolute", left: 96, right: 96, top: 122, bottom: 122, display: "flex", flexDirection: "column", justifyContent: "space-between" }}>
        {STEPS.map((s, i) => {
          const at = first + i * step;
          const d = ramp(frame, at, at + 0.3 * fps);
          return (
            <div key={s.name} style={{ display: "flex", alignItems: "center", gap: 36 }}>
              <div
                style={{
                  width: 50,
                  height: 50,
                  borderRadius: 25,
                  border: `2px solid ${d > 0.5 ? GREEN : FAINT}`,
                  backgroundColor: d > 0.5 ? GREEN : CHIP,
                  display: "grid",
                  placeItems: "center",
                  scale: interpolate(d, [0, 0.5, 1], [1, 1.12, 1]),
                }}
              >
                <svg width="26" height="26" viewBox="0 0 24 24" style={{ opacity: d }}>
                  <polyline points="4 12 9.5 17.5 20 6.5" fill="none" stroke={BG} strokeWidth="3.2" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </div>
              <div style={{ flex: 1, fontFamily: SANS, fontSize: 56, fontWeight: 700, letterSpacing: -1.4, color: INK, opacity: interpolate(d, [0, 1], [0.35, 1]) }}>
                {s.name}
              </div>
            </div>
          );
        })}
      </div>
    </AbsoluteFill>
  );
};
