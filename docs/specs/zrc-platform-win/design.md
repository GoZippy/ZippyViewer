# zrc-platform-win Design

Capture pipeline
- GDI fallback always available
- Prefer WGC or DXGI when available for speed and multi-monitor

Input pipeline
- SendInput for mouse/keyboard
- Optional raw input mapping for higher fidelity

Safety
- Clamp coordinates
- Avoid stuck keys (key-up recovery on disconnect)
