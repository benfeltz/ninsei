# Human Interface Guidelines (1992) — Review Checklist (Derived)

## How to use
- Treat this as a review aid for classic desktop-style UIs.
- Check items as “Yes / No / N/A” during design review or usability testing.
- This is paraphrased and reorganized from the book’s evaluation checklist; consult the PDF for exact wording and edge cases.

---

## General
- [ ] The product matches the classic desktop **look** (desktop/workspace, windows, menus, standard controls).
- [ ] The product matches the classic desktop **feel** (pointing, selecting, keyboard behavior).
- [ ] Any metaphor used is appropriate, visually/behaviorally consistent, and doesn’t require users to “remember the mapping.”
- [ ] Every command/action provides observable feedback; long work shows progress and completion.
- [ ] The UI supports see-and-point discovery (objects/actions are findable on-screen).
- [ ] Standard behaviors/terms match user expectations from common desktop applications.
- [ ] Output is WYSIWYG where users expect it (especially printing).
- [ ] When an action cannot be performed, the UI explains why in user terms and suggests what to do.
- [ ] Risky actions are guarded with appropriate warnings (severity matches risk) and allow backing out.
- [ ] The interface feels stable (no unnecessary movement/reflow of controls/windows/menus).
- [ ] Interruptible operations provide Cancel/Stop; Escape and Command-Period work where appropriate.
- [ ] Undo is supported for user mistakes and encourages safe exploration.
- [ ] Standard objects are not given new, conflicting behaviors.
- [ ] Changes are clearly visible; user actions are interpreted consistently.
- [ ] Progressive disclosure is used to keep common tasks simple without hiding power.
- [ ] Background tasks notify gently rather than stealing control/focus.
- [ ] The app behaves well in a multi-app workflow (can switch apps; doesn’t block the system unnecessarily).
- [ ] Modes are minimized; when unavoidable they are clearly indicated near the affected area and tested for mode errors.
- [ ] Users can typically save/quit at any time (except when intentionally blocked by a modal dialog).
- [ ] A wide range of actions is available without forcing users into long modal states.

## Accessibility and compatibility
- [ ] Color is never the only carrier of meaning; B&W and color-deficient users can succeed.
- [ ] Sound is not the only carrier of meaning; hearing-impaired users can succeed.
- [ ] The design works across bit depths and display capabilities.
- [ ] The design uses a consistent light source (upper-left) and avoids visual clutter.
- [ ] Black-and-white designs are created first and remain two-dimensional; color is an enhancement.
- [ ] Colors are chosen carefully: subtle for large regions; bright colors are used sparingly.
- [ ] True gray is used on color displays instead of 50% dither patterns where applicable.
- [ ] Custom elements respect system color schemes and update when users change system colors.

---

## Menus
- [ ] The menu bar is always available and unobscured (except in explicit presentation modes with an obvious restore path).
- [ ] Standard menus are present and titled consistently.
- [ ] Menu titles are stable; space is left for localization and third-party menus.
- [ ] Menus are short, grouped logically, and dividers are used sparingly.
- [ ] Checkmarks/dashes communicate state correctly (whole selection vs partial/mixed).
- [ ] Ellipsis is used only when more user input is required before execution.
- [ ] Toggled items are unambiguous (prefer explicit on/off or show/hide items).
- [ ] Keyboard equivalents follow reserved/common conventions and don’t hijack system-reserved shortcuts.

## Pop-up menus
- [ ] Pop-up menus show the current value and behave consistently.
- [ ] Type-in pop-ups are used only when they meaningfully improve large-but-constrained selection tasks.

## Palettes and tear-off menus
- [ ] Palettes behave like persistent tools (floating appropriately) and don’t mimic dialogs.
- [ ] Tear-off behavior is predictable and doesn’t create confusing duplicates.

---

## Windows
- [ ] The standard window size/position fits the user’s screen(s) and the task (not “always maximize”).
- [ ] New windows open in sensible default positions (cascade) and follow the user’s focus on multi-monitor systems.
- [ ] Windows don’t open spanning multiple screens by default.
- [ ] User-chosen window size/position is remembered and restored sensibly (and adjusted if it would reopen off-screen).
- [ ] Window titles are meaningful and follow common conventions for untitled/new documents.
- [ ] Zoom behavior is consistent (standard vs user state makes sense).
- [ ] Inactive-window selection handling follows conventions (don’t confuse users about where edits will occur).

## Dialog boxes
- [ ] Dialog wording is plain, positive, and user-centered.
- [ ] Dialogs appear where the user is working (contextual display, not just “where the menu bar is”).
- [ ] Buttons are placed consistently (action in lower-right; Cancel to the left), with a sensible default.
- [ ] Tab/Shift-Tab navigation works; focus indicators are clear.
- [ ] Dialog nesting is avoided; if unavoidable, it is handled carefully and doesn’t trap users.

## Alerts
- [ ] Alert severity (note/caution/stop) matches the situation.
- [ ] Alerts explain the problem and what the user can do next; they don’t expose internal error codes.

---

## Controls
- [ ] Buttons have clear verb labels and correct default-button behavior.
- [ ] Radio button groups are mutually exclusive and clearly labeled.
- [ ] Checkboxes represent independent toggles.
- [ ] Sliders/steppers/disclosure controls are used only when appropriate for the data.

## Icons
- [ ] Icons depict familiar objects/metaphors and are not culturally fragile.
- [ ] Icon families are complete across required sizes/bit depths; small icons are designed, not auto-scaled.
- [ ] Icon colors and selection/labeling behavior remain legible in selected/labeled states.

## Text and editing
- [ ] Text entry and editing follow standard conventions (selection, deletion, navigation).
- [ ] Localization is supported (text expansion, right-to-left alignment, multi-byte characters).

## Help and documentation
- [ ] Documentation exists, matches the target audience, and supports a learning path.
- [ ] On-screen help is available in context (including Balloon Help where appropriate).
- [ ] Help text is concise, task-focused, and avoids explaining obvious desktop UI basics.
