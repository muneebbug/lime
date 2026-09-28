import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  FileText,
  Search,
  Copy,
  Check,
  MapPin,
  ExternalLink,
  Shield,
  Layers,
  ChevronDown,
  ChevronRight,
} from "lucide-react";
import { ToolWindowLayout } from "./ToolWindowLayout";

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
      icon={<FileText size={16} />}
      primaryActionLabel={stripGpsOnly ? "Strip GPS Location" : "Strip All Metadata"}
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleReset}
      onPrimaryAction={handleStrip}
    >
      <div className="flex flex-col h-full gap-4">
        {/* Search & Actions Bar */}
        <div className="flex items-center justify-between gap-3">
          {/* Search field */}
          <div className="relative flex-1">
            <Search
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500"
            />
            <input
              type="text"
              placeholder="Search metadata keys and values..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-9 pr-3 py-1.5 rounded-lg bg-zinc-900/60 border border-white/10 text-xs text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-orange-500 transition-colors"
            />
          </div>

          {/* Strip scope mode selector */}
          <div className="flex items-center gap-1 bg-zinc-900/60 p-1 rounded-lg border border-white/5 text-xs">
            <button
              onClick={() => setStripGpsOnly(false)}
              className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-medium transition-all cursor-pointer ${
                !stripGpsOnly
                  ? "bg-orange-500 text-zinc-950 font-semibold shadow-sm"
                  : "text-zinc-400 hover:text-zinc-200"
              }`}
            >
              <Shield size={12} />
              Strip All
            </button>
            <button
              onClick={() => setStripGpsOnly(true)}
              className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-medium transition-all cursor-pointer ${
                stripGpsOnly
                  ? "bg-orange-500 text-zinc-950 font-semibold shadow-sm"
                  : "text-zinc-400 hover:text-zinc-200"
              }`}
            >
              <MapPin size={12} />
              GPS Only
            </button>
          </div>
        </div>

        {/* GPS location pill banner if coordinates found */}
        {report?.gps && (
          <div className="flex items-center justify-between px-3.5 py-2.5 rounded-xl bg-orange-500/10 border border-orange-500/20 text-xs">
            <div className="flex items-center gap-2 text-orange-200 font-medium">
              <MapPin size={16} className="text-orange-400 shrink-0" />
              <span>
                GPS: {report.gps.latitude.toFixed(5)}°, {report.gps.longitude.toFixed(5)}°
                {report.gps.altitude ? ` (${Math.round(report.gps.altitude)}m alt)` : ""}
              </span>
            </div>
            <button
              onClick={openMap}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md bg-orange-500/20 hover:bg-orange-500/30 text-orange-300 font-medium text-[11px] transition-colors cursor-pointer"
            >
              <ExternalLink size={12} />
              <span>View on Map</span>
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
                  className="rounded-xl border border-white/5 bg-zinc-900/40 overflow-hidden"
                >
                  <button
                    onClick={() => toggleGroup(group.name)}
                    className="w-full flex items-center justify-between px-3.5 py-2 bg-zinc-900/60 hover:bg-zinc-900/80 transition-colors text-left cursor-pointer"
                  >
                    <span className="text-xs font-semibold text-zinc-300">
                      {group.name} ({group.entries.length})
                    </span>
                    <span className="text-zinc-500">
                      {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                    </span>
                  </button>

                  {isExpanded && (
                    <div className="divide-y divide-white/5">
                      {group.entries.map((entry, idx) => {
                        const itemKey = `${group.name}-${entry.key}-${idx}`;
                        const isCopied = copiedKey === itemKey;
                        return (
                          <div
                            key={itemKey}
                            className="flex items-center justify-between px-3.5 py-2 text-xs hover:bg-white/5 transition-colors group"
                          >
                            <span className="font-medium text-zinc-400 w-1/3 truncate">
                              {entry.key}
                            </span>
                            <span className="text-zinc-200 font-mono text-[11px] flex-1 truncate pr-3 text-right">
                              {entry.value}
                            </span>
                            <button
                              onClick={() => copyToClipboard(itemKey, entry.value)}
                              className="text-zinc-500 hover:text-orange-400 p-1 rounded transition-colors cursor-pointer"
                              title="Copy value"
                            >
                              {isCopied ? (
                                <Check size={13} className="text-emerald-400" />
                              ) : (
                                <Copy size={13} />
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
