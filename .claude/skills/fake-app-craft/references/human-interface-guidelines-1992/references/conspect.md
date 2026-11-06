# Human Interface Guidelines (1992) — Detailed Conspect

## Notes on scope and quoting
- This is a paraphrased study outline (“conspect”) of **Human Interface Guidelines (1992)** (source: `Human_Interface_Guidelines_1992.pdf`).
- It is **not** a reproduction of the book’s text. Use the PDF for exact wording, diagrams, and pixel-perfect measurements.

## Book map (high level)
- **Part 1: Fundamentals**
  - Chapter 1: Human Interface Principles
  - Chapter 2: General Design Considerations (worldwide compatibility, universal access, collaborative computing)
  - Chapter 3: Human Interface Design and the Development Process
- **Part 2: The Interface Elements**
  - Chapter 4: Menus
  - Chapter 5: Windows
  - Chapter 6: Dialog Boxes (including alerts)
  - Chapter 7: Controls
  - Chapter 8: Icons
  - Chapter 9: Color
  - Chapter 10: Behaviors (mouse, keyboard, selection, editing)
  - Chapter 11: Language (terminology, messages, documentation, help, Balloon Help)
- **Appendixes**
  - A: Resources
  - B: Bibliography
  - C: Evaluation checklist
  - Glossary + Index

## Visual examples (selected)
The guidelines lean heavily on “good vs bad” screenshots and diagrams. A few recurring visual lessons:
- **Localization affects layout**: menu title length varies by language (Figure 2-1), and right-to-left scripts can expose alignment bugs if dialog item rectangles aren’t sized consistently (Figures 2-2 through 2-4).
- **Progressive disclosure in dialogs**: “More Choices / Fewer Choices” patterns expand in a predictable direction and preserve stability (Figures 3-1 and 3-2).
- **Menu semantics are visible**: ellipsis usage communicates “more input required” and should not be overloaded (Figures 4-21 through 4-24). Scrolling menus use standard top/bottom indicators (Figure 4-34).
- **Window placement and multi-monitor rules**: cascaded positions on one screen vs “follow the user” on multiple screens, and why opening across two screens is a bad default (Figures 5-15 through 5-18).
- **Dialog clarity**: spacing and focus indicators matter (Figures 6-21 and 6-22), and message wording can change “mysterious failure” into an actionable explanation (Figure 6-23).
- **Icon legibility depends on selection/labeling states**: too much black in B&W icons reduces selected-state contrast (Figures 8-19 and 8-20); the standard icon color set supports consistent selection/labeling behavior (Figures 8-23, 8-25, 8-26).
- **Black-and-white-first color design**: colorized versions should preserve the underlying design; avoid “fake 3D” via patterns in B&W (Figures 9-4 and 9-5).
- **Long operations**: status indicators can be presented in movable modal dialogs so users can switch tasks while work continues (Figure 10-3).

---

# Part 1 — Fundamentals

## Chapter 1 — Human Interface Principles

### What this chapter is doing
- Connect “why the desktop interface works” to “how to make design decisions.”
- Provide a set of principles that sometimes compete; the job is choosing what matters most for the task.

### Metaphors
- Use **concrete, familiar metaphors** so users can bring real-world expectations.
- The primary metaphor is the **desktop** (documents, folders, trash, tools).
- A metaphor should guide expectations, but the computer can also **extend** reality (e.g., folders aren’t physically limited) — keep the extension coherent.
- Avoid metaphor mismatches where appearance suggests one thing but behavior does another.

### Direct manipulation
- Make objects feel **present and controllable**:
  - Keep the target visible while acting on it.
  - Show immediate, continuous feedback as actions occur (move, resize, drag, select).
- Use feedback channels appropriately:
  - Pointer changes and brief animation for quick operations.
  - Progress/status indicators for longer operations.
- Make consequences legible before irreversible damage; warn early and clearly.

### See-and-point
- Design for recognition over recall: users should be able to **see what they can do**.
- Support the two canonical “noun–then–verb” paradigms:
  - Select an object, then choose a command (menus reveal available actions).
  - Drag an object onto another object whose role implies an action (Trash, folders, disks).
- For drag-and-drop metaphors to work, objects must **look like what they are** and behave consistently.

### Consistency
- Consistency lets skills transfer across apps and reduces relearning.
- Maintain consistency at multiple levels:
  - Visual language (what controls look like)
  - Behavioral language (what clicks/dragging mean)
  - Terminology (what things are called)
  - Interaction patterns (where to find commands, how dialogs behave)
- Prefer standard interface elements unless there is a compelling, tested reason not to.

### WYSIWYG (What You See Is What You Get)
- Screen representation should match the output users care about (often print).
- Reduce surprises by letting users edit “in place” with accurate previews.

### User control
- Users should feel in charge of:
  - The order of actions
  - Interrupting/undoing
  - Recovering from mistakes
  - Customizing the workspace where appropriate
- Don’t seize control for background tasks; notify rather than interrupt.

### Feedback and dialog
- Every command needs an observable response.
- If something cannot be done, explain why **in user terms** and what to do instead.
- Prefer messages that help users recover rather than messages that merely report failure.

### Forgiveness
- Make exploration safe:
  - Support Undo where feasible.
  - Provide Cancel/Stop for interruptible operations.
  - Use confirmations for risky, irreversible actions.
- Prefer preventing errors and offering safe defaults to “punishing” users.

### Perceived stability
- Keep the interface from “jumping around”:
  - Avoid unnecessary movement of windows/controls.
  - Keep menu titles stable.
  - Preserve user-chosen positions/sizes.
- Stability supports confidence and reduces cognitive load.

### Aesthetic integrity
- Visual design must reflect the function.
- Avoid decoration that adds noise or implies false affordances.
- Maintain a clean, readable screen with effective use of whitespace.

### Modelessness
- Prefer **modeless** interaction (do things in any order) over modes.
- If a mode is unavoidable:
  - Make the mode obvious and local to where it matters.
  - Reduce the chance of “mode errors.”
- Short, “spring-loaded” modes (only active while a control is held) are less error-prone than persistent modes.

### Additional issues
- **Know your audience**: novices and experts have different needs; plan for a sensible majority.
- **Accessibility**: design so more people can use the product without requiring special-case hacks in the core interaction.

---

## Chapter 2 — General Design Considerations

### Overview
This chapter groups early-stage design concerns into:
- **Worldwide compatibility** (localization, scripts, directionality)
- **Universal access** (disability access)
- **Collaborative computing** (networked, multi-user situations)

### Worldwide compatibility
- Plan for localization **from the start**; retrofitting is expensive.
- Treat region-dependent content as data/resources (not hard-coded UI strings).
- Cultural values:
  - Avoid region-specific assumptions about symbols, animals, colors, holidays, seasons.
  - Date/time formats and calendars vary; use proper locale utilities.
- Language differences:
  - Avoid idioms, slang, and clever wordplay that won’t translate.
  - Avoid building sentences by concatenating fragments; grammar and word order vary.
  - Expect translations to expand significantly; design UI so text can grow.
- Text display/editing assumptions to avoid:
  - Characters may be multi-byte.
  - Text is not always left-to-right.
  - Text may be spoken (text-to-speech) instead of read.
  - Fonts are not fixed to Chicago/Geneva.
- Layout alignment for right-to-left locales:
  - Design dialog item rectangles to align correctly when mirrored.
  - Keep controls within dialog boundaries after reversal.
- Keyboards and script systems:
  - Users may have multiple scripts and keyboard layouts.
  - Support the system’s Keyboard menu behavior and provide appropriate keyboard icons when shipping layouts/input methods.
- Fonts:
  - Don’t assume point sizes; let the system/script choose.
  - Leave vertical space for glyphs/diacritics that exceed typical ascenders/descenders.

### Universal access
- Goal: make software usable by people with disabilities **without** degrading usability for everyone else.
- Features for accessibility should be additional access paths, not mandatory modes.
- Physical disabilities:
  - Avoid physical barriers in hardware/media handling (where applicable).
  - In software, avoid requiring precision or timing that excludes alternative input devices.
- Visual disabilities:
  - Don’t rely on color alone; provide redundant cues (text, position, pattern, highlighting).
  - Ensure important feedback is perceivable visually and/or through assistive tech.
- Hearing disabilities:
  - Provide visual equivalents for sound-based notifications; don’t make sound the only channel.
- Speech/language disabilities:
  - Don’t block assistive communication software by bypassing standard input abstractions.
- Seizure disorders:
  - Avoid flicker in sensitive frequency ranges (notably in the ~10–50 Hz band, especially ~15–30 Hz).

### Collaborative computing
- Shared resources require clear communication:
  - Warn users of server shutdown/disconnect in time to avoid data loss.
  - Make it clear who is connected and what privileges they have.
- Access privileges:
  - Provide simple, understandable privilege setting.
  - Make the current sharing/security state visible so users don’t share accidentally.
- Password UX:
  - Allow practical length and character variety.
  - Never reveal passwords in plaintext while typing.
  - Reduce mistakes by verifying new passwords (enter twice).
- Security and interoperability:
  - Use encryption when appropriate.
  - Communicate status clearly; avoid ambiguous states.
  - Keep the network “transparent” when possible (users shouldn’t have to think in network internals for everyday tasks).

---

## Chapter 3 — Human Interface Design and the Development Process

### Design decisions and feature choice
- Every feature has non-financial costs:
  - Larger, slower, more complex UI
  - More documentation burden
  - More user error paths
  - More interactions among features
- Don’t let market/reviewer pressure override user-centered decisions.
- Watch for **feature cascade** (scope creep that reduces clarity and usability).

### The 80% solution
- Design to meet the needs of the broad majority.
- Avoid optimizing the product for a small set of expert users at the expense of most users.
- Validate with diverse user input.

### Managing complexity
- Simplicity is a primary usability strategy.
- Use **progressive disclosure**:
  - Present common options first; reveal advanced options on demand.
  - A standard pattern is a “More Choices / Fewer Choices” toggle.
  - Expand dialogs in a way that preserves stability (typically grow downward; avoid symmetric growth).
  - If expanding causes the dialog to exceed the screen, reposition minimally.

### Preferences
- Preferences are for settings users change infrequently.
- Don’t move frequently used controls into a deep preferences dialog; use modeless, accessible UI (menus, palettes) instead.
- Avoid a “junk drawer” preferences window; keep preference sets purposeful.

### Extending the interface
- Only extend when standard elements do not cover the need.
- Don’t import patterns from other platforms that conflict with established conventions.
- Build on existing visual/behavioral language:
  - Visual cues communicate behavior; don’t mix cues that imply different controls.
  - Don’t give standard symbols new meanings (example: don’t repurpose ellipsis semantics).
- If you need new behavior, design a new element and test it.

### Involving users throughout development
- Define the audience.
- Analyze tasks (what users are trying to do, not what the app can do).
- Prototype early and iteratively.
- Observe real users using the product; treat observations as primary data.

---

# Part 2 — The Interface Elements

## Chapter 4 — Menus

### Menu bar fundamentals
- The menu bar is the persistent command anchor at the top of the screen.
- Nothing should overlap or obscure it.
- Keep the standard menus present (system menu icon, File, Edit, Help, Application; Keyboard appears when needed).
- Menu titles should remain constant; leave room for localization and third-party menus.
- If a whole menu is currently unavailable, dim the title (but allow opening to browse).
- If you provide a “presentation” mode that hides the menu bar, provide an always-available way to restore it (keyboard method and a visible on-screen affordance).

### Menu interaction and feedback
- Pull-down menus support browsing without commitment (while the mouse is held).
- Execute the command on mouse release over the highlighted item.
- Keep the menu title highlighted until the operation finishes.
- Provide appropriate feedback:
  - Cursor change for short operations.
  - Status/progress UI for longer operations.
  - Prefer asynchronous/foreground-safe work for long operations.
- If a command opens an editable window (e.g., a dialog), don’t keep the originating menu title highlighted once the menu system returns.

### Naming, grouping, and structure
- Use user-language verbs and standard naming patterns.
- Group related items; separate groups with dividers sparingly.
- Keep menus scannable and stable (don’t reorder dynamically unless necessary).

### Standard menu symbols and text styles
- Checkmarks and dashes: indicate state consistently.
  - **Checkmark**: the setting applies to the entire current selection (and/or the item is “in effect”).
  - **Dash**: a mixed/partial state (the attribute applies to only part of the selection).
  - Mutually exclusive groups (like radio buttons): exactly one checkmarked item.
  - Accumulating attribute groups (like checkboxes): multiple checkmarks possible; dashes for partial application.
- Ellipsis (…): use only when the command requires **additional user input** before it can run.
  - Don’t use ellipsis merely because a window appears.
  - Don’t use ellipsis for commands that only show a warning/confirmation.
- Avoid nonstandard marks that dilute meaning.
- In the Application menu, a diamond mark can be used as a minimal “needs attention” indicator for background tasks.

### Toggled menu items (show/hide, on/off)
- Prefer showing **both opposite states** as two items (e.g., “Turn Grid On” / “Turn Grid Off”), separated as a small related group; mark the active state with a check.
- If you must use a single item whose name changes, make the verbs unambiguous (avoid names that could mean either “state” or “action”).
- Avoid using a single checkmarked item as the only indicator of an on/off feature; state should be visible in the content/workspace, not only in a menu.

### Menu types
- Scrolling menus:
  - Keep normal menus short and scannable (roughly “a dozen-ish” items is a common upper bound).
  - Use scrolling menus only for cases like user-extended lists (e.g., very long Font menus).
  - Provide standard scroll indicators at the top/bottom and allow scrolling by dragging past the visible boundary.
- Hierarchical menus: use for structured categories; keep depth reasonable.
- Pop-up menus:
  - Standard pop-ups present a set of options and show the current selection.
  - Type-in pop-ups combine typing + selection (use when lists are large but constrained).
- Tear-off menus and palettes:
  - Use when users need persistent access to a set of commands/options.
  - Palettes should behave as floating tools, not as disguised dialogs.

### Standard menus (intent)
- System menu: system-level access (“About” and desk accessories).
- File menu: document lifecycle (new/open/close/save/print/quit) and page setup.
- Edit menu: clipboard, undo/redo, selection, and related text actions.
- Font/Size/Style menus: typography choices when relevant.
- Help menu: user assistance entry points.
- Keyboard menu: script/input method selection when multiple scripts/keyboards exist.
- Application menu: switching between apps and showing attention requests.

### Keyboard equivalents (key takeaways)
- Don’t reuse system-reserved shortcuts for unrelated actions.
- Common, reserved shortcuts include:
  - File: Command-N (New), Command-O (Open), Command-W (Close), Command-S (Save), Command-P (Print), Command-Q (Quit)
  - Edit: Command-Z (Undo), Command-X/C/V (Cut/Copy/Paste), Command-A (Select All)
  - Terminate/cancel operation: Command-Period
- Worldwide/script-related reserved shortcuts include Command-Space (cycle script systems) and related variants for keyboard/layout switching.
- Common (not reserved, but widely used) shortcuts include Command-F (Find), Command-G (Find Again), Command-B/I/U (Bold/Italic/Underline), Command-T (Plain Text).

---

## Chapter 5 — Windows

### Window roles
- Document windows: primary data views.
- Utility windows: floating tools/controls that operate on the active document.
- Prefer one window per document; offer multiple views via splitting or auxiliary windows to avoid confusing “which window represents the document.”

### Appearance and standard parts
- Use standard window frames and controls (title bar, close/zoom/size boxes, scroll bars) to preserve perceived stability.
- On color systems, frames/controls use gray and accent colors while content remains primarily black/white unless user data requires color.
- Respect user customization of window colors; stick to standard color tables for compatibility.

### Utility windows (tool windows)
- Use to keep critical tools accessible.
- Hide utility windows when the app is in the background.
- Many utility windows have no title; if you include a text title, ensure it can localize (sufficient title bar height).
- Avoid adding unnecessary chrome that consumes space on small displays.
- Be explicit about keyboard focus when utility windows accept text (avoid ambiguity about where typing goes).
- Common conventions:
  - A standard utility-window drag region is smaller than a document title bar.
  - If a utility window has a text title, the title bar must be tall enough for localization (don’t design for “tiny fonts only”).
  - Utility windows typically avoid “racing stripe” title bars.
  - Typical specs mentioned in the book: an ~11 px drag region for untitled utility windows; if you include a text title, use a full-height title bar (about the same height as document windows, ~19 px) to allow localization.
  - If you provide a title bar with text, use a simple pattern fill (not racing stripes) and keep decoration minimal.

### Window behaviors and layering
- Active window is visually foregrounded; inactive windows recede.
- Floating windows sit above document windows.
- Modal dialogs appear above other windows and freeze them; movable modal dialogs can be repositioned to reveal obscured content.
- Under modal dialogs, windows may be immovable/inactive by default; a standard convention allows users to reposition frozen windows with a modifier+drag to reveal content (without making them active).

### Default positioning
- Single display: new windows open in upper-left; subsequent windows cascade down/right.
- Preserve user-chosen window size/position and restore when reopening.
- Multi-display: open first window on the display with the menu bar; subsequent windows should follow where the user is working.
- Avoid opening windows spanning multiple displays initially.
- If a saved window position/size doesn’t make sense on the current monitor configuration, adjust to a reasonable size/location (don’t reopen off-screen or oversized).
- If the user only repositions a document window (without changing document content), preserve the new window position but don’t treat it as “document content changed.”

### Dialog/alert positioning
- Center dialogs/alerts appropriately on the active display; if related to a specific document, position them near the relevant window to reinforce context.
- A common rule in the book: center horizontally; place so that roughly one-fifth of the usable vertical area is above the dialog/alert (menu bar excluded).

### Scrolling, zooming, and splitting
- Use standard scroll bars and behaviors (arrows, page regions, scroll box).
- Use automatic scrolling carefully (avoid surprising jumps).
- Zoom box should provide a sensible “best fit” size vs the user’s previous size.
- Splitting windows can provide multiple views; keep split behavior consistent and predictable.

---

## Chapter 6 — Dialog Boxes

### What dialogs are for
- Dialog boxes collect user responses and present choices in a standard frame.
- Alerts are a dialog subtype used to communicate errors, warnings, and critical information.

### Dialog types and when to use them
- Modeless dialogs:
  - Keep user control; allow working in any order.
  - Good for repeated actions (find/replace, inspectors).
- Movable modal dialogs:
  - Require user response for the current action but allow moving the dialog and switching apps.
  - Useful when the dialog would otherwise obscure needed document content.
- Modal dialogs:
  - Block interaction until resolved; use only when necessary.
- Alert boxes:
  - Use for error conditions and hazardous actions; choose the appropriate severity.

### Modeless dialog specifics
- Behave like windows (close via close box; support standard close command).
- Don’t use an “OK/Cancel” button pair to dismiss a modeless dialog.
- Pre-fill logical defaults; put initial focus in a sensible field.
- Manage menu enabling appropriately (enable editing commands when text fields are active; disable only what is invalid).

### Alert severity
- Note, caution, stop: choose based on impact and required response.
- Stop alerts are for actions that cannot be completed in the current context.

### Layout rules (core)
- Buttons: action button in lower-right; Cancel to its left.
- Default button is the most likely action, not necessarily the lower-right button.
- Use consistent margins/whitespace.
- Design for the reading flow (upper-left to lower-right in Western locales) while supporting right-to-left localization.

### Keyboard navigation
- Tab/Shift-Tab cycles focus among input controls.
- Scrolling lists can support arrow-key navigation and type selection.
- Provide a clear focus indicator (insertion point for text; border for active lists).

### Dialog and alert messages
- Use plain language.
- Explain what happened, why it matters, and what the user can do.
- Include relevant object names (document/app) to prevent confusion in multi-window workflows.

---

## Chapter 7 — Controls

### Standard controls
- Buttons: trigger actions; include a clear verb phrase label.
- Radio buttons: mutually exclusive choices within a group.
- Checkboxes: independent on/off options.

### Control selection and clarity
- Match the control to the decision type (single choice vs multiple).
- Keep grouping and alignment clean; label groups when needed.
- Make defaults sensible and safe.

### Other interaction elements
- Sliders: use for ranges where continuous adjustment is meaningful.
- Little arrows (steppers): small incremental changes.
- Outline triangles: disclose/hide additional detail without changing the model.
- Text entry fields and scrolling lists: support standard editing and navigation behaviors.

---

## Chapter 8 — Icons

### Why icons work (and why they fail)
- Icons support fast scanning and recognition.
- Failures come from ambiguity, cultural mismatch, and over-detailed artwork at small sizes.

### Designing effective icons
- Use appropriate metaphors; test that users interpret them correctly.
- Consider worldwide compatibility; avoid culturally fragile symbols.
- Avoid text inside icons.
- Design for the target display and the icon sizes actually used.
- Use a consistent light source (upper-left) and consistent perspective.
- Treat an icon family (sizes + bit depths) as one design; maintain coherence.

### Process and production
- Start with black-and-white, then colorize.
- Provide a proper mask and tuned 16×16 versions; don’t rely on automatic downscaling.
- Test icons:
  - On different desktop backgrounds
  - In selected and labeled states
  - Across bit depths (degradation)
  - With real users

### Black-and-white icon rules
- Use a clear one-pixel outline.
- Don’t overfill with black/50% gray or selection inversion won’t read.

### Color icon rules
- Keep color icons consistent with the black-and-white version (don’t redesign from scratch).
- Keep outlines crisp; avoid fuzzy gray borders.
- Use a restrained palette to avoid a “circus” desktop.
- Selection and labeling behaviors depend on standard system icon colors; large areas in nonstandard colors may not show selected/labeled states well.

### Anti-aliasing
- Use anti-aliasing to smooth internal edges without changing the shared outline/mask.
- Don’t overdo it; fuzziness is perceived as a display problem.

### Icon categories
- Different icon types (application, document, extensions/control panels, keyboard layouts) have distinct recognition roles; design them as a coherent system.

---

## Chapter 9 — Color

### Philosophy
- Color should add meaning, not decoration.
- Users should retain control to adjust or remove default color schemes.

### Standard interface color usage
- Window/dialog frames and controls use gray accents on color systems while preserving black/white readability.
- Menus: keep color use minimal except for explicit color-picking UI.
- Pointers: keep selection pointer black for visibility; allow color pointers only when they encode the active color for drawing/text.
- Selection: in black-and-white, selection uses reverse-video; in color, selection typically darkens rather than inverts.

### Application color guidelines
- Study user practice; match tools to the user’s conceptual model, not the computer’s numeric model.
- Provide novice-friendly palettes plus expert-level customization.
- Design in black and white first; then colorize.
- Use color redundantly (never as the only signal).
- Keep black-and-white designs two-dimensional; don’t fake color shadows with patterns.
- Limit colors and use bright colors sparingly; use subtle colors for large areas.
- Use true gray (not 50% dither) on color systems.

---

## Chapter 10 — Behaviors

### Pointing device fundamentals
- The pointer has a hot spot; objects have hot zones.
- Pointer shape changes should communicate meaning (context/mode), not be arbitrary.
- Standard pointers include arrow, I‑beam, crosshairs, plus sign (arrays), and wristwatch (busy).

### Long operations
- If users must wait, show a busy indicator; if it lasts more than a moment, provide progress/status.
- Prefer designs where users can switch to other tasks/apps while work continues.

### Mouse actions
- Clicking: selection and activation.
- Double-clicking: opening/primary activation; avoid making double-click the only way to perform a critical action.
- Pressing: indicates intent; releasing completes.
- Dragging: selection ranges and moving objects; show outlines when helpful; constrain movement sensibly and support auto-scroll.

### Keyboard usage
- Keyboard is primarily for text entry; navigation keys are shortcuts, not the only path.
- Use standard meanings for:
  - Enter, Return, Tab, Delete/Backspace, Escape
  - Modifier keys (Shift, Option, Command, Control)
  - Arrow keys with/without modifiers
  - Function keys (Help, Home/End/Page)
- Support type-ahead and auto-repeat appropriately.

### Selection
- Support standard selection modifications:
  - Shift-click extends
  - Command-click toggles/discontinuous selection where supported
- Selection should be clearly visible in text, graphics, and lists/tables.

### Editing text
- Insert, delete, and replace selection predictably.
- Support intelligent cut/paste behaviors where appropriate.

---

## Chapter 11 — Language

### Style and terminology
- Use simple, user-centered language.
- Prefer user terms over developer terms; avoid jargon and internal names.
- Use terminology consistently across menus, dialogs, help, and documentation.

### Labels
- Labels should describe what happens (actions, states) in the user’s language.
- Be consistent between menu item names and dialog titles.

### Dialog and alert messages
- Make messages:
  - Specific (include object names)
  - Actionable (what the user can do)
  - Non-technical (avoid system codes)
  - Non-threatening

### User documentation
- Treat documentation as part of the product.
- Tailor to different user levels and learning styles.
- Provide a sensible learning path (setup → basic tasks → advanced features → troubleshooting).

### Online help systems
- Provide help in context (concurrent help).
- Offer multiple levels (quick hints through deeper explanations).
- Focus on answering user questions and coaching tasks.

### Balloon Help (help balloons)
- Use balloons to explain UI elements in context when users need immediate guidance.
- Keep balloon text brief and task-focused.
- Provide consistent wording for different element types (buttons, menus, radio/checkboxes, palettes, window parts, icons, text entry).

---

# Appendixes — What they add

## Appendix A — Resources
- Lists professional societies, conferences, and developer resources for deeper study.

## Appendix B — Bibliography
- Curated reading list across topics such as animation, cognitive psychology, color, typography, iconography, HCI, usability testing, worldwide software, and universal access.

## Appendix C — Checklist (evaluation)
- A practical review list to validate that your product:
  - Matches the desktop “look” and “feel”
  - Provides feedback, stability, undo/cancel
  - Uses appropriate metaphors and consistent behaviors
  - Handles modes carefully
  - Is accessible (color/hearing/etc.)
  - Uses clean, uncluttered graphic design and consistent light source
  - Uses black-and-white-first color design and avoids color-only meaning
