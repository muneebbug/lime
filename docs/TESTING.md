# Wheel — Testing Matrix

## Automated Tests

### Unit Tests (CI)
```
cargo test -p wheel-core
```
Covers:
- `output.rs`: resolve_output_path with various policies
- `action.rs`: registry filtering, accepts_extension
- `settings.rs`: schema migration (v0 → v1)

### Frontend Tests (CI, planned M1)
```
pnpm --filter desktop test
```
Covers:
- wheelStore: state transitions
- RadialWheel: wedge hit-testing math
- WheelOverlay: event handling integration

### Integration Tests (CI, planned M2)
Fixture-based image conversion tests in `wheel-engines`:
```
cargo test -p wheel-engines -- --test-threads=4
```
Tests each convert format with a 100x100 fixture PNG.

---

## Manual Test Matrix (M0)

### Sources

| Source | Shift+Drag | Overlay appears | Files extracted | Source untouched |
|--------|-----------|-----------------|-----------------|------------------|
| Explorer window | ✅ | ✅ | ✅ | ✅ |
| Desktop icons | TODO | | | |
| Chrome download bar | TODO | | | |
| Outlook attachment | TODO | | | |
| Teams/Slack file | TODO | | | |
| Virtual files (CFSTR_FILEDESCRIPTOR) | TODO | | | |

### DPI / Monitor

| Scenario | Passes |
|----------|--------|
| 100% DPI single monitor | ✅ |
| 125% DPI single monitor | TODO |
| 150% DPI single monitor | TODO |
| Multi-monitor same DPI | TODO |
| Multi-monitor mixed DPI (e.g. 100% + 150%) | TODO |
| Monitor on the right (clamping) | TODO |
| Monitor on the left (negative coordinates) | TODO |

### Trigger Behavior

| Test | Expected |
|------|----------|
| Shift + drag < threshold (2px) | No overlay |
| Shift + drag > threshold (8px) | Overlay shown |
| Drag without Shift | No overlay (default settings) |
| Escape during drag | Overlay hidden, source untouched |
| Release outside wheel | Overlay hidden, no action dispatched |
| Multiple rapid drags | Only one overlay shown, no stuck state |
| Pause via tray | No overlay shown |
| Resume via tray | Overlay works again |

### Elevated App Source

| Test | Expected |
|------|----------|
| Drag from elevated Explorer | Hook detects drag, overlay shown |
| OLE DragEnter from elevated process | May fail — should show explanation |

### Edge Cases

| Test | Expected |
|------|----------|
| Very fast drag + drop | Action dispatched correctly |
| Drag 10+ files | Multi-file action dispatched |
| Unsupported file type | Relevant wedges hidden (M1) |
| Session lock then unlock | Hooks re-installed, overlay functional |
| Sleep/resume | Hooks re-installed |
| App exit during drag | No crash, source file untouched |

### Conversion (M2)

| From → To | Quality | Speed | Metadata |
|-----------|---------|-------|----------|
| PNG → WEBP | ✅ visual OK | TODO | TODO |
| PNG → JPG  | ✅ | TODO | TODO |
| JPG → PNG  | ✅ | TODO | TODO |
| JPG → AVIF | TODO | | |
| HEIC → JPG | TODO | | |
| PDF → PNG  | TODO | | |
| IMG → PDF  | TODO | | |

---

## CI Configuration

See `.github/workflows/ci.yml` (created in M7).

Steps:
1. `cargo fmt --check`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo test --workspace`
4. `pnpm --filter desktop typecheck`
5. `pnpm --filter desktop lint`
6. `pnpm tauri build` (artifact)
