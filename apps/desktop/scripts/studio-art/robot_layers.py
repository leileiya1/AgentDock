"""Extra robot sprites for owner customisation (Agent 档案):

- paint: for each antenna shape, the pixels that take the body colour ("base") and the face-plate
  colour ("light"), rendered in grey. The runtime scales them by chosen / grey and draws them over
  the family sprite, so any colour works without exporting every colour.
- acc: accessory overlays (cap, headphones, ...), found by rendering a robot with and without it.

Both are keyed by the same pose keys and frames as the family sprites and share their anchors.
"""
import room
from robot_acc import PROFILE

GREY, GREY_ALT = "#808080", "#6A6A6A"
LIGHT, LIGHT_ALT = "#C0C0C0", "#AAAAAA"


def _raw(theme, draw, tint, W, H):
    room.DAY = theme == "day"
    cv = room.Canvas()
    ax, ay = draw(cv)
    px = {}
    for y in range(max(0, ay - 80), min(H, ay + 24)):
        for x in range(max(0, ax - 44), min(W, ax + 44)):
            c = cv.px[y][x]
            if c is not None:
                px[(x, y)] = tint(c, theme)
    return (ax, ay), px


def _sprite(atlas, crop, anchor, px):
    c = crop(px, lambda p: p in px)
    if not c:
        return None
    x0, y0, w, h, rows = c
    return {"ox": anchor[0] - x0, "oy": anchor[1] - y0, "w": w, "h": h, "rect": atlas.add(w, h, rows)}


def export(theme, atlas, poses, crop, tint, W, H):
    shapes = sorted({ant for (_, _, ant) in room.FAM.values()})
    saved = dict(room.FAM)
    paint = {}
    for ant in shapes:
        room.FAM["_a"] = (GREY, LIGHT, ant)
        room.FAM["_b"] = (GREY_ALT, LIGHT, ant)
        room.FAM["_c"] = (GREY, LIGHT_ALT, ant)
        paint[ant] = {}
        for key, draw in poses():
            frames = []
            for f in (0, 1):
                anchor, a = _raw(theme, lambda cv, d=draw: d(cv, "_a", f), tint, W, H)
                _, b = _raw(theme, lambda cv, d=draw: d(cv, "_b", f), tint, W, H)
                _, c = _raw(theme, lambda cv, d=draw: d(cv, "_c", f), tint, W, H)
                base = {p: v for p, v in a.items() if b.get(p) != v}
                light = {p: v for p, v in a.items() if c.get(p) != v and p not in base}
                frames.append({"base": _sprite(atlas, crop, anchor, base), "light": _sprite(atlas, crop, anchor, light)})
            paint[ant][key] = frames
    room.FAM.clear()
    room.FAM.update(saved)

    acc = {}
    for name in PROFILE:
        acc[name] = {}
        for key, draw in poses():
            frames = []
            for f in (0, 1):
                anchor, plain = _raw(theme, lambda cv, d=draw: d(cv, "neutral", f), tint, W, H)
                _, worn = _raw(theme, lambda cv, d=draw: d(cv, "neutral", f, ("p:" + name,)), tint, W, H)
                diff = {p: v for p, v in worn.items() if plain.get(p) != v}
                frames.append(_sprite(atlas, crop, anchor, diff))
            acc[name][key] = frames
    return {
        "paint": paint,
        "acc": acc,
        "antenna": {fam: ant for fam, (_, _, ant) in room.FAM.items()},
        "grey": {"base": int(GREY[1:3], 16), "light": int(LIGHT[1:3], 16)},
    }
