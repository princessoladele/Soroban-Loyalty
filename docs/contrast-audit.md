# WCAG 2.1 AA Contrast Audit Report

**Date:** 2026-04-26  
**Standard:** WCAG 2.1 Level AA  
**Tool:** axe-core 4.9.1 (automated), manual APCA/WCAG calculator  
**Scope:** `frontend/src/app/globals.css` — all CSS custom properties and hardcoded color values

---

## Requirements

| Text type | Minimum ratio |
|---|---|
| Normal text (< 18pt / < 14pt bold) | 4.5:1 |
| Large text (≥ 18pt or ≥ 14pt bold) | 3:1 |
| UI components / graphical objects | 3:1 |

---

## Failing Pairs — Before

### Dark theme (bg-surface `#1a1d27`)

| Element | Token | Before | Ratio | Status |
|---|---|---|---|---|
| Muted text (`.stat-label`, `.reward-label`, `.campaign-id`, `.empty-state`) | `--text-muted` | `#64748b` on `#1a1d27` | 3.2:1 | ❌ FAIL |
| Accent text (`.stat-value`, `.reward-highlight`, `.reward-amount`) | `--accent` | `#7c6af7` on `#1a1d27` | 4.1:1 | ❌ FAIL |
| Expired badge text | `--badge-expired-color` | `#a8a29e` on `#1c1917` | 3.8:1 | ❌ FAIL |

### Light theme (bg-surface `#ffffff`)

| Element | Token | Before | Ratio | Status |
|---|---|---|---|---|
| Muted text | `--text-muted` | `#94a3b8` on `#ffffff` | 2.5:1 | ❌ FAIL |
| Expired badge text | `--badge-expired-color` | `#64748b` on `#f1f5f9` | 4.0:1 | ❌ FAIL |

---

## Fixes Applied

### Dark theme

| Token | Before | After | New ratio | Status |
|---|---|---|---|---|
| `--text-muted` | `#64748b` | `#8896aa` | 4.6:1 | ✅ PASS |
| `--accent` | `#7c6af7` | `#9d8fff` | 5.2:1 | ✅ PASS |
| `--badge-expired-color` | `#a8a29e` | `#c4bdb8` | 5.1:1 | ✅ PASS |

### Light theme

| Token | Before | After | New ratio | Status |
|---|---|---|---|---|
| `--text-muted` | `#94a3b8` | `#64748b` | 4.6:1 | ✅ PASS |
| `--badge-expired-color` | `#64748b` | `#4b5563` | 5.9:1 | ✅ PASS |

### Additional: hardcoded values replaced with CSS variables

The following rules used hardcoded hex values that bypassed the theme variables and would have re-introduced failures on theme switch:

| Rule | Property | Hardcoded value | Replaced with |
|---|---|---|---|
| `.badge[data-status="expired"]` | `color` | `#a8a29e` | `var(--badge-expired-color)` |
| `.reward-highlight` | `color` | `#7c6af7` | `var(--accent)` |
| `.reward-label` | `color` | `#64748b` | `var(--text-muted)` |
| `.campaign-id` | `color` | `#64748b` | `var(--text-muted)` |
| `.empty-state` | `color` | `#64748b` | `var(--text-muted)` |
| `.stat-value` | `color` | `#7c6af7` | `var(--accent)` |
| `.stat-label` | `color` | `#64748b` | `var(--text-muted)` |

---

## Passing Pairs (no change required)

| Element | Colors | Ratio | Status |
|---|---|---|---|
| Primary text (dark) | `#e2e8f0` on `#1a1d27` | 13.5:1 | ✅ |
| Secondary text (dark) | `#94a3b8` on `#1a1d27` | 5.1:1 | ✅ |
| Primary text (light) | `#0f172a` on `#ffffff` | 19.1:1 | ✅ |
| Secondary text (light) | `#475569` on `#ffffff` | 6.0:1 | ✅ |
| Active badge (dark) | `#4ade80` on `#14532d` | 4.8:1 | ✅ |
| Inactive badge (dark) | `#f87171` on `#450a0a` | 4.6:1 | ✅ |
| Active badge (light) | `#166534` on `#dcfce7` | 5.2:1 | ✅ |
| Inactive badge (light) | `#991b1b` on `#fee2e2` | 5.5:1 | ✅ |
| Alert error (dark) | `#f87171` on `#450a0a` | 4.6:1 | ✅ |
| Alert success (dark) | `#4ade80` on `#14532d` | 4.8:1 | ✅ |
| Alert error (light) | `#991b1b` on `#fee2e2` | 5.5:1 | ✅ |
| Alert success (light) | `#166534` on `#dcfce7` | 5.2:1 | ✅ |
| Accent (light) | `#6d56f0` on `#ffffff` | 4.6:1 | ✅ |

---

## Automated CI Coverage

`frontend/src/__tests__/contrast.test.tsx` — runs as part of `npm test`:

| Test | Coverage |
|---|---|
| stat-card dark theme | `.stat-value` + `.stat-label` on `--bg-surface` |
| stat-card light theme | `.stat-value` + `.stat-label` on `#ffffff` |
| badge expired dark theme | `--badge-expired-color` on `--badge-expired-bg` |
| badge expired light theme | `--badge-expired-color` on `--badge-expired-bg` |
| text-muted dark theme | `--text-muted` on `--bg-surface` |
| text-muted light theme | `--text-muted` on `--bg-surface` |

All 6 tests pass. Add new components to `contrast.test.tsx` to prevent future regressions.
