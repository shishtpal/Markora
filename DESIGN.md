# Workbench design system

Workbench is a task and operations workspace for tasks, schedules, products,
categories, clients, team members, and reports. This document describes the
implemented interface and its interaction requirements.

## Visual direction

Keep the cobalt-and-white technical workspace: flat surfaces, thin rules,
restrained corners, precise alignment, and named Lucide icons. Use spacing and
typography to establish hierarchy. Secondary controls and nested cards use
quiet borders; reserve strong cobalt for interaction, focus, and selection.
Avoid gradients, decorative artwork, glows, and oversized welcome sections.

## Task-first hierarchy

- Home starts with overdue/today/tomorrow tasks and recent work. Summary
  metrics follow; the seven-day chart and quick notes are supporting content.
- The attention list excludes completed tasks and orders by due date. Headings
  describe the actual time range, and chart totals explicitly say all-time.
- Keep task titles readable and allow two lines rather than squeezing them
  between metadata and icon actions. Expose secondary details in the drawer.
- Board toolbars keep task creation visible. Search has a separate compact
  row; entity filters use a disclosure and import/export use the Data menu.

## Colors

Use semantic tokens from src/main.css. Light/dark roles are paired:

| Role | Light | Dark |
| --- | --- | --- |
| Canvas | #f4f7fc | #101722 |
| Surface | #ffffff | #172231 |
| Muted surface | #eef3fb | #1d2a3b |
| Main text | #10234b | #e8efff |
| Supporting text | #5c6f91 | #a5b5d1 |
| Divider | #e0e7f2 | #33445e |
| Panel frame | #bac9df | #526b91 |
| Accent | #1246c5 | #6f9bff |
| Complete/saved | #08734f | #8de5c2 |
| Error | #e34254 | #ff7581 |
| Attention | #e5a719 | #ffd064 |

Status always has a text label. Charts use light/dark-aware colors and solid
versus dashed lines, with legible legends and axis ticks in both themes.
The chart component's immediate wrapper must fill the explicit height of its
container; the canvas must never paint over the next panel.

## Typography

Fonts are bundled in src/assets/fonts with their licenses; no font CDN is used.
Space Grotesk serves the interface, IBM Plex Mono serves numeric data, and
Noto Sans Devanagari serves Hindi glyphs. Use sentence case for body text,
controls, and descriptions; uppercase is limited to short section headings.

The root font size follows the 16–20px user preference. Use rem typography
throughout components and teleported dialogs so task titles, form controls,
and tables scale together. At the default size, task titles are about 15px,
controls 15–16px, supporting text 13–14px, and compact metadata 11–12px.
Metric labels wrap instead of truncating. Page headings stay compact.

## Responsive shell

- The content width is at most 1440px. Panels may scroll internally; the page
  must not scroll sideways at 360px, 390px, 768px, 1024px, or 1280px.
- Desktop navigation shows Home, Board, Schedule, and Report. A labelled More
  disclosure exposes Products, Categories, Team, Clients, Settings, and Support.
  No route depends on an undisclosed horizontal navigation scroller.
- Phones/tablets use the bottom route bar and a grid menu for all routes.
  Search remains reachable from a visible magnifying-glass button.
- Phone controls have at least 44px touch targets. The selection bar stays
  above the fixed bottom navigation and respects the safe-area inset.
- Phone schedules start in Agenda. Desktop and phone view preferences are
  stored separately. Month view on phones uses event counts that open the day
  view instead of displaying tiny event cards. Events can also be added and
  edited using explicit controls; drag-and-drop is optional.
- Wide directory tables scroll inside their panels.

## Interaction and accessibility

Use shared buttons, fields, panels, chips, empty states, and the selection bar.
All visible controls must have working handlers. Status selects apply the
chosen status directly; advancing a task is a separate action. Schedule search
filters the event title and description. Bulk status changes update every
selected entity and clear the selection after success.

ModalPanel and the task drawer share useModalDialog: focus enters the dialog,
Tab stays inside, background content becomes inert, Escape closes the topmost
dialog, scrolling is locked, and focus returns to the trigger. Closing or
navigating away from an edited form asks before discarding unsaved changes.
Successful saves do not ask for discard confirmation. Drafts also protect
against accidental browser unload. Errors are visible in the UI.

Use a visible focus ring, text labels for state, named controls, and reduced
motion preferences. Keep English and Hindi locale keys in parity. Navigation,
route titles, section headings, form labels, date labels, and user feedback use
localized messages; user-authored data is preserved as entered.

## Verification

Before delivery, run locale parity, type checking, production build, and the
Playwright smoke/regression suite. Inspect desktop and phone screenshots,
including dark mode, Hindi, and enlarged text. Confirm chart bounds, navigation
reachability, search, status updates, focus restoration, draft protection, and
bulk changes in an isolated browser profile.
