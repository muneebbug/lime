import React from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Minus, Square, X, RotateCcw, ChevronDown } from "lucide-react";

/**
 * Native Windows caption buttons (minimize, maximize, close).
 */
export function CaptionButtons({ className = "" }: { className?: string }) {
  const appWindow = getCurrentWebviewWindow();

  const handleMinimize = () => {
    appWindow.minimize().catch(console.error);
  };

  const handleMaximize = () => {
    appWindow.toggleMaximize().catch(console.error);
  };

  const handleClose = () => {
    appWindow.close().catch(console.error);
  };

  return (
    <div className={`flex items-center select-none ${className}`}>
      <button
        onClick={handleMinimize}
        className="w-10 h-7 flex items-center justify-center text-neutral-400 hover:text-white hover:bg-white/[0.08] transition-colors cursor-pointer"
        title="Minimize"
        tabIndex={-1}
      >
        <Minus size={13} strokeWidth={2} />
      </button>
      <button
        onClick={handleMaximize}
        className="w-10 h-7 flex items-center justify-center text-neutral-400 hover:text-white hover:bg-white/[0.08] transition-colors cursor-pointer"
        title="Maximize"
        tabIndex={-1}
      >
        <Square size={10} strokeWidth={2} />
      </button>
      <button
        onClick={handleClose}
        className="w-10 h-7 flex items-center justify-center text-neutral-400 hover:text-white hover:bg-[#e81123] transition-colors cursor-pointer rounded-tr-lg"
        title="Close (Esc)"
        tabIndex={-1}
      >
        <X size={13} strokeWidth={2} />
      </button>
    </div>
  );
}

/**
 * Toggle switch component.
 */
export interface ToggleSwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  id?: string;
}

export function ToggleSwitch({ checked, onChange, disabled = false, id }: ToggleSwitchProps) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none disabled:cursor-not-allowed disabled:opacity-40 ${
        checked ? "bg-neutral-200" : "bg-[#333333] border border-white/[0.06]"
      }`}
    >
      <span
        className={`pointer-events-none inline-block h-3.5 w-3.5 transform rounded-full shadow-sm transition duration-200 ease-in-out ${
          checked
            ? "translate-x-4 bg-neutral-900"
            : "translate-x-0.5 bg-neutral-400"
        }`}
      />
    </button>
  );
}

/**
 * Keyboard hotkey pill display.
 */
export interface HotkeyPillProps {
  label: string;
  sublabel?: string;
  onReset?: () => void;
  isRecording?: boolean;
  onClick?: () => void;
}

export function HotkeyPill({
  label,
  sublabel,
  onReset,
  isRecording = false,
  onClick,
}: HotkeyPillProps) {
  return (
    <div className="flex items-center gap-1.5">
      {sublabel && (
        <button
          onClick={onClick}
          className="flex items-center gap-1.5 px-3 py-1 bg-[#252525] hover:bg-[#2e2e2e] border border-white/[0.08] text-xs text-neutral-300 font-medium rounded-md transition-colors cursor-pointer"
        >
          {sublabel}
        </button>
      )}

      <button
        onClick={onClick}
        className={`flex items-center gap-1 px-3 py-1 border text-xs font-mono rounded-md transition-colors cursor-pointer ${
          isRecording
            ? "bg-[#ff6339]/20 border-[#ff6339] text-[#ff6339] animate-pulse"
            : "bg-[#252525] hover:bg-[#2e2e2e] border border-white/[0.08] text-neutral-200"
        }`}
      >
        <span>{isRecording ? "Press keys..." : label}</span>
      </button>

      {onReset && (
        <button
          onClick={onReset}
          className="p-1.5 text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.06] rounded-md transition-colors cursor-pointer"
          title="Reset to default"
        >
          <RotateCcw size={13} />
        </button>
      )}
    </div>
  );
}

/**
 * Dropdown select input.
 */
export interface SelectOption {
  label: string;
  value: string;
  icon?: React.ReactNode;
}

export interface SelectDropdownProps {
  value: string;
  options: SelectOption[];
  onChange: (val: string) => void;
  className?: string;
}

export function SelectDropdown({ value, options, onChange, className = "" }: SelectDropdownProps) {
  const current = options.find((o) => o.value === value) || options[0];

  return (
    <div className={`relative inline-block ${className}`}>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="opacity-0 absolute inset-0 w-full h-full cursor-pointer z-10"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value} className="bg-[#222222] text-white">
            {opt.label}
          </option>
        ))}
      </select>

      <div className="flex items-center gap-2 px-3 py-1 bg-[#252525] hover:bg-[#2e2e2e] border border-white/[0.08] text-xs text-neutral-200 rounded-lg pointer-events-none transition-colors">
        {current?.icon && <span className="text-neutral-400 text-xs">{current.icon}</span>}
        <span className="font-normal">{current?.label}</span>
        <ChevronDown size={12} className="text-neutral-500 ml-0.5" />
      </div>
    </div>
  );
}

/**
 * Segmented control button group.
 */
export interface SegmentedControlProps {
  options: { label: React.ReactNode; value: string; title?: string }[];
  value: string;
  onChange: (val: string) => void;
  className?: string;
}

export function SegmentedControl({
  options,
  value,
  onChange,
  className = "",
}: SegmentedControlProps) {
  return (
    <div
      className={`inline-flex items-center bg-[#252525] p-0.5 rounded-lg border border-white/[0.08] ${className}`}
    >
      {options.map((opt) => {
        const isActive = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            title={opt.title}
            onClick={() => onChange(opt.value)}
            className={`px-2.5 py-1 text-xs font-medium rounded-md transition-all cursor-pointer ${
              isActive
                ? "bg-white/[0.12] text-white shadow-sm font-semibold"
                : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
            }`}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}

/**
 * Action button component.
 */
export interface WheelButtonProps {
  children: React.ReactNode;
  onClick?: () => void;
  variant?: "primary" | "secondary" | "danger";
  disabled?: boolean;
  className?: string;
  kbd?: string;
}

export function WheelButton({
  children,
  onClick,
  variant = "secondary",
  disabled = false,
  className = "",
  kbd,
}: WheelButtonProps) {
  let baseStyle =
    "flex items-center gap-1.5 px-3.5 py-1.5 text-xs font-medium rounded-lg transition-all cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed";

  if (variant === "primary") {
    baseStyle += " bg-[#ff6339] hover:bg-[#ff7247] text-white shadow-sm active:scale-[0.98]";
  } else if (variant === "danger") {
    baseStyle += " bg-red-500/15 hover:bg-red-500/25 text-red-400 border border-red-500/25";
  } else {
    baseStyle += " bg-[#262626] hover:bg-[#303030] text-neutral-200 border border-white/[0.08]";
  }

  return (
    <button onClick={onClick} disabled={disabled} className={`${baseStyle} ${className}`}>
      <span>{children}</span>
      {kbd && (
        <kbd className="text-[10px] bg-black/25 text-white/90 px-1.5 py-0.5 rounded font-mono font-bold ml-1">
          {kbd}
        </kbd>
      )}
    </button>
  );
}

/**
 * Settings row with Title, description, and Control.
 */
export interface SettingRowProps {
  title: string;
  description?: string;
  children: React.ReactNode;
  className?: string;
}

export function SettingRow({ title, description, children, className = "" }: SettingRowProps) {
  return (
    <div
      className={`flex items-center justify-between py-3 border-b border-white/[0.04] last:border-b-0 gap-4 ${className}`}
    >
      <div className="flex-1 pr-4">
        <span className="text-[13px] text-neutral-200 font-normal leading-snug block">
          {title}
        </span>
        {description && (
          <p className="text-xs text-neutral-500 mt-0.5 leading-normal">{description}</p>
        )}
      </div>
      <div className="shrink-0 flex items-center">{children}</div>
    </div>
  );
}

/**
 * Section container with group header.
 */
export interface SettingSectionProps {
  title?: string;
  children: React.ReactNode;
  className?: string;
}

export function SettingSection({ title, children, className = "" }: SettingSectionProps) {
  return (
    <div className={`mb-6 ${className}`}>
      {title && (
        <h3 className="text-xs font-semibold text-neutral-400 mb-2 tracking-normal uppercase">
          {title}
        </h3>
      )}
      <div className="flex flex-col">{children}</div>
    </div>
  );
}
