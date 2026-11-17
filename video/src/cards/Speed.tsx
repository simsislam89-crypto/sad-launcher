import { AbsoluteFill, interpolate, useCurrentFrame, useVideoConfig } from "remotion";
import { sceneBg, FAINT, GREEN, INK, DIM, SANS, MONO, ramp, loopFade } from "./theme";

// Eight parallel lanes fill at their own pace; the counter settles on the real
// measurement from RRR.md (Additive: 54 files in 4,4 s).
const LANES = [0.92, 0.7, 1, 0.82, 0.62, 0.95, 0.76, 0.88];

export const Speed = () => {
  const frame = useCurrentFrame();
  const { durationInFrames: total, fps } = useVideoConfig();
  const run = 2.6 * fps;
  const done = ramp(frame, 8, run);
  const mods = Math.round(54 * done);
  const secs = (4.4 * done).toFixed(1).replace(".", ",");

  return (
    <AbsoluteFill style={{ backgroundColor: sceneBg(), opacity: loopFade(frame, total) }}>
      <div style={{ position: "absolute", left: 96, top: 92, fontFamily: SANS, color: INK }}>
        <div style={{ fontSize: 88, fontWeight: 700, letterSpacing: -2.5, fontVariantNumeric: "tabular-nums" }}>
          {mods} <span style={{ color: DIM, fontWeight: 500 }}>модов</span>
        </div>
        <div style={{ fontFamily: MONO, fontSize: 36, color: DIM, marginTop: 8 }}>
          {secs} с · 8 потоков
        </div>
      </div>
      <div style={{ position: "absolute", left: 96, right: 96, top: 292, display: "flex", flexDirection: "column", gap: 17 }}>
        {LANES.map((speed, i) => {
          const start = 6 + i * 3;
          const end = start + run * speed;
          const p = ramp(frame, start, end);
          const finished = p > 0.995;
          return (
            <div key={i} style={{ position: "relative", height: 6, borderRadius: 3, backgroundColor: FAINT }}>
              <div
                style={{
                  position: "absolute",
                  left: 0,
                  top: 0,
                  bottom: 0,
                  width: `${p * 100}%`,
                  borderRadius: 3,
                  backgroundColor: finished ? GREEN : INK,
                  opacity: interpolate(p, [0, 1], [0.55, 1]),
                }}
              />
            </div>
          );
        })}
      </div>
    </AbsoluteFill>
  );
};
