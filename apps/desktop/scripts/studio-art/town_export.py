"""Export the town as runtime assets.

Outputs (apps/desktop/src/assets/town/):
  town-<theme>-tiles.png      ground tiles: project lot + public facilities, 2 frames each
  town-<theme>-sprites.png    buildings (8 kinds x 3 tiers x lit / dark) and state overlays
  town-manifest.json          rects and anchors for both themes

Run:  python3 scripts/studio-art/town_export.py      (from apps/desktop)
All tile / sprite offsets are relative to the projected block origin P(0, 0, 0) of a 96 x 96 block.
"""
import json, math, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "..", "src", "assets", "town"))
sys.argv = [sys.argv[0]]
sys.path.insert(0, HERE)
import town_blocks as tb  # noqa: E402
import buildings as bld  # noqa: E402
import town_yard as ty  # noqa: E402
from export import Atlas  # noqa: E402

room, ps, OL, P = tb.room, tb.ps, tb.OL, tb.P
BAYER = [[0, 2], [3, 1]]
TILE_FRAMES = 2
# Paint layers: the owner's colour is rendered as this grey; the runtime scales each pixel by
# chosen / PAINT_GREY. PAINT_ALT only exists to find which pixels depend on the colour.
PAINT_GREY, PAINT_ALT = "#808080", "#6A6A6A"


def paint_layer(render):
    """Pixels that change with the owner's colour, as rendered in PAINT_GREY (canvas with only those)."""
    a = render(PAINT_GREY)
    b = render(PAINT_ALT)
    for y, row in enumerate(a.px):
        for x, c in enumerate(row):
            if c is not None and c == b.px[y][x]:
                row[x] = None
    return a

# footprint (w, d) per kind and tier, mirroring buildings.py
SIZE = {
    "personal": [(30, 22), (38, 26), (46, 30)],
    "team": [(30, 24), (34, 26), (38, 28)],
    "experiment": [(36, 24), (44, 26), (54, 28)],
    "client": [(32, 22), (40, 24), (48, 26)],
    "backend": [(40, 26), (48, 28), (56, 30)],
    "data": [(30, 26), (36, 30), (42, 34)],
    "docs": [(36, 24), (42, 26), (50, 28)],
    "ops": [(36, 22), (36, 22), (36, 22)],
}


def place(kind, tier):
    w, d = SIZE[kind][tier - 1]
    x0 = 48 - w / 2 - (6 if kind == "backend" else 0)
    return x0, 52 - d


def light_tile(cv, lamps=True):
    lights = []
    if not room.DAY and lamps:
        for (lx, ly) in ((9.5, 9.5), (86.5, 9.5), (9.5, 86.5), (86.5, 86.5)):
            sx, sy = P(lx, ly, 20)
            lights.append((sx, sy + 14, 70, 0.6, (255, 220, 150)))
    amb = 0.95 if room.DAY else 0.6
    out = []
    for sy, row in enumerate(cv.px):
        o = []
        for sx, c in enumerate(row):
            if c is None or (sx, sy) in cv.emit:
                o.append(c)
                continue
            lv = amb
            tint = [0.0, 0.0, 0.0]
            for (lx, ly, r, inten, col) in lights:
                d = math.hypot(sx - lx, (sy - ly) * 1.6) / r
                if d < 1:
                    k = inten * (1 - d) ** 1.6
                    lv += k
                    for i in range(3):
                        tint[i] += col[i] * k * 0.35
            step = 0.07
            bias = (BAYER[sy % 2][sx % 2] / 4 - 0.375) * step
            lq = min(math.floor((lv + bias) / step) * step, 1.25)
            o.append(tuple(max(0, min(255, round(c[i] * lq + tint[i] * 0.45))) for i in range(3)))
        out.append(o)
    cv.px = out


def crop_canvas(cv):
    xs, ys = [], []
    for y, row in enumerate(cv.px):
        for x, c in enumerate(row):
            if c is not None:
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    x0, x1, y0, y1 = min(xs), max(xs), min(ys), max(ys)
    rows = [cv.px[y][x0:x1 + 1] for y in range(y0, y1 + 1)]
    return {"dx": x0 - tb.TILE_OX, "dy": y0 - tb.TILE_OY, "w": x1 - x0 + 1, "h": y1 - y0 + 1}, rows


def add_sprite(atlas, cv):
    c = crop_canvas(cv)
    if not c:
        return None
    meta, rows = c
    meta["rect"] = atlas.add(meta["w"], meta["h"], rows)
    return meta


def export_theme(theme):
    room.DAY = theme == "day"
    tiles = Atlas(1600)
    sprites = Atlas(1024)
    out = {"tiles": {}, "buildings": {}, "overlays": {}, "smoke": {}, "paint": {}, "yard": {}}

    # ---- ground tiles
    kinds = {"project": tb.project_lot, **{k: fn for k, (_, fn) in tb.FACILITIES.items()}}
    for key, fn in kinds.items():
        frames = []
        for f in range(TILE_FRAMES):
            cv = tb.new_tile()
            fn(cv, f)
            light_tile(cv)
            frames.append(add_sprite(tiles, cv))
        out["tiles"][key] = frames
        print(theme, "tile", key)

    # ---- buildings (lit = busy, dark = quiet); record chimney anchors instead of drawing smoke
    original_smoke = bld.smoke
    for kind, build in bld.BUILDERS.items():
        out["buildings"][kind] = []
        out["smoke"][kind] = []
        for tier in (1, 2, 3):
            variants = {}
            painted = {}
            anchors = []
            for variant, state in (("lit", "busy"), ("dark", "quiet")):
                def render(main=None, st=state, rec=None):
                    bld.smoke = lambda cv, x, y, z, frame, n=4, col=None: rec.append(P(x, y, z)) if rec is not None and st == "busy" else None
                    bld.MAIN = main
                    cv = tb.new_tile()
                    x0, y0 = place(kind, tier)
                    build(cv, x0, y0, 0, st, tier)
                    bld.MAIN = None
                    amb = 0.97 if room.DAY else 0.66
                    cv.px = [[tuple(min(255, round(v * (1 if (x, y) in cv.emit else amb))) for v in c) if c else None for x, c in enumerate(row)] for y, row in enumerate(cv.px)]
                    return cv
                variants[variant] = add_sprite(sprites, render(rec=anchors))
                painted[variant] = add_sprite(sprites, paint_layer(render))
            out["buildings"][kind].append(variants)
            out["paint"].setdefault(kind, []).append(painted)
            out["smoke"][kind].append([[round(a[0] - tb.TILE_OX), round(a[1] - tb.TILE_OY)] for a in anchors[: len(anchors)]])
    bld.smoke = original_smoke

    # ---- overlays
    def overlay(name, frames, draw):
        res = []
        for f in range(frames):
            cv = tb.new_tile()
            draw(cv, f)
            amb = 0.97 if room.DAY else 0.7
            cv.px = [[tuple(min(255, round(v * (1 if (x, y) in cv.emit else amb))) for v in c) if c else None for x, c in enumerate(row)] for y, row in enumerate(cv.px)]
            res.append(add_sprite(sprites, cv))
        out["overlays"][name] = res

    def smoke_puff(cv, f):
        # drawn at block origin; runtime shifts it to each chimney anchor
        for i in range(4):
            ph = (f + i) % 4
            sx_, sy_ = P(ph * 0.8, -ph * 0.4, 3 + ph * 4 + i * 1.5)
            r_ = 2 + ph
            g = OL(outline=False)
            for yy in range(-r_, r_ + 1):
                for xx in range(-r_ * 2, r_ * 2 + 1):
                    if xx * xx / 4 + yy * yy <= r_ * r_:
                        shade = 235 - ph * 12
                        g.put(sx_ + xx, sy_ + yy, (shade, shade, shade + 4))
            cv.draw(g)
    overlay("smoke", 4, smoke_puff)

    def flag(cv, f):
        mb = OL(k=-0.55)
        mx, my = tb.MAILBOX
        mb.box(mx, my, 0, 0.8, 0.8, 9, "#6E4A2E", hl=False)
        mb.box(mx - 1.5, my - 1.5, 9, 4, 4, 3.6, "#D9534F", top=tb.hexc("#F07A6E"))
        mb.box(mx + 2.5, my - 0.5, 10 + f, 0.6, 0.6, 5, "#2B2620", hl=False)
        mb.box(mx + 2.5, my - 0.5, 14 + f, 0.6, 2.6, 2, "#F2C94C", hl=False)
        cv.draw(mb)
    overlay("flag", 2, flag)

    def cloud(cv, f):
        cl = OL(k=-0.35)
        for (ox, oy, r_, dz) in ((-10, 0, 6, 0), (0, -3, 8, 2), (10, 1, 5, 0), (0, 3, 6, 5)):
            for k in range(3):
                rr = r_ * (1 - k * 0.28)
                cl.box(48 + ox - rr, 40 + oy - rr * 0.7, dz + k * 2.2, rr * 2, rr * 1.4, 2.4, "#8496AD", top=tb.hexc("#C9D3E0"), left=tb.hexc("#A9B6C8"), hl=False)
        cv.draw(cl)
        rn = OL(outline=False)
        for i in range(14):
            rx_ = 38 + (i * 7.3) % 20
            ry_ = 34 + (i * 5.1) % 12
            rz = -4 - ((f * 5 + i * 3) % 16)
            for k in range(3):
                rn.put(*P(rx_, ry_, rz - k * 1.5), tb.hexc("#5B7FD9"))
        cv.draw(rn)
    overlay("cloud", 4, cloud)

    def leaves(cv, f):
        lv = OL(outline=False)
        for i in range(40):
            lv.put(*P(18 + (tb.h32(i, 3) % 60), 54 + (tb.h32(i, 5) % 26), 0.1), tb.hexc(["#E9873A", "#C9873A", "#D9734F"][i % 3]))
        cv.draw(lv)
    overlay("leaves", 1, leaves)

    def construction(cv, f):
        x0, y0 = 26, 22
        sc = OL(k=-0.55)
        sc.box(x0, y0, 0, 44, 30, 2, "#9C8670", top=tb.hexc("#B5A58E"))
        for (px, py) in ((x0, y0), (x0 + 44, y0), (x0, y0 + 30), (x0 + 44, y0 + 30)):
            sc.box(px - 0.6, py - 0.6, 0, 1.2, 1.2, 26, "#C9CED6", hl=False)
        for z in (13, 25):
            sc.box(x0 - 0.6, y0 - 0.6, z, 45.2, 1.2, 1, "#C9CED6", hl=False)
            sc.box(x0 - 0.6, y0 - 0.6, z, 1.2, 31.2, 1, "#C9CED6", hl=False)
        sc.box(x0 + 6, y0 + 8, 2, 22, 4, 8, "#E3D3BC")
        sc.box(x0 + 10, y0 + 18, 2, 14, 4, 1.4, "#C99A6B")
        cv.draw(sc)
        cr = OL(k=-0.55)
        cr.box(x0 + 48, y0 + 2, 0, 3, 3, 52, "#F2C94C", hl=False)
        sw = (0, 3)[f]
        cr.box(x0 + 22 + sw, y0 + 2.6, 52, 30, 1.8, 2, "#F2C94C", hl=False)
        cr.box(x0 + 26 + sw, y0 + 3, 30, 0.4, 0.4, 22, "#3A3632", hl=False)
        cr.box(x0 + 24.5 + sw, y0 + 1.5, 26, 3.4, 3.4, 4, "#C99A6B")
        cv.draw(cr)
    overlay("construction", 2, construction)

    # ---- yard decorations chosen by the owner, each at its own spot in the front yard
    for item, draw in ty.YARD_ITEMS.items():
        frames = []
        for f in range(2):
            cv = tb.new_tile()
            draw(cv, f)
            amb = 0.97 if room.DAY else 0.7
            cv.px = [[tuple(min(255, round(v * (1 if (x, y) in cv.emit else amb))) for v in c) if c else None for x, c in enumerate(row)] for y, row in enumerate(cv.px)]
            frames.append(add_sprite(sprites, cv))
        out["yard"][item] = frames

    tiles.write(os.path.join(OUT, f"town-{theme}-tiles.png"))
    sprites.write(os.path.join(OUT, f"town-{theme}-sprites.png"))
    return out


def main():
    os.makedirs(OUT, exist_ok=True)
    manifest = {
        "block": tb.BLOCK,
        "tileFrames": TILE_FRAMES,
        "sign": tb.SIGN_ANCHOR,
        "mailbox": tb.MAILBOX,
        "yard": tb.YARD,
        "paintGrey": int(PAINT_GREY[1:3], 16),
        "facilities": {k: name for k, (name, _) in tb.FACILITIES.items()},
    }
    for theme in ("day", "night"):
        manifest[theme] = export_theme(theme)
    with open(os.path.join(OUT, "town-manifest.json"), "w") as fh:
        json.dump(manifest, fh, ensure_ascii=False, separators=(",", ":"))
    print("wrote", OUT)


if __name__ == "__main__":
    main()
