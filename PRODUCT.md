# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

The primary user is an individual who uses AI agents and tools intensively in
their own work and needs to understand and manage the capacity available to
them throughout the day and week. S4Quota v1 is not designed for team managers,
organizational administration, or workforce consumption monitoring.

## Product Purpose

S4Quota is a Windows desktop utility from S4CodeWorks that makes personal AI
tool quotas continuously understandable without requiring the user to leave
their work. It should answer, at a glance:

- how much capacity remains;
- when that capacity returns;
- which quota window is closest to becoming limiting;
- whether the current work pace is sustainable;
- whether the displayed state is current and trustworthy.

Success means the user can make those decisions quickly, confidently, and with
minimal interruption while the utility remains visible for long periods.

## Positioning

S4Quota translates provider-specific usage limits into a calm, persistent view
of personal working capacity. Its differentiator is not raw telemetry; it is the
continuous relationship between remaining capacity, reset time, limiting
window, and data confidence across providers.

## Operating Context

The initial product is a Tauri desktop application for Windows. It runs beside
the user's primary work, often remaining visible for hours. The Main Surface
supports complete reading and future expansion. The Compact Surface is a
first-class, glanceable floating utility focused on the five-hour quota and its
reset countdown; it is not a responsive reduction of the Main Surface.

The current provider is Codex. The Rust backend reads real quota data through
the Codex App Server, normalizes it into a provider-agnostic domain, and
publishes canonical provider state to the frontend.

## Capabilities and Constraints

- Initial quota windows include five-hour and weekly limits, with room for
  additional or unknown provider windows.
- Availability, authentication, compatibility, refreshing, degraded data, and
  other lifecycle states are explicit product information.
- Data freshness and reliability must remain legible; stale data must never
  appear silently current.
- Future multi-provider support is expected, but must not distort the v1
  hierarchy or introduce administrative dashboard patterns.
- Main and Compact are related surfaces with distinct compositions.
- The validated Rust provider, process supervision, IPC, and backend
  architecture are outside the scope of visual-system work.
- Final Main and Compact surfaces remain deliberately unimplemented during the
  Design System foundations phase.

## Brand Commitments

The product name is S4Quota and the maker is S4CodeWorks. The official marks are
`assets/branding/s4quota-mark-primary.png` and
`assets/branding/s4quota-mark-small.png`; they must not be redrawn or modified.

The identity is monochromatic, minimal, contemporary, precise, calm, and
refined without luxury signaling. It must avoid cyberpunk, terminal and hacker
motifs, neon, decorative glow or glass, AI-category color clichés, generic SaaS
dashboard composition, and imitation of another software brand. Light and Dark
themes belong to one semantic system.

## Evidence on Hand

- Two official raster brand marks with transparent backgrounds.
- A working Tauri 2, React, TypeScript, and Rust application foundation.
- A production Codex provider and provider-agnostic quota domain.
- A sanitized provider spike and technical report under `spike/`.
- Real five-hour and weekly quota data validated on Windows.
- No approved final product surface or incumbent Design System; the current
  frontend is a disposable technical probe.

## Product Principles

1. Capacity before telemetry: communicate what the user can still do, not merely
   what the provider emitted.
2. Trust is visible: freshness, degraded operation, and uncertainty are part of
   the primary experience.
3. Glance first, depth on demand: frequent decisions must resolve peripherally;
   detail remains available without crowding the persistent view.
4. Personal utility, not administration: optimize for one person's working
   rhythm rather than organizational comparison or reporting.
5. Provider-agnostic without abstraction theater: accommodate future providers
   while keeping today's language direct and specific.

## Accessibility & Inclusion

The interface must provide sufficient contrast, visible keyboard focus,
keyboard navigation, usable hit targets, meaningful disabled states, reduced
motion behavior, and legibility across Windows scaling and DPI settings.
Numbers and countdowns must remain stable and readable as values change.
