# DockPit Design System & Style Guide

## 1. Farbsystem

### Hintergründe
| Variable | Dark Mode | Light Mode | Verwendung |
|----------|-----------|------------|------------|
| `--bg-0` | `#080a0f` | `#f5f7fa` | Seiten-Hintergrund |
| `--bg-1` | `#0d1017` | `#ffffff` | Inputs, Sidebar |
| `--bg-2` | `#141820` | `#f0f2f5` | Cards, Panels |
| `--bg-3` | `#1a1f2b` | `#e6e9ef` | Erhöhte Flächen |
| `--bg-hover` | `#1f2535` | `#dde1e8` | Hover-Zustand |

### Borders
| Variable | Dark Mode | Light Mode |
|----------|-----------|------------|
| `--border` | `#252b3a` | `#d4d8e0` |
| `--border-light` | `#323a4e` | `#c0c6d2` |

### Text
| Variable | Dark Mode | Light Mode | Verwendung |
|----------|-----------|------------|------------|
| `--text` | `#e4e7ee` | `#1a1f2b` | Primärer Text |
| `--text-secondary` | `#8892a8` | `#555e74` | Sekundärer Text |
| `--text-muted` | `#555e74` | `#8892a8` | Hints, Placeholder |

### Accent & Status
| Variable | Dark Mode | Light Mode | Verwendung |
|----------|-----------|------------|------------|
| `--accent` | `#4f8cff` | `#3b74e0` | Primäre Aktionen |
| `--green` | `#2dd4a0` | `#16a87a` | Erfolg, Starten |
| `--red` | `#f06060` | `#d94444` | Fehler, Löschen |
| `--yellow` | `#f0b840` | `#c99a20` | Warnung, Stoppen |
| `--purple` | `#a78bfa` | `#7c5cbf` | Recreate, Spezial |

---

## 2. Typografie

- **Font:** `-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Inter', sans-serif`
- **Antialiasing:** `-webkit-font-smoothing: antialiased`

| Verwendung | Klasse | Größe |
|------------|--------|-------|
| Seiten-Titel | `text-[15px] font-semibold` | 15px |
| Card-Titel | `text-sm font-semibold` | 14px |
| Body-Text | `text-sm` | 14px |
| Small Text | `text-xs` | 12px |
| Tiny/Labels | `text-[11px]` | 11px |
| Micro | `text-[10px]` | 10px |
| Kategorie-Labels | `text-[10px] font-semibold uppercase tracking-wider` | 10px |

---

## 3. Button-Varianten

**Immer `<Button>` Komponente verwenden.** Keine manuellen Button-Styles.

| Variant | Funktion | Beispiele |
|---------|----------|-----------|
| `primary` | Bestätigen, Speichern, Anmelden | Speichern, Bestätigen, Schliessen |
| `danger` | Löschen, Entfernen | Container löschen, Image entfernen |
| `warning` | Stoppen, Pausieren | Container stoppen |
| `success` | Starten, Aktivieren | Container starten |
| `secondary` | Neutrale Aktionen, Abbrechen | Abbrechen, Test senden, Aktualisieren |
| `ghost` | Subtile Aktionen | Icon-Buttons, Inline-Aktionen |
| `purple` | Spezial-Aktionen | Recreate |

### Größen
| Size | Padding | Font | Verwendung |
|------|---------|------|------------|
| `sm` | `px-2.5 py-1` | 11px | Tabellen-Aktionen, Inline |
| `md` | `px-4 py-2` | 12px | Standard |
| `lg` | `px-5 py-2.5` | 14px | Formular-Submit |

### Wichtige Regeln
- **Abbrechen ist NICHT rot** — verwende `secondary`
- **Löschen ist IMMER rot** — verwende `danger`
- Loading-Zustand: `loading={true}` zeigt Spinner

---

## 4. Formular-Elemente

### TextInput
```svelte
<TextInput bind:value={name} label="Name" placeholder="Eingabe..." />
<TextInput bind:value={pw} type="password" label="Passwort" error={pwError} />
```

### CustomSelect (statt `<select>`)
```svelte
<CustomSelect options={[{value: '1', label: 'Option 1'}]} value={selected} onchange={handleChange} />
```

### CustomCheckbox (statt `<input type="checkbox">`)
```svelte
<CustomCheckbox checked={enabled} onchange={(v) => enabled = v} label="Aktivieren" />
```

### Tabs (statt manuelle Tab-Buttons)
```svelte
<Tabs tabs={[{id: 0, label: 'Tab 1'}]} active={activeTab} onchange={(id) => activeTab = id} />
```

---

## 5. Spacing

| Verwendung | Wert |
|------------|------|
| Seiten-Padding | `p-4 md:p-5` |
| Card-Padding | `p-5` |
| Card-Header | `px-5 py-4` (mit border-b) |
| Formular-Gaps | `space-y-4` |
| Grid-Gaps | `gap-3` |
| Button-Gaps | `gap-2` |
| Inline-Gaps | `gap-1.5` |

---

## 6. Border Radius

| Token | Wert | Verwendung |
|-------|------|------------|
| `--radius-sm` | 6px | Buttons (sm), Badges |
| `--radius-md` | 8px | Buttons, Inputs, Dropdowns |
| `--radius-lg` | 12px | Cards, Modals, Dropdown-Panels |
| `--radius-xl` | 16px | Login-Card, große Modals |

---

## 7. Dark/Light Mode Regeln

1. **Niemals** Hex-Farben direkt verwenden — immer CSS-Variablen
2. **Alle** Elemente müssen in beiden Modi getestet werden
3. Schatten: Dark = subtil mit Glow, Light = stärker mit Elevation
4. Glassmorphism: `bg-[var(--glass-bg)] backdrop-blur-xl border-[var(--glass-border)]`
5. Der Theme-Toggle sitzt in der Topbar

---

## 8. Mobile Responsive

### Breakpoints
| Prefix | Min-Width | Verwendung |
|--------|-----------|------------|
| (none) | 0px | Mobile-First |
| `sm:` | 640px | Kleine Tablets |
| `md:` | 768px | Tablets, Sidebar sichtbar |
| `lg:` | 1024px | Desktop |
| `xl:` | 1280px | Große Screens |

### Regeln
- **Touch-Targets:** Mindestens 44x44px auf Mobile
- **Modals:** Bottom-Sheet auf Mobile (`items-end sm:items-center`)
- **Tabellen:** `overflow-x-auto` mit `hidden md:table-cell` für optionale Spalten
- **Sidebar:** Fixed mit Overlay auf Mobile, Static ab `md:`

---

## 9. Animationen

| Effekt | Dauer | Easing | Verwendung |
|--------|-------|--------|------------|
| Farb-Transition | 200ms | ease | Hover/Focus-States |
| Modal öffnen | 250ms | ease-out | Scale + Fade |
| Dropdown öffnen | 150ms | ease-out | Scale + Fade |
| Toast erscheinen | 250ms | ease-out | Slide-in |
| Checkbox Check | 200ms | ease | SVG Draw |
| Tab-Indicator | 300ms | ease-out | Slide |

### Hover-Glow (futuristisch)
```css
hover:shadow-[var(--shadow-glow)]
```

---

## 10. Glassmorphism

Verwende Glassmorphism sparsam für:
- Dropdown-Panels
- Toast-Benachrichtigungen
- Modal-Overlays (Backdrop)

```svelte
<div class="bg-[var(--glass-bg)] backdrop-blur-xl border border-[var(--glass-border)] shadow-[var(--shadow-lg)]">
```

**Nicht verwenden für:** Sidebar, Tabellen, reguläre Cards, Formulare.

---

## Komponenten-Import

```svelte
import Button from '$lib/components/ui/Button.svelte';
import TextInput from '$lib/components/ui/TextInput.svelte';
import CustomSelect from '$lib/components/ui/CustomSelect.svelte';
import CustomCheckbox from '$lib/components/ui/CustomCheckbox.svelte';
import Tabs from '$lib/components/ui/Tabs.svelte';
import Modal from '$lib/components/ui/Modal.svelte';
import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
import Badge from '$lib/components/ui/Badge.svelte';
import Pagination from '$lib/components/ui/Pagination.svelte';
import Toast from '$lib/components/ui/Toast.svelte';
```
