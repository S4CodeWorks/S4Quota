# S4Quota Main + Compact Surfaces

Mode: **Operate**

Approved composition: **Capacity Fold**

Approved decision comp:
`.impeccable/mocks/decision/surface-capacity-fold.png`

## Direction contract

**THESIS** — Personal quota is read as a present capacity and a known return
boundary, not as telemetry. The five-hour window owns the first reading; the
weekly window remains visibly attached as the secondary horizon.

**OWN WORLD** — Tidal Almanac supplies cadence, interrupted rules and temporal
markers without literal nautical imagery. Optical Index supplies strict
baselines and columns. Limiting Spine contributes language that explicitly
states which window governs the present moment, but no persistent spine.

**STORY** — Read remaining capacity, then reset, then compare the weekly
horizon, then inspect freshness or provider detail only when needed.

**FIRST VIEWPORT** — Main is one continuous ruled field. Compact is a separate
304 × 120 logical-pixel composition containing the small mark, remaining
percentage, reset countdown and a subordinate rail. Ordinary freshness is
silent in Compact.

**FORM** — Capacity Fold, selected from surface seed `b9f9a930`.

**FINISH** — Source Sans 3, lining tabular figures, one-pixel rules, 4 px
cadence track, 4 px control radii, no content shadows and no equal cards.

## Approved refinements

- Percentage and countdown share a baseline relationship without equal visual
  weight.
- `Currently limiting` is explicit in Main and omitted from normal Compact,
  where the five-hour context is intrinsic to the mode.
- Compact removes `remaining`, provider identity and ordinary freshness copy.
- The compact cadence rail remains: at 4 logical px it maps to 4, 5 and 6
  device pixels at 100%, 125% and 150% Windows scaling.
- A no-rail compact variant remains documented as an extreme-reduction option,
  not the default.
- Weekly is never a detached card and is not shown in normal Compact.

## Dimensions and adaptation

| Surface | Proposed size | Constraint |
| --- | --- | --- |
| Main minimum | `760 × 560` logical px | Same hierarchy; tighter vertical rhythm |
| Main default | `960 × 680` logical px | Reference composition |
| Main maximized | Fluid, content capped at `1184` px | More context, never a larger hero number |
| Compact | `304 × 120` logical px | First-class fixed composition |
| Compact minimum | `280 × 112` logical px | Rail retained; labels may not wrap |

At maximized width, rules and data bands may extend to the content cap. The
primary numeral remains 84 px. Additional width may expose real provider or
history detail later, but empty placeholder regions must not be invented.

## State semantics

- **Fresh:** no normal-state status copy in Compact; Main says provider and
  update time in its quiet operational row.
- **Refreshing:** retain the last valid values and add a brief activity label.
- **Stale/degraded:** retain last-known quota, add dashed treatment and explicit
  age/recovery text.
- **At limit:** objective `0%` state with reset still dominant; no invented
  warning threshold.
- **Needs authentication:** remove quota values and present the recovery action.
- **Unavailable/unsupported:** remove quota values and name the cause; never
  simulate zero.

## Composition inventory

| Ingredient | Commitment | Medium later |
| --- | --- | --- |
| Window chrome | 44 px Main, 26 px Compact; official small mark | Tauri chrome + existing PNG |
| Primary reading | 84 px Main / 44 px Compact, tabular | Semantic text/CSS |
| Reset countdown | 40 px Main / 24 px Compact, tabular | Semantic text/CSS |
| Cadence rail | 4 px Compact, 6 px Main, numeric extent only | Semantic HTML/CSS |
| Weekly horizon | Attached ruled band with 48 px value | Semantic HTML/CSS |
| Status language | Text plus rule/line/inversion treatment | Semantic HTML/CSS/SVG icon |
| Window controls | 16 px authored stroke icons in 32 px targets | Authored SVG |

## Non-literal commitments

The comp is not a pixel-tracing target. Window controls must use real Tauri
behavior, values come from canonical provider state, timestamps localize, and
text must survive Windows text/DPI scaling. The cadence rail is a capacity
extent, not a timeline chart, confidence score or fabricated projection.
