# S4Quota Design System

<!-- impeccable:design-system 1 -->

## Direction

**Tidal Almanac** is the primary visual direction, strengthened by the optical
discipline of **Optical Proof**.

S4Quota treats quota as cyclical personal capacity: a measurable level at the
present moment, a limiting window, and a known return point. The system borrows
the cadence, level bands, and temporal markers of an almanac without borrowing
nautical imagery. It must read as a precise desktop instrument, never as a
maritime application, scientific console, financial terminal, or monitoring
dashboard.

The system is recognizable without its mark through five recurring traits:

1. stable tabular numbers on shared baselines;
2. ruled bands instead of collections of floating cards;
3. a visible `now` marker and an explicit reset boundary;
4. one dominant limiting condition while context remains present;
5. restrained, interrupted geometry: rules pause for labels and data markers
   break a continuous rail without becoming decorative rings.

## Decision Principles

- **Capacity before telemetry.** Remaining capacity and return time outrank raw
  provider data, charts, and metadata.
- **Trust is content.** Freshness, degradation, and uncertainty are shown near
  the value they qualify; stale data never looks current.
- **Cadence must be real.** Curves and temporal rhythm may appear only when they
  encode actual product data. Ornamental waveforms are prohibited.
- **Hierarchy is tonal.** Monochrome status uses weight, inversion, line style,
  icon shape, and explicit language—not hidden hue conventions.
- **Compact is composed, not collapsed.** It shares foundations with Main but
  has its own information hierarchy and spacing.
- **One field, not a dashboard.** Prefer continuous surfaces, ruled sections,
  and aligned rows. A card must represent a genuinely independent object or
  floating layer, not merely group nearby content.

## Brand Assets

The canonical files remain:

- `assets/branding/s4quota-mark-primary.png`
- `assets/branding/s4quota-mark-small.png`

Use the primary mark at ordinary brand sizes and the small optical mark in tray,
titlebar, and similarly constrained contexts. Do not edit, regenerate, trace,
or bake either asset into a new file. Dark-theme presentation may invert the
rendered monochrome asset without modifying its source. The marks do not become
progress rings, loading indicators, or chart geometry.

## Color

The strategy is **restrained monochrome**. A subtly warm neutral scale avoids
both sterile blue-gray software and cream editorial styling. There is no brand
accent color in v1.

Light and Dark share semantic roles; they are not separate component themes.
Product code consumes semantic variables from
`src/design-system/tokens.css`, never raw neutral steps.

| Role | Light | Dark | Purpose |
| --- | --- | --- | --- |
| Canvas | `#F4F4F0` | `#0F100E` | Window ground |
| Surface | `#FAFAF7` | `#171816` | Primary working field |
| Subtle surface | `#E9EAE5` | `#20211E` | Quiet grouping and tracks |
| Raised surface | `#FFFFFF` | `#292A26` | Popovers and protected focus only |
| Primary text | `#171816` | `#F4F4F0` | Values, headings, actions |
| Secondary text | `#4C4E47` | `#B7B9B1` | Labels and explanations |
| Tertiary text | `#666961` | `#9A9D94` | Metadata and timestamps |
| Default border | `#D1D3CC` | `#343630` | Controls and section rules |
| Strong fill | `#171816` | `#F4F4F0` | Primary action and limiting state |

Primary, secondary, and tertiary text pairs meet WCAG AA for normal text on
their canvas/surface roles. Disabled content may use lower contrast, but it
must remain recognizable as content and may never be the only explanation of
why an action is unavailable.

### Status without color

- **Current/healthy:** ordinary surface, solid mark, explicit freshness text
  only when useful.
- **In progress:** preserve the last valid value; reduce its emphasis slightly
  and show a small activity affordance. Never blank stable data for refresh.
- **Attention/authentication:** stronger border plus explicit label and action.
- **Critical/limited:** inverse neutral field plus icon and text; never rely on
  inversion alone.
- **Stale/degraded:** dashed boundary or rail, timestamp, and recovery state.
- **Unsupported/unavailable:** no simulated quota. State the cause and next
  meaningful action.

## Typography

The single product family is **Source Sans 3 Variable**, self-hosted through
`@fontsource-variable/source-sans-3` under OFL-1.1.

It was chosen for small-size legibility, humanist construction, stable UI
metrics, useful variable weights, and tabular figures. A single family keeps
Main and Compact related and avoids an editorial display face that would make
the product feel promotional. The trade-off is a small bundled font payload in
exchange for consistent rendering across supported Windows versions.

Use weights 400, 500, and 600. Weight 700 is intentionally absent from the
normal hierarchy; importance comes from scale, placement, and contrast before
heaviness.

| Token | Size / line | Typical use |
| --- | --- | --- |
| `2xs` | 11 / 14 | Timestamps and tertiary metadata |
| `xs` | 12 / 15 | Labels and status |
| `sm` | 13 / 16 | Dense supporting text |
| `md` | 14 / 20 | Default desktop UI |
| `lg` | 16 / 20 | Emphasized labels |
| `xl` | 20 / 23 | Section titles |
| `2xl` | 28 / 32 | Compact primary number / page value |
| `3xl` | 40 / 40 | Main primary value |
| `4xl` | 56 / 56 | Exceptional primary quota reading |

Headings use at most `-0.025em` tracking. Labels use sentence case; uppercase
is reserved for acronyms and very short system abbreviations, not as a general
visual texture.

## Numbers

Quota figures use lining tabular numerals through
`font-variant-numeric: tabular-nums lining-nums`. Width must not change when a
countdown or percentage updates.

- Remaining percentage is primary; the percent sign is part of the value but
  may use roughly half the numeral size.
- Countdown is secondary and uses a stable format appropriate to its horizon:
  `04:18`, `2h 14m`, or `6d 08h`. Do not switch formats mid-countdown unless the
  horizon crosses a meaningful threshold.
- Reset date/time is explicit when a countdown alone could be ambiguous.
- Labels align optically to the numeral baseline, not the outside of its box.
- Numeric updates crossfade or change in place. Never use slot-machine rolls,
  bounces, or width-changing transitions.
- Unknown provider values remain unknown; do not coerce them to zero.

## Spacing and Density

The base rhythm is 4 px with 2 px optical corrections. Canonical steps are
`2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48, 64`.

- Tight label/value groups use 2–8 px.
- Related controls and rows use 8–16 px.
- Section separation uses 24–32 px.
- Major surface regions use 40–64 px when the window permits.
- Space above a title is always greater than space below it.

Main defaults to comfortable density: 36 px controls and 44 px data rows.
Compact defaults to 32 px controls/rows. Pointer hit targets are at least 32 px;
coarse-pointer environments increase them to 44 px through tokens.

## Geometry, Borders, and Elevation

S4Quota is near-square, not sharp and not pill-shaped.

- Radius scale: `0, 2, 4, 6, 8, 12` px.
- Controls normally use 4 px; working surfaces use 0–6 px.
- Pills are reserved for compact statuses and binary selections whose shape
  communicates grouping.
- Hairlines are 1 px. Keyboard focus is 2 px with a one-pixel contrasting halo.
- Default hierarchy uses surface tone plus a border, not shadow plus border.
- Shadows are reserved for genuine elevation: the OS window boundary, popover,
  menu, or protected modal layer. Nested content does not cast shadows.

Separators may pause around a label or current marker. This controlled break is
a signature detail derived from the brand geometry; random dashed decoration is
not.

## Cadence and Data Graphics

The foundational data primitive is the **cadence rail**: a horizontal measured
track with remaining/used extent, a `now` marker, and a reset boundary. It is
more compact and direct than a chart.

Level bands—low, mid, high—are useful when their thresholds are explained or
provider data supports them. They use neutral tone and labels, never green,
amber, and red substitutes.

Curves are permitted only when real history or a real temporal projection is
available. A curve must answer a user question such as “at this pace, which
window becomes limiting first?” If it cannot, use a rail, value, or timestamp.
Do not infer a smooth curve from two snapshots.

## Motion

Motion confirms response and preserves spatial understanding:

- press: 80 ms;
- hover: 120 ms;
- state/value change: 180 ms;
- surface enter/exit: up to 240 ms;
- easing: `cubic-bezier(0.2, 0, 0, 1)` for ordinary state changes.

Allowed moments include a marker settling to a newly reconciled value, a menu
entering, or a stale state becoming current. Progress may interpolate only when
the old and new readings are both valid. No springs, bounce, chase lights,
ambient breathing, continuous waves, or decorative chart drawing.

With `prefers-reduced-motion: reduce`, transitions collapse to 1 ms and no
meaning depends on animation.

## Interaction States

Every interactive primitive supports default, hover, pressed, focus-visible,
disabled, and busy where relevant.

- Hover changes one tonal step or border strength; it does not lift every item.
- Press moves at most one physical pixel and returns immediately.
- Focus-visible is always drawn and never replaced by hover styling.
- Disabled controls keep their label and provide contextual explanation when
  the reason is not obvious.
- Busy controls retain their width and label where possible.
- Selected state is distinct from keyboard focus.

Keyboard order follows visual order. Icon-only controls require accessible
names and a tooltip when the action is not universally understood.

## Primitive Set

Foundational CSS recipes live in `src/design-system/` and do not constitute a
finished product surface.

- **Surface:** continuous canvas, subtle band, or true raised overlay.
- **Section / Rule:** establishes hierarchy without card containers.
- **Text / Label / Caption:** semantic hierarchy using one family.
- **NumericValue:** stable tabular value with optional scaled unit.
- **Action / IconAction:** primary, secondary, and disabled behavior.
- **Field:** shared border, focus, and disabled grammar.
- **Status:** neutral, attention, critical, and stale treatments with text.
- **CadenceRail:** measured capacity track, marker, and freshness treatment. Its
  `--s4-cadence-ratio` input is a unitless value from `0` to `1`, so updates can
  use a composited transform instead of animating layout.
- **DataBand:** aligned label/value row; the limiting window may strengthen its
  leading rule rather than becoming a colored card.

Composition favors adjacent ruled regions and shared baselines. Components may
nest only when their semantic ownership nests; visual convenience is not enough.

No component library is committed yet. Native HTML behavior should be preferred
for simple controls. A headless accessibility library may be selected later for
dialogs, menus, or composite widgets after the exact surface requirements are
known. It must not dictate styling.

No icon library is committed yet. The icon grammar is 16 or 20 px, 1.5 px
optical stroke, `currentColor`, open shapes, and no surrounding icon tile.
Filled icons are reserved for selected or critical state. Library selection is
deferred until the real Main/Compact icon inventory can be compared optically.

## Main Surface Principles

- Use one continuous working field divided by cadence bands and rules.
- Show the limiting window first without hiding the other window.
- Keep remaining capacity and reset time visible before provider details.
- Detailed freshness, provider state, and future history can occupy supporting
  bands; they must not create a grid of equal cards.
- Maximization increases useful context and data resolution, not merely element
  size or empty margins.
- A chart appears only when there is actual history or a defensible projection.

## Compact Surface Principles

Compact is a distinct composition:

1. remaining percentage for the five-hour window;
2. reset countdown;
3. supporting state only when it affects trust or action.

The default compact view should not chart both windows. At most, a single
cadence rail may support the dominant number when it stays legible. Weekly quota
belongs behind a deliberate expansion, alternate view, or exceptional warning
unless future research proves it is frequently needed at a glance.

The window must remain quiet when always-on-top. Avoid persistent animation,
large logos, ornamental graphs, redundant labels, and background elevation
inside the window. Optical balance is evaluated at real Windows scaling levels,
not only at 100% DPI.

## Accessibility

- Normal text targets at least 4.5:1 contrast; large text targets 3:1.
- Focus-visible is never removed and uses both a 2 px ring and contrasting halo.
- Meaningful state is expressed by text plus shape/line treatment, never tone
  alone.
- Pointer targets are at least 32 px on desktop and 44 px for coarse pointers.
- Numerals remain stable under updates and at Windows text/DPI scaling.
- Theme follows the OS by default; a future manual override sets
  `data-theme="light|dark"` without component-specific branches.
- High-contrast/forced-colors support must be verified when surfaces exist;
  primitives must not suppress system colors without an equivalent fallback.

## Prohibited Drift

- No literal water, tides, waves, maps, nautical icons, or almanac decoration.
- No decorative waveform, sparkline, ring, or progress visualization.
- No color-coded provider/state system in v1.
- No terminal, oscilloscope, cyberpunk, industrial-hazard, or hacker styling.
- No giant metric used merely to fill an otherwise empty window.
- No equal-weight dashboard cards or nested cards.
- No blur/glass, gratuitous gradient, glow, or soft shadow on content.
- No manual Light/Dark styling inside components when a semantic token exists.

## Open Until Surface Composition

- Exact Main and Compact dimensions and breakpoints.
- Final icon library or authored icon subset.
- Whether historical data justifies a real curve in Main.
- The precise placement of brand marks within window chrome.
- Provider-switching interaction once a second provider becomes real.

These remain open because deciding them from foundations alone would constrain
the product before the two first-class surfaces are composed comp-first.
