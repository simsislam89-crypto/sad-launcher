import { AbsoluteFill, interpolate, useCurrentFrame, useVideoConfig } from "remotion";
import { sceneBg, FAINT, GREEN, INK, DIM, SANS, MONO, ramp, loopFade } from "./theme";

const CODE = "48213";

const Node = ({ x, label, show }: { x: number; label: string; show: number }) => (
  <div
    style={{
      position: "absolute",
      left: x - 60,
      top: 236,
      width: 120,
      display: "flex",
      flexDirection: "column",
      alignItems: "center",
      gap: 18,
      opacity: show,
      scale: interpolate(show, [0, 1], [0.85, 1]),
    }}
  >
    <div style={{ width: 92, height: 92, borderRadius: 46, border: `2px solid ${INK}`, display: "grid", placeItems: "center" }}>
      <div style={{ width: 22, height: 22, borderRadius: 11, backgroundColor: INK }} />
    </div>
    <div style={{ fontFamily: SANS, fontSize: 40, fontWeight: 500, color: DIM }}>{label}</div>
  </div>
);

export const Friend = () => {
  const frame = useCurrentFrame();
  const { durationInFrames: total, fps, width } = useVideoConfig();
  const left = 170;
  const right = width - 170;
  const link = ramp(frame, 1.6 * fps, 2.2 * fps);
  const lan = ramp(frame, 2.4 * fps, 2.8 * fps);
  // Two packets ride the link in opposite directions once it is up.
  const t = Math.max(0, frame - 2.2 * fps) / (1.1 * fps);
  const a = t % 1;
  const b = (t + 0.5) % 1;

  return (
    <AbsoluteFill style={{ backgroundColor: sceneBg(), opacity: loopFade(frame, total) }}>
      <div style={{ position: "absolute", top: 70, width: "100%", display: "flex", justifyContent: "center", gap: 14 }}>
        {CODE.split("").map((d, i) => {
          const s = ramp(frame, 0.3 * fps + i * 4, 0.3 * fps + i * 4 + 12);
          return (
            <div
              key={i}
              style={{
                width: 84,
                height: 104,
                borderRadius: 18,
                border: `2px solid ${s > 0.5 ? INK : FAINT}`,
                display: "grid",
                placeItems: "center",
                fontFamily: MONO,
                fontSize: 54,
                color: INK,
                opacity: interpolate(s, [0, 1], [0.3, 1]),
                translate: `0px ${(1 - s) * 14}px`,
              }}
            >
              {d}
            </div>
          );
        })}
      </div>
      <div style={{ position: "absolute", left, top: 282, width: (right - left) * link, height: 2, backgroundColor: FAINT }} />
      {link > 0.99 && (
        <>
          <div style={{ position: "absolute", left: left + (right - left) * a - 7, top: 276, width: 14, height: 14, borderRadius: 7, backgroundColor: GREEN }} />
          <div style={{ position: "absolute", left: right - (right - left) * b - 5, top: 278, width: 10, height: 10, borderRadius: 5, backgroundColor: INK }} />
        </>
      )}
      <Node x={left} label="Вы" show={ramp(frame, 0.9 * fps, 1.4 * fps)} />
      <Node x={right} label="Друг" show={ramp(frame, 1.2 * fps, 1.7 * fps)} />
      <div
        style={{
          position: "absolute",
          bottom: 52,
          width: "100%",
          display: "flex",
          justifyContent: "center",
          opacity: lan,
          translate: `0px ${(1 - lan) * 10}px`,
        }}
      >
        <div style={{ padding: "12px 28px", borderRadius: 40, border: `2px solid ${GREEN}`, color: GREEN, fontFamily: SANS, fontSize: 38, fontWeight: 500 }}>
          в одной сети
        </div>
      </div>
    </AbsoluteFill>
  );
};
