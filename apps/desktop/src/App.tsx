import { useEffect } from "react";
import { WheelOverlay } from "./windows/overlay/WheelOverlay";

/** Route to the correct window based on URL search params */
function getWindowType(): string {
  const params = new URLSearchParams(window.location.search);
  return params.get("window") ?? "overlay";
}

export function App() {
  const windowType = getWindowType();

  useEffect(() => {
    // Prevent default context menu globally
    document.addEventListener("contextmenu", (e) => e.preventDefault());
  }, []);

  switch (windowType) {
    case "overlay":
      return <WheelOverlay />;
    case "tool":
      // Tool windows loaded in M3+
      return <div className="p-8 text-white">Tool window (M3+)</div>;
    case "settings":
      return <div className="p-8 text-white">Settings (M6+)</div>;
    default:
      return <WheelOverlay />;
  }
}
