import { useEffect } from "react";
import { WheelOverlay } from "./windows/overlay/WheelOverlay";
import { CropTool } from "./windows/tools/CropTool";
import { CompressTool } from "./windows/tools/CompressTool";
import { MetadataTool } from "./windows/tools/MetadataTool";
import { AddBgTool } from "./windows/tools/AddBgTool";
import { EditTool } from "./windows/tools/EditTool";
import { RemoveBgTool } from "./windows/tools/RemoveBgTool";
import { RedactTool } from "./windows/tools/RedactTool";
import { AnnotateTool } from "./windows/tools/AnnotateTool";
import { ToolWindowLayout } from "./windows/tools/ToolWindowLayout";

/** Route to the correct window based on URL search params */
function getWindowParams() {
  const params = new URLSearchParams(window.location.search);
  const windowType = params.get("window") ?? "overlay";
  const toolId = params.get("tool") ?? "";
  const rawFiles = params.get("files") ?? "[]";
  let files: string[] = [];
  try {
    files = JSON.parse(rawFiles);
  } catch {
    files = [];
  }
  return { windowType, toolId, files };
}

export function App() {
  const { windowType, toolId, files } = getWindowParams();
  const firstFile = files[0] ?? "";

  useEffect(() => {
    // Prevent default context menu globally
    document.addEventListener("contextmenu", (e) => e.preventDefault());
  }, []);

  switch (windowType) {
    case "overlay":
      return <WheelOverlay />;

    case "tool":
      switch (toolId) {
        case "tool.crop":
          return <CropTool filePath={firstFile} />;
        case "tool.compress":
          return <CompressTool filePath={firstFile} />;
        case "tool.metadata":
          return <MetadataTool filePath={firstFile} />;
        case "tool.addbg":
        case "tool.add_bg":
          return <AddBgTool filePath={firstFile} />;
        case "tool.edit":
          return <EditTool filePath={firstFile} />;
        case "tool.removebg":
        case "tool.remove_bg":
          return <RemoveBgTool filePath={firstFile} />;
        case "tool.redact":
          return <RedactTool filePath={firstFile} />;
        case "tool.annotate":
          return <AnnotateTool filePath={firstFile} />;
        default:
          return (
            <ToolWindowLayout
              title={toolId || "Tool Window"}
              primaryActionLabel="Done"
              onReset={() => {}}
              onPrimaryAction={() => {}}
            >
              <div className="flex-1 flex flex-col items-center justify-center text-zinc-500 gap-2">
                <span className="text-sm">Tool window for: <code className="text-orange-400">{toolId}</code></span>
                <span className="text-xs text-zinc-600 truncate max-w-sm">{firstFile}</span>
              </div>
            </ToolWindowLayout>
          );
      }

    case "settings":
      return <div className="p-8 text-white">Settings (M6+)</div>;

    default:
      return <WheelOverlay />;
  }
}
