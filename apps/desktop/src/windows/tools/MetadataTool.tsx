import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  FileText,
  Search,
  Copy,
  Check,
  MapPin,
  ExternalLink,
  Layers,
  ChevronDown,
  ChevronRight,
} from "lucide-react";
import { ToolWindowLayout } from "./ToolWindowLayout";
import { SegmentedControl } from "../../ui/WheelUI";

export interface MetadataToolProps {
  filePath: string;
}

interface MetadataEntry {
  key: string;
  value: string;
}

interface MetadataGroup {
  name: string;
  entries: MetadataEntry[];
}

interface GpsCoordinates {
  latitude: number;
  longitude: number;
  altitude?: number;
}

interface FileMetadataReport {
  file_name: string;
  file_size: number;
  file_path: string;
  format: string;
  dimensions?: [number, number];
  gps?: GpsCoordinates;
  groups: MetadataGroup[];
}

export function MetadataTool({ filePath }: MetadataToolProps) {
  const [report, setReport] = useState<FileMetadataReport | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [stripGpsOnly, setStripGpsOnly] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [successPath, setSuccessPath] = useState<string | null>(null);
  const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>({});

  const fileName = filePath.split(/[/\\]/).pop() ?? filePath;

  useEffect(() => {
    invoke<FileMetadataReport>("get_image_metadata", { inputPath: filePath })
      .then((data) => {
        setReport(data);
        // Expand all groups by default
        const initExpanded: Record<string, boolean> = {};
        for (const g of data.groups) {
          initExpanded[g.name] = true;
        }
        setExpandedGroups(initExpanded);
      })
      .catch((e) => console.error("Failed to read metadata", e));
  }, [filePath]);

  const copyToClipboard = async (key: string, val: string) => {
    try {
      await navigator.clipboard.writeText(val);
      setCopiedKey(key);
      setTimeout(() => setCopiedKey(null), 1500);
    } catch (e) {
      console.error("Failed to copy to clipboard", e);
    }
  };

  const openMap = async () => {
    if (!report?.gps) return;
    const { latitude, longitude } = report.gps;
    const url = `https://www.google.com/maps/search/?api=1&query=${latitude},${longitude}`;
    try {
      await invoke("open_file", { path: url });
    } catch {
      window.open(url, "_blank");
    }
  };

  const toggleGroup = (groupName: string) => {
    setExpandedGroups((prev) => ({
      ...prev,
      [groupName]: !prev[groupName],
    }));
  };

  // Filter groups by search query
  const filteredGroups = useMemo(() => {
    if (!report) return [];
    if (!searchQuery.trim()) return report.groups;

    const q = searchQuery.toLowerCase();
    return report.groups
      .map((group) => {
        const matchesName = group.name.toLowerCase().includes(q);
        const matchedEntries = group.entries.filter(
          (e) => e.key.toLowerCase().includes(q) || e.value.toLowerCase().includes(q)
        );
        if (matchesName || matchedEntries.length > 0) {
          return {
            name: group.name,
            entries: matchesName ? group.entries : matchedEntries,
          };
        }
        return null;
      })
      .filter((g): g is MetadataGroup => g !== null);
  }, [report, searchQuery]);

  const handleReset = () => {
    setStripGpsOnly(false);
    setSearchQuery("");
    setSuccessPath(null);
  };

  const handleStrip = async () => {
    if (isProcessing) return;
    setIsProcessing(true);
    try {
      const res = await invoke<string>("strip_image_metadata", {
        inputPath: filePath,
        stripGpsOnly,
        outputPath: null,
      });
      setSuccessPath(res);
    } catch (e) {
      console.error("Failed to strip metadata", e);
      alert(`Strip metadata failed: ${e}`);
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <ToolWindowLayout
      title="File Metadata"
      subtitle={fileName}
      icon={<FileText size={14} />}
      primaryActionLabel={stripGpsOnly ? "Strip GPS Location" : "Strip All Metadata"}
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleReset}
      onPrimaryAction={handleStrip}
    >
      <div className="flex flex-col h-full gap-3 p-3">
        {/* Search & Actions Bar */}
        <div className="flex items-center justify-between gap-3">
          {/* Search field */}
          <div className="relative flex-1">
            <Search
              size={13}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-neutral-500"
            />
            <input
              type="text"
              placeholder="Search metadata keys and values..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-8 pr-3 py-1.5 rounded-lg bg-[#242424] border border-white/[0.06] text-xs text-neutral-200 placeholder:text-neutral-500 focus:outline-none focus:border-white/20 transition-colors"
            />
          </div>

          {/* Strip scope mode selector */}
          <SegmentedControl
            value={stripGpsOnly ? "gps" : "all"}
            options={[
              { label: "Strip All", value: "all" },
              { label: "GPS Only", value: "gps" },
            ]}
            onChange={(val) => setStripGpsOnly(val === "gps")}
          />
        </div>

        {/* GPS location pill banner if coordinates found */}
        {report?.gps && (
          <div className="flex items-center justify-between px-3.5 py-2 rounded-xl bg-[#242424] border border-white/[0.08] text-xs">
            <div className="flex items-center gap-2 text-neutral-200 font-medium">
              <MapPin size={14} className="text-[#ff6339] shrink-0" />
              <span>
                GPS: {report.gps.latitude.toFixed(5)}°, {report.gps.longitude.toFixed(5)}°
                {report.gps.altitude ? ` (${Math.round(report.gps.altitude)}m alt)` : ""}
              </span>
            </div>
            <button
              onClick={openMap}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md bg-white/[0.06] hover:bg-white/[0.1] text-neutral-300 font-medium text-[11px] transition-colors cursor-pointer"
            >
              <ExternalLink size={11} />
              <span>View Map</span>
            </button>
          </div>
        )}

        {/* Grouped Metadata List */}
        <div className="flex-1 overflow-y-auto space-y-3 pr-1">
          {filteredGroups.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-48 text-zinc-500 text-xs">
              <Layers size={24} className="mb-2 opacity-50" />
              <span>No metadata entries found</span>
            </div>
          ) : (
            filteredGroups.map((group) => {
              const isExpanded = expandedGroups[group.name] ?? true;
              return (
                <div
                  key={group.name}
                  className="rounded-xl border border-white/[0.06] bg-[#242424] overflow-hidden"
                >
                  <button
                    onClick={() => toggleGroup(group.name)}
                    className="w-full flex items-center justify-between px-3.5 py-2 bg-[#282828] hover:bg-[#2e2e2e] transition-colors text-left cursor-pointer"
                  >
                    <span className="text-xs font-semibold text-neutral-200">
                      {group.name} ({group.entries.length})
                    </span>
                    <span className="text-neutral-500">
                      {isExpanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
                    </span>
                  </button>

                  {isExpanded && (
                    <div className="divide-y divide-white/[0.04]">
                      {group.entries.map((entry, idx) => {
                        const itemKey = `${group.name}-${entry.key}-${idx}`;
                        const isCopied = copiedKey === itemKey;
                        return (
                          <div
                            key={itemKey}
                            className="flex items-center justify-between px-3.5 py-2 text-xs hover:bg-white/[0.03] transition-colors group"
                          >
                            <span className="font-normal text-neutral-400 w-1/3 truncate">
                              {entry.key}
                            </span>
                            <span className="text-neutral-200 font-mono text-[11px] flex-1 truncate pr-3 text-right">
                              {entry.value}
                            </span>
                            <button
                              onClick={() => copyToClipboard(itemKey, entry.value)}
                              className="text-neutral-500 hover:text-white p-1 rounded transition-colors cursor-pointer"
                              title="Copy value"
                            >
                              {isCopied ? (
                                <Check size={12} className="text-emerald-400" />
                              ) : (
                                <Copy size={12} />
                              )}
                            </button>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>
    </ToolWindowLayout>
  );
}
