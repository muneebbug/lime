import { useEffect } from "react";
import { WheelOverlay } from "./windows/overlay/WheelOverlay";
import { SettingsWindow } from "./windows/settings/SettingsWindow";
import { StatusHud } from "./windows/status/StatusHud";
import { OnboardingWindow } from "./windows/onboarding/OnboardingWindow";
import { RecolorWindow } from "./windows/recolor/RecolorWindow";
import { CompressWindow } from "./windows/compress/CompressWindow";

/** Route to the correct window based on URL search params */
function getWindowParams() {
  const params = new URLSearchParams(window.location.search);
  const windowType = params.get("window") ?? "overlay";
  return { windowType };
}

/**
 * Shown when a tool window is opened with an id this build does not know about.
 *
 * Reachable after a downgrade, or if a manifest is renamed without the window
 * following it.
 */
function UnknownTool({ tool }: { tool: string | null }) {
  return (
    <div className="h-screen w-screen flex items-center justify-center rounded-window border border-line-subtle bg-surface-window text-text font-sans select-none">
      <div className="max-w-[360px] text-center">
        <div className="text-[14px]">This tool is not available</div>
        <div className="text-[12px] text-text-muted mt-2 leading-relaxed">
          {tool ? (
            <>
              <code className="font-mono text-text-secondary">{tool}</code> is not a tool
              this build provides.
            </>
          ) : (
            "No tool was named."
          )}
        </div>
      </div>
    </div>
  );
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

    case "status":
      return <StatusHud />;

    case "onboarding":
      return <OnboardingWindow />;

    // Tool windows are all dispatched from one route: Rust opens them with
    // `?window=tool&tool=<action id>&files=<json>`, so the id decides which
    // screen renders. Adding a tool means adding a case here, not a new route.
    case "tool": {
      const tool = new URLSearchParams(window.location.search).get("tool");
      switch (tool) {
        case "tool.recolor":
          return <RecolorWindow />;
        case "tool.compress":
          return <CompressWindow />;
        default:
          // An unknown id means the window was opened by a build that had a tool
          // this one does not. Falling back to the overlay would be baffling, so
          // say what happened instead.
          return <UnknownTool tool={tool} />;
      }
    }

    default:
      return <WheelOverlay />;
  }
}
