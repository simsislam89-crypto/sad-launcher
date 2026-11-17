import { AbsoluteFill, interpolate, useCurrentFrame, useVideoConfig } from "remotion";
import { CHIP, sceneBg, FAINT, GREEN, INK, DIM, SANS, ramp, loopFade } from "./theme";

const NODES = [
  { name: "Forge", x: -330, y: -120 },
  { name: "NeoForge", x: -330, y: 120 },
  { name: "Fabric", x: 330, y: -120 },
  { name: "Quilt", x: 330, y: 120 },
];

export const Loaders = () => {
  const frame = useCurrentFrame();
  const { durationInFrames: total, fps, width, height } = useVideoConfig();
  const cx = width / 2;
  const cy = height / 2;
  const core = ramp(frame, 0.2 * fps, 0.7 * fps);
  // The active loader moves round the four: the launcher installs any of them.
  const active = Math.floor(Math.max(0, frame - 1.4 * fps) / (0.6 * fps)) % NODES.length;

  return (
    <AbsoluteFill style={{ backgroundColor: sceneBg(), opacity: loopFade(frame, total) }}>
      <svg width={width} height={height} style={{ position: "absolute", inset: 0 }}>
        {NODES.map((n, i) => {
          const draw = ramp(frame, 0.6 * fps + i * 4, 1.2 * fps + i * 4);
          const x2 = cx + n.x * draw;
          const y2 = cy + n.y * draw;
          const on = frame > 1.4 * fps && i === active;
          return <line key={n.name} x1={cx} y1={cy} x2={x2} y2={y2} stroke={on ? GREEN : FAINT} strokeWidth={on ? 3 : 2} />;
        })}
      </svg>
      {NODES.map((n, i) => {
        const show = ramp(frame, 0.9 * fps + i * 4, 1.4 * fps + i * 4);
        const on = frame > 1.4 * fps && i === active;
        return (
          <div
            key={n.name}
            style={{
              position: "absolute",
              left: cx + n.x - 110,
              top: cy + n.y - 34,
              width: 220,
              height: 68,
              borderRadius: 34,
              border: `2px solid ${on ? GREEN : FAINT}`,
              backgroundColor: CHIP,
              display: "grid",
              placeItems: "center",
              fontFamily: SANS,
              fontSize: 32,
              fontWeight: 500,
              color: on ? GREEN : DIM,
              opacity: show,
              scale: interpolate(show, [0, 1], [0.9, 1]),
            }}
          >
            {n.name}
          </div>
        );
      })}
      <div
        style={{
          position: "absolute",
          left: cx - 64,
          top: cy - 64,
          width: 128,
          height: 128,
          borderRadius: 30,
          border: `2px solid ${INK}`,
          backgroundColor: CHIP,
          display: "grid",
          placeItems: "center",
          fontFamily: SANS,
          fontSize: 70,
          fontWeight: 700,
          color: INK,
          opacity: core,
          scale: interpolate(core, [0, 1], [0.8, 1]),
        }}
      >
        S
      </div>
    </AbsoluteFill>
  );
};
