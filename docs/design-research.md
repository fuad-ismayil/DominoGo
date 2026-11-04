# Design tooling research and applied refinement

Evaluated 2026-11-02 using primary repositories and the connected plugin directory.

| Candidate | Value | Fit for DominoGo |
|---|---|---|
| [Impeccable](https://github.com/pbakaus/impeccable) | Explicit critique, polish, responsive/accessibility and craft guidance, with a standalone detector | Selected: its refinement mode preserves an existing visual identity and working interaction paths. The installed distribution supports Codex without introducing a Node application stack. |
| [Anthropic frontend-design](https://github.com/anthropics/skills/tree/main/skills/frontend-design) | Subject-specific direction, typography and avoidance of generic page scaffolds | Useful foundation, but less of the targeted inspection/refinement workflow needed here. |
| [Taste](https://github.com/Leonxlnx/taste-skill) | Art direction, composition and rendered verification | Strong direction-oriented alternative; its prescribed tooling uses Node scripts and is less suitable for this Rust-only refinement. |
| [UI UX Pro Max](https://github.com/nextlevelbuilder/ui-ux-pro-max-skill) | Broad multi-platform UI/UX reference material | Useful reference library; breadth alone does not resolve this game's specific hierarchy and navigation issues. |

Plugin discovery found Figma, Product Design, MagicPath, Canva and Webflow. They were not connected or represented as installed. Figma and Product Design can support collaborative design work, but no external design workspace was required for this local implementation. MagicPath's React/TypeScript export route does not fit the user's source-stack requirement. No connection was needed to finish.

Installed Impeccable v4.8.0 globally at C:/Users/Fuad/.codex/skills/impeccable, using the skill-installer helper and upstream `.agents/skills/impeccable` distribution. Loaded SKILL.md, context output, polish playbook and craft floor. The context scanner did not recognize the Rust UI as visual implementation; source, existing screenshot fixtures and the running app established the incumbent design instead. This was a scoped refinement, not a new-surface redesign requiring a new product interview.

Applied changes: removed repeated slogan eyebrows from menus/dialogs; removed font-glyph decorative dominoes on the welcome screen; opened the greeting/action area; reduced nested glass/card borders in history and secondary shortcuts; established readable menu spacing/type/focus tokens; standardized panel corners while preserving glass capsules; made navigation persistent with reserved bottom space; retained all existing labels, actions, locales and gameplay behavior. The identity and control-flow constraints are recorded in DESIGN.md for future work.

A single manual detector run over the changed Rust/menu CSS targets returned an empty finding list (`docs/design-detector.json`). It is not a proof that the UI is flawless, and the tool's Rust understanding is limited. Visual and interaction verification remain required. No downloaded plugin source or JavaScript runtime was added to the game repository.

Verification: all 48 native tests and strict WebAssembly Clippy passed. Browser checks used the isolated port-3012 database: create a table with restricted drawing, preserve the rule, add a bot and start gameplay. At 320×568, 768×1024 and 1440×900 there was no horizontal overflow. Navigation stayed visible; Azerbaijani and Russian labels were not clipped. A phone navigation expansion found during review was corrected to a consistent 68px bar. No browser console warnings/errors were observed. Captures are in docs/screenshots/design-refinement. Physical-device validation remains outstanding; these checks do not establish universal flawlessness.
