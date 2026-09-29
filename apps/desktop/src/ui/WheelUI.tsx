import React from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Minus, X, RotateCcw, ChevronDown } from "lucide-react";

/** Native Windows caption buttons (minimize, close). */
export function CaptionButtons({ className = "" }: { className?: string }) {
  const appWindow = getCurrentWebviewWindow();
  const handleMinimize = () => appWindow.minimize().catch(console.error);
  const handleClose = () => appWindow.close().catch(console.error);

  return (
    <div className={`h-full flex items-stretch select-none ${className}`}>
      <button
        onClick={handleMinimize}
        className="w-[46px] h-full flex items-center justify-center text-[#9a9a9e] hover:text-white hover:bg-white/[0.08] transition-colors cursor-default"
        title="Minimize"
        tabIndex={-1}
      >
        <Minus size={15} strokeWidth={1.75} />
      </button>
      <button
        onClick={handleClose}
        className="w-[46px] h-full flex items-center justify-center text-[#9a9a9e] hover:text-white hover:bg-[#c4332a] transition-colors cursor-default"
        title="Close"
        tabIndex={-1}
      >
        <X size={15} strokeWidth={1.75} />
      </button>
    </div>
  );
}

/**
 * Toggle switch — exact Raycast Windows match.
 * OFF: dark track #3a3a3c, white knob
 * ON:  white track, dark knob #1c1c1e
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
      className={`relative inline-flex h-[22px] w-[38px] shrink-0 cursor-default items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none disabled:opacity-40 select-none ${
        checked ? "bg-white" : "bg-[#3a3a3c]"
      }`}
    >
      <span
        className={`pointer-events-none inline-block h-[18px] w-[18px] transform rounded-full shadow transition duration-200 ease-in-out ${
          checked ? "translate-x-[18px] bg-[#1c1c1e]" : "translate-x-[2px] bg-white"
        }`}
      />
    </button>
  );
}

/** Keyboard hotkey pill — Raycast compact button style. */
export interface HotkeyPillProps {
  label: string;
  sublabel?: string;
  onReset?: () => void;
  isRecording?: boolean;
  onClick?: () => void;
}

export function HotkeyPill({ label, sublabel, onReset, isRecording = false, onClick }: HotkeyPillProps) {
  return (
    <div className="flex items-center gap-1 select-none">
      {sublabel && (
        <button
          onClick={onClick}
          className="flex items-center gap-1 px-2.5 py-1 bg-[#2c2c2e] hover:bg-[#38383a] border border-white/[0.1] text-[13px] text-white font-medium rounded-md transition-colors cursor-default"
        >
          {sublabel}
        </button>
      )}
      <button
        onClick={onClick}
        className={`flex items-center gap-1 px-2.5 py-1 border text-[13px] font-mono rounded-md transition-colors cursor-default ${
          isRecording
            ? "bg-[#ff6339]/20 border-[#ff6339] text-[#ff6339] animate-pulse"
            : "bg-[#2c2c2e] hover:bg-[#38383a] border-white/[0.1] text-white"
        }`}
      >
        {isRecording ? "Press keys..." : label}
      </button>
      {onReset && (
        <button
          onClick={onReset}
          className="p-1 text-[#8e8e93] hover:text-white hover:bg-white/[0.08] rounded-md transition-colors cursor-default"
          title="Reset to default"
        >
          <RotateCcw size={13} />
        </button>
      )}
    </div>
  );
}

/** Dropdown select — Raycast pill style. */
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
    <div className={`relative inline-block select-none ${className}`}>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="opacity-0 absolute inset-0 w-full h-full cursor-default z-10"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value} className="bg-[#2c2c2e] text-white">
            {opt.label}
          </option>
        ))}
      </select>
      <div className="flex items-center gap-1.5 px-2.5 py-1 bg-[#2c2c2e] hover:bg-[#38383a] border border-white/[0.1] text-[13px] text-white rounded-md pointer-events-none transition-colors">
        {current?.icon && <span className="text-[#8e8e93]">{current.icon}</span>}
        <span>{current?.label}</span>
        <ChevronDown size={12} className="text-[#8e8e93] ml-0.5" />
      </div>
    </div>
  );
}

/** Segmented control — Raycast style. */
export interface SegmentedControlProps {
  options: { label: React.ReactNode; value: string; title?: string }[];
  value: string;
  onChange: (val: string) => void;
  className?: string;
}

export function SegmentedControl({ options, value, onChange, className = "" }: SegmentedControlProps) {
  return (
    <div className={`inline-flex items-center bg-[#2c2c2e] p-[3px] rounded-[8px] border border-white/[0.08] select-none ${className}`}>
      {options.map((opt) => {
        const isActive = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            title={opt.title}
            onClick={() => onChange(opt.value)}
            className={`px-3 py-[5px] text-[13px] font-medium rounded-[6px] transition-all cursor-default ${
              isActive ? "bg-[#48484a] text-white shadow-sm" : "text-[#8e8e93] hover:text-white"
            }`}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}

/** Action button — Raycast desktop style. */
export interface WheelButtonProps {
  children: React.ReactNode;
  onClick?: () => void;
  variant?: "primary" | "secondary" | "danger";
  disabled?: boolean;
  className?: string;
  kbd?: string;
}

export function WheelButton({ children, onClick, variant = "secondary", disabled = false, className = "", kbd }: WheelButtonProps) {
  let base =
    "inline-flex items-center gap-1.5 px-3 py-[6px] text-[13px] font-medium rounded-[8px] transition-all cursor-default select-none disabled:opacity-40";

  if (variant === "primary") {
    base += " bg-[#ff6339] hover:bg-[#ff7247] text-white active:scale-[0.98]";
  } else if (variant === "danger") {
    base += " bg-red-500/15 hover:bg-red-500/25 text-red-400 border border-red-500/20";
  } else {
    base += " bg-[#2c2c2e] hover:bg-[#38383a] text-white border border-white/[0.1]";
  }

  return (
    <button onClick={onClick} disabled={disabled} className={`${base} ${className}`}>
      <span>{children}</span>
      {kbd && (
        <kbd className="text-[10px] bg-black/25 text-white/80 px-1.5 py-0.5 rounded font-mono ml-1">
          {kbd}
        </kbd>
      )}
    </button>
  );
}

/**
 * Setting row — exact Raycast style.
 * Rows sit directly on the dark background. No card container.
 * Subtle border-bottom separates rows.
 */
export interface SettingRowProps {
  title: string;
  description?: string;
  children: React.ReactNode;
  className?: string;
}

export function SettingRow({ title, description, children, className = "" }: SettingRowProps) {
  return (
    <div className={`flex items-center justify-between min-h-[44px] py-[10px] px-4 border-b border-white/[0.06] last:border-b-0 gap-6 ${className}`}>
      <div className="flex-1 min-w-0">
        <div className="text-[13px] text-white font-normal leading-snug">{title}</div>
        {description && (
          <div className="text-[12px] text-[#8e8e93] mt-[2px] leading-snug">{description}</div>
        )}
      </div>
      <div className="shrink-0 flex items-center">{children}</div>
    </div>
  );
}

/**
 * Setting section — plain header + rows, NO card/container border.
 * Section header = small muted grey text, exactly like Raycast.
 */
export interface SettingSectionProps {
  title?: string;
  children: React.ReactNode;
  className?: string;
}

export function SettingSection({ title, children, className = "", first = false }: SettingSectionProps & { first?: boolean }) {
  return (
    <div className={`${className}`}>
      {title && (
        <h3 className={`text-[12px] font-semibold uppercase tracking-wider text-[#8e8e93] mb-[6px] ${first ? "mt-0" : "mt-6"}`}>
          {title}
        </h3>
      )}
      {/* Raycast grouped-list card */}
      <div className="bg-[#242323] rounded-[10px] overflow-hidden">
        {children}
      </div>
    </div>
  );
}
