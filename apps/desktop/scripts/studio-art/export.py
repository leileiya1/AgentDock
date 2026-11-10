"""Export the studio room as runtime assets for the desktop app.

Outputs (apps/desktop/src/assets/studio/):
  studio-<theme>-bg.png      4 animation frames side by side: room, furniture, no robots, desks dark
  studio-<theme>-atlas.png   furniture sprites (for re-drawing over robots), lit-desk overlays, robot sprites
  studio-manifest.json       coordinates and atlas rects for both themes

Run:  python3 scripts/studio-art/export.py      (from apps/desktop)
Everything is generated from code (room.py scene and builders, pixel_base.py); no hand-edited pixels.
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "..", "src", "assets", "studio"))
sys.argv = [sys.argv[0]]
sys.path.insert(0, HERE)
import room  # noqa: E402
import robot_layers  # noqa: E402

ps = room.ps
W, H = room.W, room.H
FRAMES = 4
FAMILIES = list(room.FAM.keys())


def robot_tint(c, theme):
    if theme == "day":
        return tuple(min(255, round(v * 0.96)) for v in c)
    return tuple(min(255, round(c[i] * 0.8 + (6, 10, 30)[i])) for i in range(3))


def render_room(theme, frame, lit):
    room.DAY = theme == "day"
    room.VARIANT = "away"
    room.NO_ROBOTS = True
    room.LIT_DESKS = lit
    cv, _ = room.scene(frame)
    return cv


def crop(px, keep):
    """px: dict (x,y)->rgb. Returns (x0, y0, w, h, rows) for the bounding box of `keep` pixels."""
    pts = [p for p in px if keep(p)]
    if not pts:
        return None
    x0 = min(p[0] for p in pts)
    y0 = min(p[1] for p in pts)
    x1 = max(p[0] for p in pts)
    y1 = max(p[1] for p in pts)
    rows = [[px.get((x, y)) if keep((x, y)) else None for x in range(x0, x1 + 1)] for y in range(y0, y1 + 1)]
    return x0, y0, x1 - x0 + 1, y1 - y0 + 1, rows


class Atlas:
    """Simple shelf packer."""

    def __init__(self, width=1024):
        self.width = width
        self.x = 0
        self.y = 0
        self.row_h = 0
        self.blits = []

    def add(self, w, h, rows):
        if self.x + w + 1 > self.width:
            self.x = 0
            self.y += self.row_h + 1
            self.row_h = 0
        rect = [self.x, self.y, w, h]
        self.blits.append((self.x, self.y, rows))
        self.x += w + 1
        self.row_h = max(self.row_h, h)
        return rect

    def write(self, path):
        hgt = self.y + self.row_h + 1
        img = [[None] * self.width for _ in range(hgt)]
        for (x, y, rows) in self.blits:
            for dy, row in enumerate(rows):
                for dx, c in enumerate(row):
                    if c is not None:
                        img[y + dy][x + dx] = c
        ps.write_png(path, self.width, hgt, img)


def robot_sprite(theme, draw):
    room.DAY = theme == "day"
    cv = room.Canvas()
    ax, ay = draw(cv)
    px = {}
    for y in range(max(0, ay - 80), min(H, ay + 24)):
        for x in range(max(0, ax - 44), min(W, ax + 44)):
            c = cv.px[y][x]
            if c is not None:
                px[(x, y)] = robot_tint(c, theme)
    x0, y0, w, h, rows = crop(px, lambda p: True)
    return {"ox": ax - x0, "oy": ay - y0, "w": w, "h": h}, rows


# reference positions inside the room where robots are drawn for export
RX, RY = 60, 50


def poses():
    """(pose key, frames, draw(cv, fam, frame) -> anchor screen point)."""
    P = room.P

    def front(mode, z0=0, back=False):
        def d(cv, fam, f, acc=()):
            room.robot_front(cv, RX, RY, fam, mode, f, z0=z0, acc=acc, back=back)
            return tuple(int(v) for v in P(RX, RY, 0))
        return d

    def side(cv, fam, f, acc=()):
        room.robot_side(cv, RX, RY, fam, f, z=7.6, acc=acc)
        return tuple(int(v) for v in P(RX, RY, 0))

    def seated_back(cv, fam, f, acc=()):
        room.robot_back(cv, RX, RY, fam, f, acc=acc)
        return tuple(int(v) for v in P(RX, RY, 0))

    return [
        ("stand-idle", front("idle")),
        ("stand-work", front("work")),
        ("stand-hand", front("hand")),
        ("stand-cup", front("cup")),
        ("stand-rest", front("rest")),
        ("walk-front", front("walk")),
        ("walk-back", front("walk", back=True)),
        ("sit-front", front("work", z0=8)),
        ("sleep", front("sleep", z0=4.5)),
        ("sit-back", seated_back),
        ("sit-side", side),
    ]


def export_theme(theme, manifest):
    atlas = Atlas()
    # ---- background frames (desks dark) + ownership for furniture sprites
    frames = [render_room(theme, f, False) for f in range(FRAMES)]
    sheet = []
    for y in range(H):
        row = []
        for cv in frames:
            row.extend(cv.px[y])
        sheet.append(row)
    ps.write_png(os.path.join(OUT, f"studio-{theme}-bg.png"), W * FRAMES, H, sheet)

    base = frames[0]
    keys = base.item_keys
    owned = {}
    for p, idx in base.owner.items():
        owned.setdefault(idx, []).append(p)
    items = []
    for idx, k in enumerate(keys):
        if k < 0 or idx not in owned:
            continue  # wall-mounted / floor decals never cover a robot
        px = {p: base.px[p[1]][p[0]] for p in owned[idx] if 0 <= p[0] < W and 0 <= p[1] < H and base.px[p[1]][p[0]]}
        c = crop(px, lambda p: p in px)
        if not c:
            continue
        x0, y0, w, h, rows = c
        items.append({"key": k, "x": x0, "y": y0, "rect": atlas.add(w, h, rows)})

    # ---- lit-desk overlays (screens on), 4 frames per desk
    lit = [render_room(theme, f, True) for f in range(FRAMES)]
    desks = {}
    desk_keys = {14 + 20 + 38 + 9: "desk-1", 56 + 20 + 38 + 9: "desk-2", 56 + 20 + 72 + 9: "desk-3"}
    desk_items = [(i, desk_keys[k]) for i, k in enumerate(keys) if k in desk_keys]
    for idx, desk_id in desk_items:
        frames_out = []
        region = set(owned.get(idx, []))
        diff_all = set()
        for f in range(FRAMES):
            for p in region:
                a = frames[f].px[p[1]][p[0]]
                b = lit[f].px[p[1]][p[0]]
                if a != b:
                    diff_all.add(p)
        if not diff_all:
            continue
        xs = [p[0] for p in diff_all]
        ys = [p[1] for p in diff_all]
        x0, y0, x1, y1 = min(xs), min(ys), max(xs), max(ys)
        for f in range(FRAMES):
            rows = [[(lit[f].px[y][x] if (x, y) in diff_all else None) for x in range(x0, x1 + 1)] for y in range(y0, y1 + 1)]
            frames_out.append(atlas.add(x1 - x0 + 1, y1 - y0 + 1, rows))
        desks[desk_id] = {"x": x0, "y": y0, "frames": frames_out}

    # ---- robots
    robots = {}
    for fam in FAMILIES:
        robots[fam] = {}
        for key, draw in poses():
            out = []
            for f in (0, 1):
                meta, rows = robot_sprite(theme, lambda cv, d=draw, fm=fam, ff=f: d(cv, fm, ff))
                meta["rect"] = atlas.add(meta["w"], meta["h"], rows)
                out.append(meta)
            robots[fam][key] = out
    # the human (you) at the boss desk
    meta, rows = robot_sprite(theme, lambda cv: (room.human_front(cv, RX, RY, 15, 0), tuple(int(v) for v in room.P(RX, RY, 0)))[1])
    meta["rect"] = atlas.add(meta["w"], meta["h"], rows)
    you = meta

    custom = robot_layers.export(theme, atlas, poses, crop, robot_tint, W, H)

    atlas.write(os.path.join(OUT, f"studio-{theme}-atlas.png"))
    manifest[theme] = {"items": items, "desks": desks, "robots": robots, "you": you, "custom": custom}
    print(theme, "items", len(items), "desks", list(desks), "atlas rows", atlas.y)


def main():
    os.makedirs(OUT, exist_ok=True)
    manifest = {"W": W, "H": H, "OX": ps.OX, "OY": ps.OY, "frames": FRAMES}
    for theme in ("day", "night"):
        export_theme(theme, manifest)
    with open(os.path.join(OUT, "studio-manifest.json"), "w") as fh:
        json.dump(manifest, fh, separators=(",", ":"))
    print("wrote", OUT)


if __name__ == "__main__":
    main()
