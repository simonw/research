# cssDOOM Analysis - Research Notes

## Sources
- https://cssdoom.wtf - the live site
- https://nielsleenheer.com/articles/2026/css-is-doomed-rendering-doom-in-3d-with-css/ - author's blog post
- Fetched index.html, main JS bundle (~81KB minified), CSS bundle (~42KB minified)

## Key findings
- Created by **Niels Leenheer** (nielsleenheer.com, GitHub: NielsLeenheer)
- Niels is a Microsoft MVP, Google Developer Expert, CTO of Salonhub, creator of HTML5test
- The project is open source at github.com/NielsLeenheer/cssDOOM
- Game logic is in JavaScript, but ALL rendering is done via CSS transforms, animations, and SVG filters
- Uses Vite as bundler (evident from module preload polyfill)
- Maps loaded from JSON files extracted from DOOM WAD files
- Supports Episode 1 (E1M1 through E1M9)
- Includes keyboard, mouse, touch, and gamepad input support

## CSS techniques discovered
1. 3D wall positioning with `translate3d` + `rotateY(atan2())` - geometry computed in CSS
2. `hypot()` for wall width calculation
3. `@property` for animatable custom properties (--player-z, --light, --bob, etc.)
4. CSS `perspective` for the camera, world moves inversely around player
5. `clip-path: polygon()` and `clip-path: shape(evenodd)` for non-rectangular floor sectors
6. Sprite billboarding via `rotateY(calc(var(--player-angle) * 57.2958deg))`
7. Sprite sheet animation with `steps()` timing function
8. SVG filter for Spectre "fuzz" effect (feTurbulence + feDisplacementMap + feColorMatrix)
9. `anchor-name` / `position-anchor` for weapon positioning relative to HUD
10. Paused animation trick for CSS-based visibility culling (type grinding)
11. CSS `if()` with `@supports` for future-proof culling
12. CSS frustum culling computing dot products and distances entirely in custom properties
13. Light falloff computed in CSS using dot product of view direction and element position
14. Separate `translate`/`rotate` properties for independent animation of projectile position vs. billboarding
15. HUD numbers rendered purely in CSS using `round()`, `mod()`, `min()`, and `::before`/`::after` pseudo-elements
16. Door/lift animations via CSS transitions triggered by `data-state` attribute changes
17. Weapon bob, head bob, weapon switching all CSS keyframe animations
18. Powerup effects as viewport CSS filters (sepia, hue-rotate, saturate, brightness)
19. Sky rendered as viewport background-image with parallax via `--player-angle`
20. `@starting-style` for spectator mode entry transitions

## JS game loop architecture
- `requestAnimationFrame` driven game loop
- Player movement, collision detection, AI, projectile physics all in JS
- Spatial grid (128px cells) for wall/sight-line/sector queries
- Point-in-polygon for sector identification
- Line-segment collision for wall interactions
- Full enemy AI state machine (idle -> chasing -> attacking -> pain)
- Hitscan and projectile weapon systems
- Door, lift, crusher mechanics
- Teleporter support
- Item/weapon pickup system
- Sound via Web Audio API (wav files)
