import { useEffect, type ReactNode } from "react";
import type { VisibleSceneDTO } from "../../api/types";
import { useGameStore } from "../../stores/gameStore";

interface AtmosphereLayerProps {
  atmosphere?: VisibleSceneDTO["atmosphere"];
  san?: number;
  sanMax?: number;
  children: ReactNode;
}

function getAtmosphereClass(atmosphere?: VisibleSceneDTO["atmosphere"]): string {
  switch (atmosphere) {
    case "candlelight":
      return "vellum-atmosphere-candlelight";
    case "fog":
      return "vellum-atmosphere-fog";
    case "san_strain":
    case "rain_manor":
    default:
      return "vellum-atmosphere-rain";
  }
}

export function AtmosphereLayer({
  atmosphere = "rain_manor",
  san,
  sanMax,
  children,
}: AtmosphereLayerProps) {
  const lowSanEffectsEnabled = useGameStore((s) => s.lowSanEffectsEnabled);
  const lowSanAttr = lowSanEffectsEnabled ? "on" : "off";

  useEffect(() => {
    document.documentElement.dataset.lowSanFx = lowSanAttr;
  }, [lowSanAttr]);

  const isLowSan =
    atmosphere === "san_strain" ||
    (typeof san === "number" &&
      typeof sanMax === "number" &&
      sanMax > 0 &&
      san <= Math.min(35, Math.floor(sanMax * 0.55)));

  const atmosphereClass = getAtmosphereClass(atmosphere);

  return (
    <div
      data-testid="atmosphere-layer"
      data-atmosphere={atmosphere}
      data-low-san-fx={lowSanAttr}
      className={`relative min-h-screen bg-[var(--vellum-bg)] text-[var(--vellum-ink)] ${atmosphereClass}`}
    >
      {isLowSan ? (
        <div
          aria-hidden="true"
          className="vellum-san-strain pointer-events-none fixed inset-0 z-10"
        />
      ) : null}
      <div className="relative z-10 flex min-h-screen flex-col">{children}</div>
    </div>
  );
}
