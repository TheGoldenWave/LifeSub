# LifeSub Interface Design Context

## Design Context

### Users

LifeSub is a native desktop tool for people who capture meetings, conversations, and personal spoken notes over long sessions, then return to searchable evidence and transcripts. Users need recording controls to remain understandable under time pressure and need source identity, permissions, failures, and recovery states to be trustworthy.

### Brand Personality

Calm, precise, and dependable. Copy is concise and concrete. The product should feel like a focused macOS work tool rather than a consumer media app or a promotional website.

### Aesthetic Direction

Use the established dark, restrained, utilitarian interface defined by `base.json`. Prefer compact native-feeling controls, clear hierarchy, stable dimensions, and semantic status colors. Avoid decorative gradients, glass effects, oversized type, floating page cards, playful ornament, and attention-seeking animation.

### Design Principles

1. Make capture truth visible: starting, recording, paused, disconnected, unavailable, and failed states must never share ambiguous controls.
2. Keep repeated workflows efficient: source selection, refresh, start, pause, resume, and stop should remain compact and keyboard accessible.
3. Fail explicitly: never hide permission, hardware, model, or persistence problems behind silent fallback.
4. Preserve context: show the saved device name when hardware is disconnected and keep long names readable without shifting the layout.
5. Use the token system: colors, spacing, typography, radii, shadows, and layer order come from `base.json` through the project CSS variables.
