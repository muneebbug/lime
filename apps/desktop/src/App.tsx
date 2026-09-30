import { useEffect } from "react";
import { WheelOverlay } from "./windows/overlay/WheelOverlay";
import { SettingsWindow } from "./windows/settings/SettingsWindow";
import { CommandPalette } from "./windows/palette/CommandPalette";

/** Route to the correct window based on URL search params */
function getWindowParams() {
  const params = new URLSearchParams(window.location.search);
  const windowType = params.get("window") ?? "overlay";
  return { windowType };
}

export function App() {
  const { windowType } = getWindowParams();

  useEffect(() => {
    // Prevent default context menu globally
    document.addEventListener("contextmenu", (e) => e.preventDefault());
  }, []);

  switch (windowType) {
    case "overlay":
      return <WheelOverlay />;

    case "settings":
      return <SettingsWindow />;

    case "palette":
      return <CommandPalette />;

    default:
      return <WheelOverlay />;
  }
}
