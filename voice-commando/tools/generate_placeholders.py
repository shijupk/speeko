"""
Generate placeholder sprite-sheet PNGs for Commando Komodo.

Each animal gets a sprite sheet with 64x64 tiles in a 4-column grid.
Frames show a colored silhouette shape + label so the game works
immediately while waiting for real art.

Usage:
    pip install Pillow
    python tools/generate_placeholders.py
"""

import os
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

TILE = 64
COLS = 4
OUT_DIR = Path(__file__).resolve().parent.parent / "assets" / "sprites"

# fmt: off
ANIMALS = {
    #  name          base_color       rows  role        silhouette_fn
    "komodo":    ((25, 128, 25),      6,    "player",   "lizard"),
    "rabbit":    ((179, 153, 102),    1,    "prey",     "small"),
    "chicken":   ((230, 204, 51),     1,    "prey",     "small"),
    "goat":      ((153, 153, 153),    1,    "prey",     "medium"),
    "bull":      ((153, 38, 25),      1,    "predator", "large"),
    "tiger":     ((230, 128, 25),     1,    "predator", "large"),
    "crocodile": ((77, 115, 51),      1,    "predator", "wide"),
}
# fmt: on

ROW_LABELS = ["idle", "walk_l", "walk_r", "eat", "hurt", "death"]


def draw_silhouette(draw: ImageDraw.ImageDraw, shape: str, x: int, y: int,
                    color: tuple, frame: int):
    """Draw a simple animal-like silhouette in the 64x64 tile."""
    cx, cy = x + TILE // 2, y + TILE // 2
    # Slight frame-based wobble to simulate animation
    wobble = (frame % 2) * 3 - 1

    if shape == "lizard":
        # Horizontal body + 4 legs + tail
        bw, bh = 28, 12
        draw.ellipse([cx - bw, cy - bh + wobble, cx + bw, cy + bh + wobble], fill=color)
        # head
        draw.ellipse([cx + 18, cy - 8 + wobble, cx + 32, cy + 4 + wobble], fill=color)
        # tail
        draw.line([(cx - 28, cy + wobble), (cx - 40, cy - 6 + wobble)],
                  fill=color, width=4)
        # legs
        for lx in [-14, -4, 10, 20]:
            ly = 10 + (frame % 2) * 3 if (lx + frame) % 2 == 0 else 12
            draw.rectangle([cx + lx - 3, cy + ly + wobble - 2,
                            cx + lx + 3, cy + ly + wobble + 6], fill=color)
    elif shape == "small":
        # Small round body + ears/head
        r = 14
        draw.ellipse([cx - r, cy - r + 4 + wobble, cx + r, cy + r + 4 + wobble],
                     fill=color)
        # head
        draw.ellipse([cx - 6, cy - 16 + wobble, cx + 10, cy - 2 + wobble], fill=color)
        # legs
        for lx in [-8, 8]:
            draw.rectangle([cx + lx - 3, cy + r + wobble, cx + lx + 3,
                            cy + r + 8 + wobble], fill=color)
    elif shape == "medium":
        # Medium body
        bw, bh = 18, 14
        draw.ellipse([cx - bw, cy - bh + wobble, cx + bw, cy + bh + wobble], fill=color)
        # head
        draw.ellipse([cx + 10, cy - 18 + wobble, cx + 26, cy - 4 + wobble], fill=color)
        # legs
        for lx in [-12, -2, 8, 16]:
            draw.rectangle([cx + lx - 2, cy + bh - 2 + wobble,
                            cx + lx + 2, cy + bh + 8 + wobble], fill=color)
    elif shape == "large":
        # Large muscular body
        bw, bh = 22, 16
        draw.ellipse([cx - bw, cy - bh + wobble, cx + bw, cy + bh + wobble], fill=color)
        # head
        draw.ellipse([cx + 12, cy - 20 + wobble, cx + 30, cy - 2 + wobble], fill=color)
        # horns / ears
        draw.line([(cx + 16, cy - 20 + wobble), (cx + 12, cy - 28 + wobble)],
                  fill=color, width=3)
        draw.line([(cx + 26, cy - 20 + wobble), (cx + 30, cy - 28 + wobble)],
                  fill=color, width=3)
        # legs
        for lx in [-14, -4, 8, 18]:
            draw.rectangle([cx + lx - 3, cy + bh - 2 + wobble,
                            cx + lx + 3, cy + bh + 10 + wobble], fill=color)
    elif shape == "wide":
        # Low wide body (croc)
        bw, bh = 28, 10
        draw.ellipse([cx - bw, cy - bh + wobble, cx + bw, cy + bh + wobble], fill=color)
        # snout
        draw.ellipse([cx + 20, cy - 6 + wobble, cx + 38, cy + 4 + wobble], fill=color)
        # tail
        draw.line([(cx - 28, cy + wobble), (cx - 42, cy + 4 + wobble)],
                  fill=color, width=5)
        # legs
        for lx in [-16, -4, 10, 20]:
            draw.rectangle([cx + lx - 3, cy + bh - 2 + wobble,
                            cx + lx + 3, cy + bh + 6 + wobble], fill=color)


def lighter(color: tuple, amount: int = 40) -> tuple:
    return tuple(min(255, c + amount) for c in color)


def darker(color: tuple, amount: int = 40) -> tuple:
    return tuple(max(0, c - amount) for c in color)


def generate_sheet(name: str, base_color: tuple, rows: int, role: str,
                   shape: str):
    width = TILE * COLS
    height = TILE * rows
    img = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    try:
        font = ImageFont.truetype("arial.ttf", 10)
    except OSError:
        font = ImageFont.load_default()

    for row in range(rows):
        row_label = ROW_LABELS[row] if row < len(ROW_LABELS) else f"r{row}"
        for col in range(COLS):
            x = col * TILE
            y = row * TILE
            frame = row * COLS + col

            # Vary color per row for the komodo (player) to distinguish states
            if rows > 1:
                if row == 3:       # eat
                    c = (50, 220, 80)
                elif row == 4:     # hurt
                    c = (220, 50, 50)
                elif row == 5:     # death
                    c = (80, 80, 80)
                elif row in (1, 2):  # walk
                    c = lighter(base_color, 30)
                else:
                    c = base_color
            else:
                c = base_color

            # Slight color variation per frame for visual movement
            c = lighter(c, col * 8) if col % 2 == 0 else darker(c, col * 5)

            # Draw the silhouette
            draw_silhouette(draw, shape, x, y, c, frame)

            # Draw frame label
            label = f"{name[0].upper()}{frame}"
            draw.text((x + 2, y + 2), label, fill=(255, 255, 255, 180), font=font)
            # Draw row label in first col
            if col == 0:
                draw.text((x + 2, y + TILE - 14), row_label,
                          fill=(255, 255, 255, 120), font=font)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out_path = OUT_DIR / f"{name}.png"
    img.save(out_path)
    print(f"  {out_path}  ({width}x{height}, {COLS}x{rows} grid)")


def main():
    print("Generating placeholder sprite sheets...")
    for name, (color, rows, role, shape) in ANIMALS.items():
        generate_sheet(name, color, rows, role, shape)
    print(f"\nDone! {len(ANIMALS)} sprite sheets in {OUT_DIR}")


if __name__ == "__main__":
    main()
