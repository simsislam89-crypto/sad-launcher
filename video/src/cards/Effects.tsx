import { AbsoluteFill, interpolate, useCurrentFrame, useVideoConfig } from "remotion";
import { sceneBg, FAINT, GREEN, INK, SANS, ramp, loopFade } from "./theme";

const ROWS = [
  { name: "Fullbright", note: "яркость 15" },
  { name: "Без тумана", note: "вода и лава" },
  { name: "Автокликер", note: "10 мс" },
];

export const Effects = () => {
  const frame = useCurrentFrame();
  const { durationInFrames: total, fps } = useVideoConfig();

  return (
    <AbsoluteFill style={{ backgroundColor: sceneBg(), opacity: loopFade(frame, total) }}>
      <div style={{ position: "absolute", left: 120, right: 120, top: 104, display: "flex", flexDirection: "column", gap: 34 }}>
        {ROWS.map((row, i) => {
          const at = 0.5 * fps + i * 0.55 * fps;
          const on = ramp(frame, at, at + 0.35 * fps);
          return (
            <div key={row.name} style={{ display: "flex", alignItems: "center", gap: 28 }}>
              <div style={{ flex: 1, fontFamily: SANS, fontSize: 60, fontWeight: 700, color: INK, letterSpacing: -1.5 }}>
                {row.name}
              </div>
              <div
                style={{
                  position: "relative",
                  width: 112,
                  height: 60,
                  borderRadius: 30,
                  border: `2px solid ${interpolate(on, [0, 1], [0, 1]) > 0.5 ? GREEN : FAINT}`,
                  backgroundColor: `rgba(130, 230, 165, ${0.16 * on})`,
                }}
              >
                <div
                  style={{
                    position: "absolute",
                    top: 8,
                    left: 8,
                    width: 40,
                    height: 40,
                    borderRadius: 20,
                    translate: `${on * 52}px 0px`,
                    backgroundColor: on > 0.5 ? GREEN : INK,
                  }}
                />
              </div>
            </div>
          );
        })}
      </div>
    </AbsoluteFill>
  );
};
