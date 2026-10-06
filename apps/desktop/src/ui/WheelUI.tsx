import React, { useMemo } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Minus, X, RotateCcw, ChevronDown } from "lucide-react";

/**
 * Window chrome shared by every Lime window.
 *
 * The title bar lives here rather than in each screen so a new window gets an
 * identical one for free. Height, background, border and drag handling all come
 * from the same tokens; see `index.css` for the values.
 */
const TITLE_BAR_CLASS =
  "h-[38px] shrink-0 flex items-center gap-2 pl-4 pr-0 border-b border-chrome-border bg-chrome-bg z-20";

/**
 * Native Windows caption buttons (minimize, close).
 *
 * Pass `onClose` for windows that must survive being closed — it replaces the
 * destroy with whatever the caller wants, usually a hide.
 */
export interface WindowFrameProps {
  /** Window name shown in the title bar. */
  title: string;
  /** Window content below the title bar. */
  children: React.ReactNode;
  /** Hide the wordmark, for windows that do not need one. */
  hideLogo?: boolean;
  /**
   * Extra controls for the title bar, rendered before the caption buttons.
   * This is the reserved slot for window-specific actions.
   */
  titleBarExtras?: React.ReactNode;
  /**
   * Replaces the default destroy-on-close with a custom behaviour. Windows that
   * must outlive a close pass this; see `TitleBar.onClose`.
   */
  onClose?: () => void;
  /**
   * Classes for the content area. The default is a plain block; override it to
   * centre content or to lay the body out with flex.
   */
  bodyClassName?: string;
}

/**
 * A complete Lime window: title bar above, content below.
 *
 * Every window should use this rather than assembling its own root, so chrome,
 * background, corner radius and border stay identical as more windows are added.
 *
 * ```tsx
 * <WindowFrame title="Convert to PNG" titleBarExtras={<HelpButton />}>
 *   ...content...
 * </WindowFrame>
 * ```
 */
export function WindowFrame({
  title,
  children,
  hideLogo,
  titleBarExtras,
  onClose,
  bodyClassName = "",
}: WindowFrameProps) {
  return (
    /*
     * Full bleed, with no margin. The window is transparent, so any padding
     * would show desktop pixels as a halo around the card. The card also draws
     * no outer box-shadow: a transparent window clips it at its own edge, which
     * reads as a hard outline. Depth comes from the OS drop shadow instead.
     *
     * `border` is inside the radius, so it defines the edge cleanly and will
     * never fight the Windows frame — the DWM border is removed in Rust.
     */
    <div className="h-screen w-screen flex flex-col overflow-hidden rounded-window border border-line-subtle bg-surface-window text-text font-sans select-none">
      <TitleBar title={title} hideLogo={hideLogo} onClose={onClose}>
        {titleBarExtras}
      </TitleBar>
      <div className={`flex-1 min-h-0 overflow-hidden ${bodyClassName}`}>{children}</div>
    </div>
  );
}

export function CaptionButtons({ className = "", onClose }: { className?: string; onClose?: () => void }) {
  // Resolve once: calling this per render would hand back a new object each
  // time and defeat any effect that depends on it.
  const appWindow = useMemo(() => getCurrentWebviewWindow(), []);
  const handleMinimize = () => appWindow.minimize().catch(console.error);

  // Windows that must outlive a close (onboarding, for one) pass `onClose` so
  // they can hide instead of being destroyed.
  const handleClose = () =>
    onClose ? onClose() : appWindow.close().catch(console.error);

  return (
    <div data-tauri-drag-region="false" className={`h-full flex items-stretch select-none ${className}`}>
      <button
        data-tauri-drag-region="false"
        onClick={handleMinimize}
        className="w-[46px] h-full flex items-center justify-center text-chrome-icon hover:text-chrome-icon-hover hover:bg-overlay transition-colors cursor-default"
        title="Minimize"
        tabIndex={-1}
      >
        <Minus size={15} strokeWidth={1.75} />
      </button>
      <button
        data-tauri-drag-region="false"
        onClick={handleClose}
        className="w-[46px] h-full flex items-center justify-center text-chrome-icon hover:text-chrome-icon-hover hover:bg-chrome-close-hover transition-colors cursor-default"
        title="Close"
        tabIndex={-1}
      >
        <X size={15} strokeWidth={1.75} />
      </button>
    </div>
  );
}

/**
 * The Lime wordmark.
 *
 * `logo.svg` is authored on a 1254x1254 canvas with the artwork inside a thin
 * band, so `object-contain` renders it as a sliver. Cropping with `object-cover`
 * shows the whole mark; the numbers below frame it, not its aspect ratio.
 */
export function LimeLogo({ className = "" }: { className?: string }) {
  return (
    <img
      src="/logo.svg"
      alt="Lime"
      className={`object-cover object-[center_42%] pointer-events-none select-none ${className}`}
    />
  );
}

export interface TitleBarProps {
  /** Window name shown beside the logo. */
  title: string;
  /** Hide the wordmark, for windows that do not need one. */
  hideLogo?: boolean;
  /** Extra controls rendered before the caption buttons. */
  children?: React.ReactNode;
  /**
   * Replaces the default destroy-on-close with a custom behaviour. Windows that
   * must outlive a close (onboarding, for one) pass this to hide instead.
   */
  onClose?: () => void;
}

/**
 * The title bar every Lime window uses.
 *
 * It owns its own drag region and caption buttons, so a new window gets correct
 * window dragging and Windows-style minimise/close for free. Sizes, colours and
 * spacing all come from tokens; see `index.css`.
 */
export function TitleBar({ title, hideLogo, children, onClose }: TitleBarProps) {
  // Resolve once. `getCurrentWebviewWindow()` builds a new object per call, so
  // using it directly as an effect dependency would re-run effects every render.
  const appWindow = useMemo(() => getCurrentWebviewWindow(), []);

  return (
    <header
      data-tauri-drag-region
      onMouseDown={(e) => {
        // Only a bare drag moves the window; presses on controls must not.
        if (
          e.button === 0 &&
          !(e.target as HTMLElement).closest(
            "button, input, select, textarea, a, [data-no-drag], [data-tauri-drag-region='false']"
          )
        ) {
          appWindow.startDragging().catch(() => {});
        }
      }}
      className={TITLE_BAR_CLASS}
    >
      <div data-tauri-drag-region className="flex items-center gap-2 min-w-0">
        {hideLogo ? null : <LimeLogo className="w-[26px] h-[10px]" />}
        <span className="text-[13px] text-text-secondary pointer-events-none truncate">
          {title}
        </span>
      </div>

      <div className="flex items-center gap-3 h-full ml-auto">
        {children}
        <CaptionButtons onClose={onClose} />
      </div>
    </header>
  );
}

/**
 * Toggle switch.
 * Off: muted track, white knob. On: accent track, dark green knob.
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
        checked ? "bg-accent" : "bg-surface-track"
      }`}
    >
      <span
        className={`pointer-events-none inline-block h-[18px] w-[18px] transform rounded-full shadow transition duration-200 ease-in-out ${
          checked ? "translate-x-[18px] bg-accent-ink" : "translate-x-[2px] bg-text"
        }`}
      />
    </button>
  );
}

/** Keyboard hotkey pill — compact button style. */
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
          className="flex items-center gap-1 px-2.5 py-1 bg-surface-control hover:bg-surface-control-hover border border-line-strong text-[13px] text-text font-medium rounded-md transition-colors cursor-default"
        >
          {sublabel}
        </button>
      )}
      <button
        onClick={onClick}
        className={`flex items-center gap-1 px-2.5 py-1 border text-[13px] font-mono rounded-md transition-colors cursor-default ${
          isRecording
            ? "bg-accent/20 border-accent text-accent animate-pulse"
            : "bg-surface-control hover:bg-surface-control-hover border-line-strong text-text"
        }`}
      >
        {isRecording ? "Press keys..." : label}
      </button>
      {onReset && (
        <button
          onClick={onReset}
          className="p-1 text-text-muted hover:text-text hover:bg-overlay rounded-md transition-colors cursor-default"
          title="Reset to default"
        >
          <RotateCcw size={13} />
        </button>
      )}
    </div>
  );
}

/** Dropdown select — pill style. */
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
          <option key={opt.value} value={opt.value} className="bg-surface-control text-text">
            {opt.label}
          </option>
        ))}
      </select>
      <div className="flex items-center gap-1.5 px-2.5 py-1 bg-surface-control hover:bg-surface-control-hover border border-line-strong text-[13px] text-text rounded-md pointer-events-none transition-colors">
        {current?.icon && <span className="text-text-muted">{current.icon}</span>}
        <span>{current?.label}</span>
        <ChevronDown size={12} className="text-text-muted ml-0.5" />
      </div>
    </div>
  );
}

/** Segmented control. */
export interface SegmentedControlProps {
  options: { label: React.ReactNode; value: string; title?: string }[];
  value: string;
  onChange: (val: string) => void;
  className?: string;
}

export function SegmentedControl({ options, value, onChange, className = "" }: SegmentedControlProps) {
  return (
    <div
      role="radiogroup"
      className={`inline-flex items-center bg-surface-control p-[3px] rounded-[8px] border border-line-subtle select-none ${className}`}
    >
      {options.map((opt) => {
        const isActive = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            role="radio"
            aria-checked={isActive}
            title={opt.title}
            onClick={() => onChange(opt.value)}
            className={`px-3 py-[5px] text-[13px] font-medium rounded-[6px] transition-all cursor-default ${
              isActive
                ? "bg-surface-selected text-text shadow-sm"
                : "text-text-muted hover:text-text"
            }`}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}

/** Action button. */
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
    base +=
      " bg-accent hover:bg-accent-hover text-accent-ink font-semibold active:scale-[0.98]";
  } else if (variant === "danger") {
    base += " bg-negative-soft hover:bg-negative/25 text-negative border border-negative/20";
  } else {
    base += " bg-surface-control hover:bg-surface-control-hover text-text border border-line-strong";
  }

  return (
    <button onClick={onClick} disabled={disabled} className={`${base} ${className}`}>
      <span>{children}</span>
      {kbd && (
        <kbd className="text-[10px] bg-shadow-key text-text-secondary px-1.5 py-0.5 rounded font-mono ml-1">
          {kbd}
        </kbd>
      )}
    </button>
  );
}

/** Small state badge, e.g. Ready / Missing. */
export function StatusPill({ ready }: { ready: boolean }) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 text-[11px] font-medium px-2 py-[3px] rounded-full ${
        ready ? "text-positive bg-positive-soft" : "text-caution bg-caution-soft"
      }`}
    >
      <span
        className={`w-1.5 h-1.5 rounded-full ${ready ? "bg-positive" : "bg-caution"}`}
        aria-hidden="true"
      />
      {ready ? "Ready" : "Missing"}
    </span>
  );
}

/** A labelled, truncated path. Long paths must not break the row layout. */
export function PathRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline gap-2 min-w-0">
      <span className="text-[11px] text-text-muted shrink-0">{label}</span>
      <span
        className="text-[11px] text-text-muted/70 font-mono truncate"
        title={value}
      >
        {value}
      </span>
    </div>
  );
}

/**
 * Setting row.
 *
 * Rows sit directly on the window background with no card of their own; a
 * subtle bottom border separates them.
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
      className={`flex items-center justify-between min-h-[44px] py-[10px] px-4 border-b border-line-subtle last:border-b-0 gap-6 ${className}`}
    >
      <div className="flex-1 min-w-0">
        <div className="text-[13px] text-text font-normal leading-snug">{title}</div>
        {description && (
          <div className="text-[12px] text-text-muted mt-[2px] leading-snug">{description}</div>
        )}
      </div>
      <div className="shrink-0 flex items-center">{children}</div>
    </div>
  );
}

/**
 * Setting section: a small muted heading above a grouped-list card.
 */
export interface SettingSectionProps {
  title?: string;
  /** Optional supporting copy under the heading. */
  description?: string;
  children: React.ReactNode;
  className?: string;
}

export function SettingSection({
  title,
  description,
  children,
  className = "",
  first = false,
}: SettingSectionProps & { first?: boolean }) {
  return (
    <div className={`${className}`}>
      {title && (
        <h3
          className={`text-[12px] font-semibold uppercase tracking-wider text-text-muted mb-[6px] ${first ? "mt-0" : "mt-6"}`}
        >
          {title}
        </h3>
      )}
      {description && (
        <p className="text-[11px] text-text-muted mb-[8px] leading-relaxed -mt-1">{description}</p>
      )}
      <div className="bg-surface-raised rounded-[10px] overflow-hidden">{children}</div>
    </div>
  );
}