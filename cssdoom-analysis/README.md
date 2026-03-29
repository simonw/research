# cssDOOM: A Deep Analysis of DOOM Rendered Entirely in CSS

> **Website:** [cssdoom.wtf](https://cssdoom.wtf)
> **Creator:** [Niels Leenheer](https://nielsleenheer.com/) — Microsoft MVP, Google Developer Expert, CTO of Salonhub, creator of [HTML5test](https://html5test.com), and conference speaker. GitHub: [NielsLeenheer](https://github.com/NielsLeenheer)
> **Source code:** [github.com/NielsLeenheer/cssDOOM](https://github.com/NielsLeenheer/cssDOOM)
> **Blog post:** [CSS is DOOMed — Rendering DOOM in 3D with CSS](https://nielsleenheer.com/articles/2026/css-is-doomed-rendering-doom-in-3d-with-css/) (March 27, 2026)

## Overview

cssDOOM is a fully playable version of DOOM where **every wall, floor, ceiling, barrel, enemy sprite, projectile, and HUD element is a `<div>` positioned in 3D space using CSS transforms**. The game logic (AI, collision detection, physics) runs in JavaScript, but the entire visual rendering pipeline — geometry, lighting, animation, billboarding, culling — is handled by CSS and SVG filters.

The project supports all 9 levels of Episode 1 ("Knee-Deep in the Dead"), with keyboard, mouse, touch, and gamepad controls, a spectator/map mode, and a full HUD rendered without any canvas or WebGL.

---

## Architecture

The site is built with Vite and ships three key files:

| File | Size | Role |
|------|------|------|
| `index.html` | 229 lines | DOM structure: viewport, HUD, menus, SVG filters |
| `index-*.js` | ~81 KB (minified) | Game loop, physics, AI, input, map loading |
| `index-*.css` | ~42 KB (minified) | **All rendering**: 3D transforms, animations, lighting, culling, HUD |

Maps are loaded as JSON files (`maps/E1M1.json`, etc.) extracted from the original DOOM WAD files. Assets (textures, sprites, sounds, flats) are served as individual PNG/WAV files.

---

## The CSS Rendering Tricks

### 1. Walls: 3D Geometry Computed Entirely in CSS

Each wall receives its DOOM coordinates as CSS custom properties. CSS then computes the wall's width using `hypot()` and its rotation angle using `atan2()` — no JavaScript geometry needed:

```css
.wall {
    --delta-x: calc(var(--end-x) - var(--start-x));
    --delta-y: calc(var(--end-y) - var(--start-y));

    width:  calc(hypot(var(--delta-x), var(--delta-y)) * 1px);
    height: calc((var(--ceiling-z) - var(--floor-z)) * 1px);

    transform:
        translate3d(
            calc(var(--start-x) * 1px),
            calc(var(--ceiling-z) * -1px),
            calc(var(--start-y) * -1px)
        )
        rotateY(atan2(var(--delta-y), var(--delta-x)));
}
```

The coordinate mapping `translate3d(x, -z, -y)` elegantly converts DOOM's top-down coordinate system (Y = north) to CSS 3D space (Y = up, Z = toward viewer) without any manual axis conversion.

### 2. The "Camera": Moving the World, Not the Viewer

CSS has no camera object. Instead, the **entire scene** is translated and rotated inversely to the player's position. JavaScript sets just four custom properties; CSS handles the rest:

```css
#scene {
    translate: 0 0 var(--perspective);
    transform:
        rotateY(calc(var(--player-angle) * -57.2958deg))
        translate3d(
            calc(var(--player-x) * -1px),
            calc(var(--player-z) * 1px),
            calc(var(--player-y) * 1px)
        );
}
```

The initial `translate: 0 0 var(--perspective)` compensates for CSS perspective origin being at the screen plane. Using a separate `translate` property (instead of folding it into `transform`) enables smooth transitions when switching camera modes (e.g., entering spectator view).

### 3. Floors and Ceilings: Rotated Planes with Polygon Clipping

Floors are vertical `<div>`s rotated 90° to lie flat, with textures aligned to world coordinates:

```css
.floor {
    transform:
        translate3d(
            calc(var(--min-x) * 1px),
            calc((var(--floor-z) + (var(--max-y) - var(--min-y)) / 2) * -1px),
            calc((var(--min-y) + var(--max-y)) / 2 * -1px)
        )
        rotateX(90deg);

    background-repeat: repeat;
    background-size: 64px 64px;
    background-position:
        calc(var(--min-x) * -1px)
        calc(var(--max-y) * 1px);
}
```

Non-rectangular sectors use `clip-path: polygon(...)`. Sectors with holes (pillars, windows) use the newer CSS `shape()` function with the `evenodd` fill rule:

```css
clip-path: shape(evenodd from 0% 0%, move to 30% 20%, line to 70% 20%, ... close);
```

### 4. Texture Alignment Across Sectors

Textures tile seamlessly across adjacent sectors by anchoring `background-position` to world coordinates rather than element-local coordinates. This creates a unified texture grid that spans the entire map.

### 5. Doors and Lifts: CSS Transitions on State Attributes

Doors group their walls and ceiling into a container `<div>`. JavaScript toggles a `data-state` attribute; CSS handles the animation:

```css
.door > .panel {
    transform: translateY(0);
    transition: transform 1s ease-in-out;
}

.door[data-state="open"] > .panel {
    transform: translateY(var(--offset));
}
```

Lifts use the same pattern with `.platform[data-state="lowered"]`.

### 6. Animatable Custom Properties via `@property`

The `--player-z` property is registered with `@property` so it can transition smoothly when the player steps off ledges or rides lifts:

```css
@property --player-z {
    syntax: "<number>";
    inherits: true;
    initial-value: 0;
}

#scene {
    transition: --player-z 0.25s ease-out;
}
```

The same technique enables animated lighting (`--light`), head bobbing (`--bob`), and scroll offsets (`--scroll-offset`).

### 7. Sprite Billboarding

Every sprite (enemy, item, decoration) always faces the camera using a counter-rotation derived from the player's angle:

```css
.decoration img, .pickup img, .sprite {
    transform-origin: 50% 100%;
    transform:
        translateX(-50%)
        rotateY(calc(var(--player-angle) * 57.2958deg))
        scaleX(var(--mirror, 1));
}
```

DOOM's 8-angle sprite system is replicated with 5 unique sprite rows and `scaleX(-1)` mirroring for angles 6–8. JavaScript computes the viewing angle and sets `--heading` (row) and `--mirror` (flip direction).

### 8. Sprite Sheet Animation with `steps()`

Walking, attacking, and dying animations use CSS sprite sheets with `steps()`:

```css
@keyframes sprite-cycle {
    from { background-position-x: 0; }
    to   { background-position-x: calc(var(--w) * var(--frames) * -1px); }
}

.sprite {
    animation: sprite-cycle calc(var(--frames) * 0.25s)
               steps(var(--frames)) infinite;
}

.sprite[data-state="dead"] {
    animation: sprite-stop calc(var(--frames) * 0.1s)
               steps(calc(var(--frames) - 1)) forwards;
}
```

Each sprite type declares its dimensions and frame count via custom properties:

```css
.sprite[data-type="imp"] {
    --w: 58; --h: 62; --cols: 5; --frames: 2; --rows: 7;
    background-image: url(/assets/sprites/sheets/TROO.png);
}
```

Random `animation-delay` values prevent enemies from marching in lockstep.

### 9. Projectile Animation: Separate `translate` and `rotate`

Fireballs use separate CSS properties for position (animated) and facing (reactive):

```css
.projectile {
    rotate: y calc(var(--player-angle) * 57.2958deg);
    animation: projectile-move var(--duration) linear both,
               projectile-glow 0.15s ease-in-out infinite alternate;
}

@keyframes projectile-move {
    from { translate: calc(var(--start-x)*1px) calc(var(--start-z)*-1px) calc(var(--start-y)*-1px); }
    to   { translate: calc(var(--end-x)*1px)   calc(var(--end-z)*-1px)   calc(var(--end-y)*-1px); }
}
```

Because `translate` and `rotate` are independent CSS properties, the animation drives position while `rotate` continuously tracks the player angle — keeping fireballs billboarded mid-flight.

### 10. The Spectre "Fuzz" Effect: SVG Filters

The original DOOM Spectre uses a pixel-displacement "fuzz" effect. cssDOOM replicates this with an inline SVG filter chain:

```html
<filter id="fuzz">
    <feTurbulence type="turbulence" baseFrequency="0.8 0.03"
                  numOctaves="3" result="noise">
        <animate attributeName="seed" values="0;1;2;3;4;5;6;7;8;9"
                 dur="0.4s" repeatCount="indefinite" calcMode="discrete" />
    </feTurbulence>
    <feDisplacementMap in="SourceGraphic" in2="noise" scale="4"
                       xChannelSelector="R" yChannelSelector="G" />
    <feColorMatrix type="matrix"
        values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0" />
    <!-- ... compositing steps for shimmering alpha -->
</filter>
```

Applied via CSS:

```css
.sprite[data-type="spectre"] {
    filter: url(#fuzz);
    opacity: 0.35;
}
```

The filter: (1) generates animated turbulence noise, (2) displaces the sprite pixels, (3) flattens to a black silhouette preserving alpha, (4) composites shimmering noise into the alpha channel.

### 11. Sector Lighting and Light Falloff

Each sector's light level becomes a CSS `filter: brightness()`:

```css
.wall, .floor, .ceiling {
    filter: brightness(var(--light, 1));
}
```

Flickering/glowing lights use `@property`-registered `--light` values animated with keyframes:

```css
@keyframes light-flicker {
    0%, 4%   { --light: 1; }
    5%, 8%   { --light: 0.5; }
    9%, 50%  { --light: 1; }
    /* ... */
}

body.sector-lights .light-flicker {
    animation: 2s step-end infinite light-flicker;
}
```

Distance-based light falloff computes a dot product of the camera forward vector and element position **entirely in CSS**:

```css
body.light-falloff #scene {
    --camera-forward-x: calc(sin(calc(var(--player-angle) * 57.2958deg)) * -1);
    --camera-forward-y: cos(calc(var(--player-angle) * 57.2958deg));
}

body.light-falloff .wall {
    --view-depth: calc(
        (var(--x) - var(--player-x)) * var(--camera-forward-x) +
        (var(--y) - var(--player-y)) * var(--camera-forward-y)
    );
    --light-falloff: clamp(0.15, calc(1 - var(--view-depth) / 1500), 1);
    filter: brightness(calc(var(--light, 1) * var(--light-falloff)));
}
```

### 12. CSS-Based Frustum Culling (The "Type Grinding" Trick)

The most ingenious trick. CSS can compute whether an element is outside the view frustum using custom properties, but has no native conditional visibility. The workaround uses a **paused animation with a computed negative delay**:

```css
body.css-frustum-culling .wall {
    --cull-x: calc(var(--x) - var(--player-x));
    --cull-y: calc(var(--y) - var(--player-y));
    --cull-depth: calc(var(--cull-x) * var(--cam-fx) + var(--cull-y) * var(--cam-fy));
    --cull-lateral: calc(var(--cull-x) * var(--cam-rx) + var(--cull-y) * var(--cam-ry));
    --cull-abs-lateral: max(var(--cull-lateral), calc(var(--cull-lateral) * -1));
    --cull-outside: clamp(0,
        calc((var(--cull-abs-lateral) - var(--cull-depth) * var(--fov-tangent)
              - var(--cull-radius)) * 1000), 1);
    --cull-any: clamp(0, calc((var(--cull-outside) + var(--cull-far)) * 1000), 1);

    animation: 1s step-end paused cull-toggle;
    animation-delay: calc(var(--cull-any) * -0.5s);
}

@keyframes cull-toggle {
    0%, 49.9% { visibility: visible; }
    50%, 100% { visibility: hidden; }
}
```

When `--cull-any` is 0 (visible), the delay is `0s` → animation at frame 0 → visible. When 1 (culled), delay is `-0.5s` → animation at 50% → hidden. This converts a continuous `<number>` into a boolean visibility toggle using only CSS.

The code also includes a progressive enhancement for future browsers supporting CSS `if()`:

```css
@supports (display: if(style(--cull-any: 0): block; else: none)) {
    body.css-frustum-culling .wall {
        display: if(style(--cull-any: 0): block; else: none);
        animation: none;
    }
}
```

### 13. HUD Numbers: Pure CSS Digit Rendering

The ammo, health, and armor counters extract individual digits using CSS math and render them via sprite-sheet offsets — no JavaScript DOM updates needed for the digit display itself:

```css
.hud-number {
    --glyph: calc(14px * var(--scale));
    --hundreds: round(down, var(--value) / 100, 1);
    --tens: mod(round(down, var(--value) / 10, 1), 10);
    --ones: mod(var(--value), 10);

    --hide-leading-zero: calc(-9999px * (1 - min(1, var(--hundreds))));
    --hide-middle-zero:  calc(-9999px * (1 - min(1, round(down, var(--value) / 10, 1))));

    --digit-1: calc(var(--hundreds) * var(--glyph) * -1 + var(--hide-leading-zero));
    --digit-2: calc(var(--tens)     * var(--glyph) * -1 + var(--hide-middle-zero));
    --digit-3: calc(var(--ones)     * var(--glyph) * -1);
}
```

The `::before` and `::after` pseudo-elements display the hundreds and ones digits, while the element itself shows the tens digit. Leading zeros are hidden by offsetting them `-9999px` off-screen.

### 14. Weapon Animation System

Weapons use sprite sheets with CSS variables for dimensions and frame count:

```css
#weapon[data-type="SHOTGUN"] {
    --w: 119; --h: 151; --frames: 6;
    background-image: url(/assets/weapons/SHOTGUN.png);
}

#weapon.firing {
    animation: weapon-fire var(--fire-duration)
               steps(calc(var(--frames) - 1)) forwards;
}
```

Weapon bobbing while walking, weapon switching (slide down/up), and the chaingun's continuous-fire loop are all pure CSS animations:

```css
body:not(.spectator) #renderer.moving #weapon:not(.firing):not(.switching) {
    animation: 1.4s linear infinite weapon-bob;
}
```

### 15. Anchor Positioning for Responsive Layout

The weapon and touch controls position themselves relative to the HUD status bar using CSS Anchor Positioning:

```css
#status { anchor-name: --status; }

#weapon {
    position-anchor: --status;
    bottom: anchor(top);
    left: anchor(center);
}
```

This ensures the weapon always floats above the status bar regardless of viewport size or whether the HUD wraps on narrow screens.

### 16. Powerup Visual Effects as Viewport Filters

Each powerup applies a CSS filter to the entire viewport:

```css
#renderer.powerup-invulnerability #viewport {
    filter: saturate(0) brightness(1.5) sepia() hue-rotate(90deg);
}
#renderer.powerup-radsuit #viewport {
    filter: sepia(0.3) hue-rotate(60deg) brightness(1.1);
}
#renderer.powerup-lightamp .wall, .floor, .ceiling {
    --light: 1 !important;
}
```

### 17. Sky Parallax

The sky is a repeating background image on the viewport, scrolled based on the player's angle:

```css
#viewport {
    background: url(/assets/textures/SKY1.png) 0 0 / auto 70% repeat-x;
    background-position-x: calc(var(--player-angle) * 163px), 0;
}
```

### 18. Animated Textures

DOOM's animated floor textures (nukage, lava) cycle through frames with CSS keyframes:

```css
@keyframes nukage {
    0%, 33%  { background-image: url(/assets/flats/NUKAGE1.png); }
    33%, 66% { background-image: url(/assets/flats/NUKAGE2.png); }
    66%, 100%{ background-image: url(/assets/flats/NUKAGE3.png); }
}

body.animated-flats .floor[data-texture^="NUKAGE"] {
    animation: 1s step-end infinite nukage;
}
```

Scrolling wall textures use an animated `@property`-registered `--scroll-offset`.

### 19. Spectator Mode: Third-Person Camera in Pure CSS

Follow mode computes a chase-camera position using CSS `sin()` and `cos()`:

```css
body.spectator.follow-mode #scene {
    --follow-dist: calc(var(--follow-height) * 0.7);

    rotate: x -55deg;
    transform:
        rotateY(calc(var(--player-angle) * -57.2958deg))
        translate3d(
            calc((var(--player-x) + sin(calc(var(--player-angle) * 57.2958deg))
                  * var(--follow-dist)) * -1px),
            calc((var(--follow-height) + var(--player-floor)) * 1px),
            calc((var(--player-y) - cos(calc(var(--player-angle) * 57.2958deg))
                  * var(--follow-dist)) * 1px)
        );
}
```

### 20. Damage/Pickup/Teleport Flash Overlays

Screen flashes are full-viewport `<div>` overlays animated with a single class toggle:

```css
#renderer.hurt #damage-overlay {
    animation: 0.3s ease-out forwards flash-overlay;
}

@keyframes flash-overlay {
    from { opacity: 0.35; }
    to   { opacity: 0; }
}
```

JavaScript uses `classList.remove()` + `offsetWidth` (forced reflow) + `classList.add()` to retrigger the animation on rapid successive hits.

---

## JavaScript Architecture Summary

While rendering is CSS, the JS (~3600 lines minified/beautified) handles:

- **Game state**: Player position, health, armor, ammo, weapons, keys, powerups
- **Map loading**: Fetches JSON maps extracted from DOOM WAD files
- **Collision detection**: Spatial hash grid (128px cells) with line-segment intersection tests
- **Sector queries**: Point-in-polygon tests for floor height, lighting, damage sectors
- **Enemy AI**: Full state machine (idle → chasing → attacking → pain → dead) with line-of-sight checks, pathfinding, reaction timers, and infighting
- **Weapons**: Hitscan, pellet spread (shotgun), melee, and rocket projectiles
- **Projectile physics**: CSS-animated flight with JS-based hit detection
- **Door/Lift/Crusher mechanics**: State management with CSS-driven animation
- **Sound**: Web Audio API loading WAV files from `/assets/sounds/`
- **Input**: Keyboard, mouse (with pointer lock), touch (virtual joystick), and gamepad (via gameControl.js library)
- **JS culling**: Distance, backface, frustum, and sky-occlusion culling running every 3rd frame
- **HUD updates**: Sets CSS custom properties (`--health`, `--ammo`, `--armor`, etc.) that CSS uses for digit rendering

---

## Notable Modern CSS Features Used

| Feature | Usage |
|---------|-------|
| `hypot()` | Wall width from endpoints |
| `atan2()` | Wall rotation angle |
| `sin()`, `cos()` | Camera offset in follow mode, light falloff |
| `round()`, `mod()` | Digit extraction for HUD numbers |
| `clamp()` | Light falloff, culling thresholds |
| `@property` | Animatable `--player-z`, `--light`, `--bob`, `--scroll-offset` |
| `anchor-name` / `position-anchor` | Weapon and touch control positioning |
| `clip-path: shape(evenodd)` | Sectors with holes |
| `@starting-style` | Spectator mode entry transitions |
| CSS `if()` (future) | Progressive enhancement for culling |
| `perspective` + `transform-style: preserve-3d` | The entire 3D scene |

---

## Credits

**cssDOOM** was created by **Niels Leenheer** ([nielsleenheer.com](https://nielsleenheer.com/)), a web platform expert, Microsoft MVP, Google Developer Expert, and CTO of [Salonhub](https://salonhub.nl). He is the creator of [HTML5test](https://html5test.com) and a regular speaker at web conferences including [CSS Day](https://cssday.nl/). He is also known for rendering DOOM on a 1980s oscilloscope and for creating a CSS-powered flamethrower.

The game logic was developed with the assistance of Claude (Anthropic's AI), based on the publicly available DOOM source code. The project uses original DOOM assets under fair use.

- GitHub: [github.com/NielsLeenheer](https://github.com/NielsLeenheer)
- Bluesky: [@html5test.com](https://bsky.app/profile/html5test.com)
- Mastodon: [@html5test@front-end.social](https://front-end.social/@html5test)
