// Icon mapping for actions — renders emoji/text icons for now.
// In M1+, these will be replaced with SVG icon assets.

const ICON_MAP: Record<string, string> = {
  "format-png": "PNG",
  "format-jpg": "JPG",
  "format-webp": "WEB",
  "format-avif": "AVIF",
  "format-tiff": "TIFF",
  "format-bmp": "BMP",
  "format-heic": "HEIC",
  "format-pdf": "PDF",
  "crop": "✂",
  "compress": "⚡",
  "metadata": "🏷",
  "add-bg": "🖼",
  "remove-bg": "✨",
  "edit": "🎨",
  "annotate": "✏️",
  "redact": "⬛",
};

interface WedgeIconProps {
  icon: string;
  isHovered?: boolean;
  size?: number;
}

export function WedgeIcon({ icon, isHovered = false, size = 20 }: WedgeIconProps) {
  const label = ICON_MAP[icon] ?? icon.slice(0, 4).toUpperCase();
  return (
    <span
      style={{
        fontSize: size,
        color: isHovered ? "white" : "hsla(0,0%,90%,0.85)",
        lineHeight: 1,
        userSelect: "none",
      }}
    >
      {label}
    </span>
  );
}
