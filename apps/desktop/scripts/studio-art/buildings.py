"""Building catalog for the AgentFlow town: one style per project kind, three size tiers each.

Every builder: build(cv, x0, y0, frame, state, tier) where (x0, y0) is the back-left corner of the plot core
(about 40 x 28 ground units for tier 1). `state` is one of busy | waiting | failing | quiet | archived.
"""
import math
import town

room = town.room
hexc, lit, C, OL, P = town.hexc, town.lit, town.C, town.OL, town.P


def _P(x, y, z=0):
    return town.ps.P(x, y, z)


# The owner's colour choice (roof / main accent). Export renders it with two greys to find the
# pixels it touches; the runtime then repaints those pixels in the chosen colour.
MAIN = None


def main(default):
    return MAIN or default


def windows_y(cv, yface, x0, x1, z0, z1, frame, lit_ratio, step_x=6, step_z=7, ww=3.6, wh=4.2, frame_col="#FFFFFF", glass=None, emissive=True):
    """Grid of windows on a +y facing wall."""
    DAY = room.DAY
    on_col = C("#FFD98A", "#CFE6F7")
    off_col = glass or C("#2A2C36", "#8FA8C2")
    w = OL(outline=False, emissive=emissive and not DAY)
    i = 0
    z = z0
    while z + wh <= z1:
        x = x0
        while x + ww <= x1:
            on = ((i * 7 + frame) % 10) / 10 < lit_ratio
            w.face_y(yface + 0.02, x - 0.5, z - 0.5, ww + 1, wh + 1, hexc(frame_col))
            w.face_y(yface + 0.04, x, z, ww, wh, on_col if on else off_col)
            w.face_y(yface + 0.05, x + ww * 0.65, z, 0.5, wh, lit(on_col if on else off_col, 0.25))
            x += step_x
            i += 1
        z += step_z
    cv.draw(w)


def windows_x(cv, xface, y0, y1, z0, z1, frame, lit_ratio, step_y=6, step_z=7, ww=3.6, wh=4.2, frame_col="#FFFFFF", glass=None, emissive=True):
    DAY = room.DAY
    on_col = C("#FFD98A", "#CFE6F7")
    off_col = glass or C("#2A2C36", "#8FA8C2")
    w = OL(outline=False, emissive=emissive and not DAY)
    i = 3
    z = z0
    while z + wh <= z1:
        y = y0
        while y + ww <= y1:
            on = ((i * 7 + frame) % 10) / 10 < lit_ratio
            w.face_x(xface + 0.02, y - 0.5, z - 0.5, ww + 1, wh + 1, hexc(frame_col))
            w.face_x(xface + 0.04, y, z, ww, wh, on_col if on else off_col)
            y += step_y
            i += 1
        z += step_z
    cv.draw(w)


def lit_ratio(state):
    return {"busy": 0.75, "waiting": 0.6, "failing": 0.5, "quiet": 0.0, "archived": 0.0}.get(state, 0.4)


def slab(cv, x0, y0, w, d, col="#B5A58E"):
    s = OL(k=-0.5)
    s.box(x0 - 2, y0 - 2, 0, w + 4, d + 4, 1.2, col, top=C("#5A5D6A", "#D8D0BF"), hl=False)
    cv.draw(s)


def smoke(cv, x, y, z, frame, n=4, col=(235, 235, 240)):
    for i in range(n):
        ph = (frame + i) % 4
        sx_, sy_ = _P(x + ph * 0.8, y - ph * 0.4, z + 3 + ph * 4 + i * 1.5)
        r_ = 2 + ph
        for yy in range(-r_, r_ + 1):
            for xx in range(-r_ * 2, r_ * 2 + 1):
                if xx * xx / 4 + yy * yy <= r_ * r_:
                    cv.blend(sx_ + xx, sy_ + yy, col, 0.55 - ph * 0.1)


def gable(cv, x0, y0, w, d, zt, roof, wall, o=2, rh=None):
    rf = OL(k=-0.6)
    rh = rh if rh is not None else d * 0.55
    R = hexc(roof)
    rf.poly([P(x0 - o, y0 + d + o, zt - 0.5), P(x0 + w + o, y0 + d + o, zt - 0.5), P(x0 + w + o, y0 + d / 2, zt + rh), P(x0 - o, y0 + d / 2, zt + rh)],
            lambda X, Y: lit(R, -0.08) if (Y % 3 == 0) else R)
    rf.poly([P(x0 + w, y0, zt), P(x0 + w, y0 + d, zt), P(x0 + w, y0 + d / 2, zt + rh)], lit(hexc(wall), -0.12))
    cv.draw(rf)
    return rh


def door_y(cv, yface, x, w=6, h=10, col="#B5651D"):
    d = OL(k=-0.55)
    d.face_y(yface + 0.03, x, 1.2, w, h, hexc("#6E3712"))
    d.face_y(yface + 0.05, x + 0.7, 1.2, w - 1.4, h - 0.7, hexc(col))
    d.face_y(yface + 0.07, x + w - 2.2, h * 0.5, 0.6, 0.8, hexc("#E6B54A"))
    cv.draw(d)


# ------------------------------------------------------------------ 1. cottage (personal project)
def cottage(cv, x0, y0, frame, state="busy", tier=1, roof="#C9605F", wall="#F3E3C3"):
    w, d, h = (30, 22, 16) if tier == 1 else (38, 26, 18) if tier == 2 else (46, 30, 20)
    slab(cv, x0, y0, w, d)
    b = OL(k=-0.6)
    b.box(x0, y0, 1.2, w, d, h, wall, top=lit(hexc(wall), 0.1))
    for i in range(int(h // 3)):
        b.face_y(y0 + d + 0.01, x0, 1.2 + i * 3, w, 0.3, lit(hexc(wall), -0.08))
    cv.draw(b)
    windows_y(cv, y0 + d, x0 + 4, x0 + w / 2 - 4, 6, h - 2, frame, lit_ratio(state), step_x=7, ww=4.4, wh=4.6, frame_col="#7A5536")
    windows_y(cv, y0 + d, x0 + w / 2 + 5, x0 + w - 2, 6, h - 2, frame, lit_ratio(state), step_x=7, ww=4.4, wh=4.6, frame_col="#7A5536")
    windows_x(cv, x0 + w, y0 + 4, y0 + d - 2, 6, h - 2, frame, lit_ratio(state), step_y=8, ww=4.4, wh=4.6, frame_col="#7A5536")
    door_y(cv, y0 + d, x0 + w / 2 - 3)
    zt = 1.2 + h
    rh = gable(cv, x0, y0, w, d, zt, main(roof), wall)
    ch = OL(k=-0.55)
    chx, chy = x0 + w * 0.72, y0 + d * 0.25
    ch.box(chx, chy, zt + rh * 0.4, 3.6, 3.6, rh * 0.9, "#9C5B3E", top=hexc("#5A3A2A"))
    cv.draw(ch)
    if state in ("busy", "waiting"):
        smoke(cv, chx + 1.8, chy + 1.8, zt + rh * 1.3, frame)
    # picket fence + flower box + mailbox
    f = OL(k=-0.5)
    for i in range(int((w + 4) // 3) + 1):
        if abs(x0 - 2 + i * 3 - (x0 + w / 2)) < 4:
            continue
        f.box(x0 - 2 + i * 3, y0 + d + 7, 0, 0.9, 0.9, 4, "#FFFFFF", hl=False)
    f.box(x0 - 2, y0 + d + 7.1, 2.6, w + 4, 0.6, 0.6, "#FFFFFF", hl=False)
    f.box(x0 + 3, y0 + d, 5, 8, 1.6, 1.4, "#9C7049", hl=False)
    cv.draw(f)
    fl = OL(outline=False)
    for i in range(6):
        fl.put(*_P(x0 + 3.5 + i * 1.3, y0 + d + 0.8, 6.8), hexc(["#E66A9A", "#F2C94C", "#FFFFFF"][i % 3]))
    cv.draw(fl)


# ------------------------------------------------------------------ 2. office tower (company / team project)
def office(cv, x0, y0, frame, state="busy", tier=1, accent="#34406B"):
    accent = main(accent)
    w, d, h = (30, 24, 44) if tier == 1 else (34, 26, 62) if tier == 2 else (38, 28, 84)
    slab(cv, x0, y0, w, d, "#C9C2B6")
    b = OL(k=-0.62)
    glass = C("#2B3A55", "#7FA7C9")
    b.box(x0, y0, 1.2, w, d, h, "#9AA6B6", top=hexc("#C9D3E0"), left=hexc("#AAB6C6"), right=hexc("#8592A4"))
    cv.draw(b)
    windows_y(cv, y0 + d, x0 + 2, x0 + w - 2, 9, h - 2, frame, lit_ratio(state), step_x=4.6, step_z=5.4, ww=3.4, wh=4, frame_col="#C9D3E0", glass=glass)
    windows_x(cv, x0 + w, y0 + 2, y0 + d - 2, 9, h - 2, frame, lit_ratio(state), step_y=4.6, step_z=5.4, ww=3.4, wh=4, frame_col="#B9C4D2", glass=glass)
    lobby = OL(k=-0.55)
    lobby.box(x0 + w / 2 - 9, y0 + d, 1.2, 18, 4, 1, "#C9C2B6", hl=False)
    lobby.face_y(y0 + d + 0.06, x0 + w / 2 - 7, 1.2, 14, 6.5, C("#3A4A66", "#BFE3FF"))
    lobby.face_y(y0 + d + 0.08, x0 + w / 2 - 0.3, 1.2, 0.6, 6.5, hexc("#FFFFFF"))
    lobby.box(x0 + w / 2 - 9, y0 + d, 8, 18, 5, 1, accent, top=lit(hexc(accent), 0.2))
    cv.draw(lobby)
    top = OL(k=-0.55)
    zt = 1.2 + h
    top.box(x0 + 3, y0 + 3, zt, 7, 6, 3.5, "#C9CED6", top=hexc("#E3E6EC"))
    top.box(x0 + 13, y0 + 4, zt, 5, 5, 2.5, "#C9CED6", top=hexc("#E3E6EC"))
    top.box(x0 + w - 6, y0 + d - 6, zt, 0.8, 0.8, 14, "#3A3632", hl=False)
    top.box(x0 + 2, y0 + d - 1, zt - 6, w - 4, 1.2, 5, accent, left=hexc(accent))
    cv.draw(top)
    bl = OL(outline=False, emissive=True)
    bl.put(*_P(x0 + w - 5.6, y0 + d - 5.6, zt + 14.4), hexc("#FF5C5C") if frame % 2 else hexc("#7A2A2A"))
    cv.draw(bl)


# ------------------------------------------------------------------ 3. garage workshop (experiments / prototypes)
def workshop(cv, x0, y0, frame, state="busy", tier=1, accent="#E9875A"):
    w, d, h = (36, 24, 14) if tier == 1 else (44, 26, 15) if tier == 2 else (54, 28, 16)
    slab(cv, x0, y0, w, d, "#9C8670")
    b = OL(k=-0.6)
    b.box(x0, y0, 1.2, w, d, h, "#B9B2A6", top=hexc("#D8D0C4"), left=hexc("#C4BDB1"))
    for i in range(int(w // 2)):
        b.face_y(y0 + d + 0.01, x0 + i * 2, 1.2, 0.3, h, hexc("#A69E92"))
    cv.draw(b)
    # roll-up door, half open when busy
    dr = OL(k=-0.55)
    open_h = {"busy": 6, "waiting": 6, "failing": 3}.get(state, 0)
    dw = min(18, w - 12)
    dr.face_y(y0 + d + 0.03, x0 + 4, 1.2, dw, 10.5, hexc("#3A3632"))
    for i in range(int((10.5 - open_h) // 1.2)):
        dr.face_y(y0 + d + 0.05, x0 + 4.4, 1.2 + open_h + i * 1.2, dw - 0.8, 1, hexc(accent if i % 2 else "#F5A57A"))
    if open_h:
        dr.face_y(y0 + d + 0.04, x0 + 4.4, 1.2, dw - 0.8, open_h, C("#FFD98A", "#4A4540"))
    cv.draw(dr)
    windows_y(cv, y0 + d, x0 + dw + 8, x0 + w - 3, 5, h - 1, frame, lit_ratio(state), step_x=6, ww=4, wh=4)
    # sawtooth roof
    rf = OL(k=-0.6)
    zt = 1.2 + h
    n = int(w // 9)
    for i in range(n):
        xa, xb = x0 + i * 9, x0 + i * 9 + 9
        rf.poly([P(xa, y0 + d, zt), P(xb, y0 + d, zt), P(xb, y0 + d, zt + 6), P(xa, y0 + d, zt)], hexc("#8C8C96"))
        rf.poly([P(xa, y0, zt), P(xa, y0 + d, zt), P(xb, y0 + d, zt + 6), P(xb, y0, zt + 6)], lit(hexc(main("#9AA3AE")), -0.05 * (i % 2)))
        rf.poly([P(xb, y0, zt), P(xb, y0 + d, zt), P(xb, y0 + d, zt + 6), P(xb, y0, zt + 6)], C("#3A4A66", "#BFE3FF"))
    cv.draw(rf)
    # workbench, crate, toolbox outside
    o = OL(k=-0.55)
    o.box(x0 + w + 3, y0 + d - 10, 0, 4, 9, 5, "#8E6542", top=hexc("#C99A6B"))
    o.box(x0 + w + 3.6, y0 + d - 8, 5, 2.4, 2.2, 1.6, "#D9534F", hl=False)
    o.box(x0 + w + 2, y0 + d + 2, 0, 5, 5, 4, "#C99A6B", top=hexc("#D9B48A"))
    o.box(x0 + 2, y0 + d + 6, 0, 1.6, 1.6, 6, "#F2C94C", top=hexc("#FFE08A"))
    cv.draw(o)
    if state == "busy" and frame % 2 == 0:
        sp = OL(outline=False, emissive=True)
        for i in range(4):
            sp.put(*_P(x0 + 10 + i, y0 + d + 2, 3 + (i * 3) % 5), hexc("#FFD24A"))
        cv.draw(sp)


# ------------------------------------------------------------------ 4. shop (client / product delivery)
def shop(cv, x0, y0, frame, state="busy", tier=1, awning=("#D9534F", "#FFFFFF")):
    w, d, h = (32, 22, 20) if tier == 1 else (40, 24, 24) if tier == 2 else (48, 26, 28)
    slab(cv, x0, y0, w, d)
    b = OL(k=-0.6)
    b.box(x0, y0, 1.2, w, d, h, "#E8D9C0", top=hexc("#F3EEE7"))
    for i in range(int(w // 3)):
        for j in range(int(h // 2)):
            if (i + j) % 2:
                b.face_y(y0 + d + 0.01, x0 + i * 3, 1.2 + j * 2, 3, 0.25, hexc("#D9C7AC"))
    cv.draw(b)
    vit = OL(outline=False, emissive=not room.DAY and state != "quiet")
    vit.face_y(y0 + d + 0.02, x0 + 2, 2, w / 2 - 5, 8, hexc("#6E4A2E"))
    vit.face_y(y0 + d + 0.04, x0 + 2.6, 2.6, w / 2 - 6.2, 6.8, C("#FFD98A", "#BFE3FF"))
    vit.face_y(y0 + d + 0.02, x0 + w / 2 + 3, 2, w / 2 - 5, 8, hexc("#6E4A2E"))
    vit.face_y(y0 + d + 0.04, x0 + w / 2 + 3.6, 2.6, w / 2 - 6.2, 6.8, C("#FFD98A", "#BFE3FF"))
    for i in range(3):
        vit.face_y(y0 + d + 0.06, x0 + 4 + i * 3, 2.6, 2, 2.4 + i, hexc(["#E9875A", "#5B7FD9", "#F2C94C"][i]))
    cv.draw(vit)
    door_y(cv, y0 + d, x0 + w / 2 - 2.5, 5, 9, "#34406B")
    windows_y(cv, y0 + d, x0 + 4, x0 + w - 3, 14, h - 1, frame, lit_ratio(state), step_x=7, ww=4, wh=4.4)
    windows_x(cv, x0 + w, y0 + 3, y0 + d - 2, 4, h - 1, frame, lit_ratio(state), step_y=7, ww=4, wh=4.4)
    # striped awning
    aw = OL(k=-0.5)
    n = int(w // 2.5)
    for i in range(n):
        col = (main(awning[0]), awning[1])[i % 2]
        xa = x0 - 1 + i * (w + 2) / n
        aw.poly([P(xa, y0 + d, 13), P(xa + (w + 2) / n, y0 + d, 13), P(xa + (w + 2) / n, y0 + d + 6, 8), P(xa, y0 + d + 6, 8)], hexc(col))
    cv.draw(aw)
    sg = OL(k=-0.55)
    sg.box(x0 + 3, y0 + d, h - 4, w - 6, 1, 4.6, "#34406B", left=hexc("#34406B"))
    sg.box(x0, y0, 1.2 + h, w, d, 1.4, "#C9B08C", top=hexc("#E3D3BC"))
    cv.draw(sg)
    pl = OL(k=-0.5)
    for px in (x0 - 3, x0 + w + 1):
        pl.box(px, y0 + d + 2, 0, 3, 3, 3, "#C4703A")
        pl.box(px - 0.4, y0 + d + 1.6, 3, 3.8, 3.8, 3.6, "#5E8A50", hl=False)
    cv.draw(pl)


# ------------------------------------------------------------------ 5. factory (backend / infrastructure)
def factory(cv, x0, y0, frame, state="busy", tier=1):
    w, d, h = (40, 26, 18) if tier == 1 else (48, 28, 20) if tier == 2 else (56, 30, 22)
    slab(cv, x0, y0, w, d, "#9C9690")
    b = OL(k=-0.6)
    b.box(x0, y0, 1.2, w, d, h, "#B05E46", top=hexc(main("#8C8C96")), left=hexc("#B9674E"), right=hexc("#94503C"))
    for i in range(int(h // 2)):
        b.face_y(y0 + d + 0.01, x0, 1.2 + i * 2, w, 0.3, hexc("#9A5240"))
    cv.draw(b)
    windows_y(cv, y0 + d, x0 + 3, x0 + w - 3, 8, h - 2, frame, lit_ratio(state), step_x=5.5, ww=3.6, wh=6, frame_col="#5A5550")
    dr = OL(k=-0.55)
    dr.face_y(y0 + d + 0.03, x0 + w - 12, 1.2, 9, 8, hexc("#5A5550"))
    for i in range(6):
        dr.face_y(y0 + d + 0.05, x0 + w - 11.6, 1.6 + i * 1.2, 8.2, 0.5, hexc("#7A7570"))
    cv.draw(dr)
    zt = 1.2 + h
    stacks = 2 if tier < 3 else 3
    for i in range(stacks):
        sx, sy = x0 + 6 + i * 9, y0 + 4
        st = OL(k=-0.6)
        for k in range(10):
            st.box(sx - 2.4 + 0.08 * k, sy - 2.4 + 0.08 * k, zt + k * 2.4, 4.8 - 0.16 * k, 4.8 - 0.16 * k, 2.4, "#C9C2B6" if k % 3 else "#D9534F", hl=False)
        cv.draw(st)
        if state in ("busy", "waiting", "failing"):
            smoke(cv, sx, sy, zt + 24, frame + i, col=(200, 200, 205) if state != "failing" else (120, 120, 125))
    tk = OL(k=-0.55)
    tx, ty = x0 + w + 4, y0 + 6
    for k in range(6):
        tk.box(tx, ty, k * 3, 8, 8, 3, "#C9CED6" if k % 2 else "#B9C0CE", hl=False)
    tk.box(tx + 0.5, ty + 0.5, 18, 7, 7, 1, "#9AA3AE", hl=False)
    tk.box(tx - 3, ty + 3, 10, 3.4, 1.2, 1.2, "#8496AD", hl=False)
    cv.draw(tk)


# ------------------------------------------------------------------ 6. observatory (data / AI)
def observatory(cv, x0, y0, frame, state="busy", tier=1):
    w, d, h = (30, 26, 16) if tier == 1 else (36, 30, 18) if tier == 2 else (42, 34, 20)
    slab(cv, x0, y0, w, d)
    b = OL(k=-0.6)
    b.box(x0, y0, 1.2, w, d, h, "#E3E6EC", top=hexc("#F4F6F9"))
    cv.draw(b)
    windows_y(cv, y0 + d, x0 + 3, x0 + w - 3, 5, h - 2, frame, lit_ratio(state), step_x=6, ww=3.2, wh=6, frame_col="#C9CED6")
    windows_x(cv, x0 + w, y0 + 3, y0 + d - 3, 5, h - 2, frame, lit_ratio(state), step_y=6, ww=3.2, wh=6, frame_col="#C9CED6")
    door_y(cv, y0 + d, x0 + w / 2 - 3, 6, 9, "#34406B")
    zt = 1.2 + h
    cx, cy = x0 + w / 2, y0 + d / 2
    R = min(w, d) / 2 - 1
    dm = OL(k=-0.55)
    for k in range(9):
        r_ = R * math.cos(k / 9 * math.pi / 2)
        dm.box(cx - r_, cy - r_, zt + k * R / 9 * 1.1, 2 * r_, 2 * r_, R / 9 * 1.1, main("#C9D3E0") if k % 2 else "#D9E0EA", hl=False)
    cv.draw(dm)
    slit = OL(outline=False)
    ang = (frame % 4) * 0.2
    for k in range(9):
        r_ = R * math.cos(k / 9 * math.pi / 2)
        slit.face_y(cy + r_ + 0.05, cx - 1 + ang * 4, zt + k * R / 9 * 1.1, 2, R / 9 * 1.1, hexc("#2B3A55"))
    cv.draw(slit)
    tel = OL(k=-0.55)
    tel.box(cx - 0.8 + ang * 4, cy + R - 2, zt + R * 0.6, 1.6, 6, 1.6, "#3A3632", hl=False)
    cv.draw(tel)
    dish = OL(k=-0.55)
    dx, dy = x0 + w + 5, y0 + d - 6
    dish.box(dx, dy, 0, 1, 1, 8, "#9AA3AE", hl=False)
    dish.poly([P(dx - 3, dy - 1, 9), P(dx + 3, dy - 1, 13), P(dx + 3, dy + 1, 13), P(dx - 3, dy + 1, 9)], hexc("#E3E6EC"))
    cv.draw(dish)
    if not room.DAY:
        st = OL(outline=False, emissive=True)
        for i in range(6):
            st.put(*_P(cx - 8 + i * 4, cy - 10 - i, zt + 30 + (i * 7) % 10), hexc("#F7F2A0") if (i + frame) % 3 else hexc("#FFFFFF"))
        cv.draw(st)


# ------------------------------------------------------------------ 7. library (docs / open source)
def library(cv, x0, y0, frame, state="busy", tier=1):
    w, d, h = (36, 24, 18) if tier == 1 else (42, 26, 20) if tier == 2 else (50, 28, 22)
    s = OL(k=-0.5)
    s.box(x0 - 3, y0 - 3, 0, w + 6, d + 6, 2, "#B5A58E", top=C("#5A5D6A", "#E3DCD0"))
    for i in range(3):
        s.box(x0 + 8 + i * 2, y0 + d + 3 + i * 1.6, 0, w - 16 - i * 4, 1.6, 2 - i * 0.6 + 0.6, "#C9C2B6", hl=False)
    s.box(x0, y0, 2, w, d, h, "#E8DCC4", top=hexc("#F3EEE7"))
    cv.draw(s)
    windows_x(cv, x0 + w, y0 + 3, y0 + d - 3, 6, h - 2, frame, lit_ratio(state), step_y=6, ww=3, wh=8, frame_col="#C9B08C")
    cols = OL(k=-0.5)
    n = int((w - 4) // 6)
    for i in range(n + 1):
        cols.box(x0 + 2 + i * (w - 6) / n, y0 + d + 0.5, 2, 2, 2, h - 3, "#FFFFFF", hl=False)
    cols.box(x0 - 1, y0 + d - 1, h - 1, w + 2, 4, 3, "#E3DCD0", top=hexc("#FFFFFF"))
    cv.draw(cols)
    door_y(cv, y0 + d, x0 + w / 2 - 3, 6, 11, "#6E3712")
    zt = 2 + h
    rf = OL(k=-0.6)
    rf.poly([P(x0 - 2, y0 + d + 2, zt), P(x0 + w + 2, y0 + d + 2, zt), P(x0 + w + 2, y0 + d / 2, zt + 9), P(x0 - 2, y0 + d / 2, zt + 9)], hexc(main("#3E9C8E")))
    rf.poly([P(x0 + w, y0, zt), P(x0 + w, y0 + d, zt), P(x0 + w, y0 + d / 2, zt + 9)], hexc("#E3DCD0"))
    cv.draw(rf)
    bk = OL(outline=False)
    for i, col in enumerate(("#E9875A", "#5B7FD9", "#F2C94C")):
        bk.face_y(y0 + d + 2.1, x0 + w / 2 - 3 + i * 2.2, zt + 1.2, 1.8, 3.4, hexc(col))
    cv.draw(bk)
    bn = OL(k=-0.55)
    bn.box(x0 - 2, y0 + d + 8, 0, 1, 4, 3, "#3A3632", hl=False)
    bn.box(x0 + 6, y0 + d + 8, 0, 1, 4, 3, "#3A3632", hl=False)
    bn.box(x0 - 3, y0 + d + 7, 3, 11, 5, 1, "#C99A6B", top=hexc("#D9B48A"))
    cv.draw(bn)


# ------------------------------------------------------------------ 8. lighthouse (ops / monitoring)
def lighthouse(cv, x0, y0, frame, state="busy", tier=1):
    hgt = 44 if tier == 1 else 54 if tier == 2 else 64
    cx, cy = x0 + 14, y0 + 12
    slab(cv, x0 + 2, y0 + 2, 24, 20, "#B9B2A0")
    hut = OL(k=-0.6)
    hut.box(x0 + 22, y0 + 6, 1.2, 14, 14, 10, "#F3EEE7", top=hexc("#FFFFFF"))
    cv.draw(hut)
    gable(cv, x0 + 22, y0 + 6, 14, 14, 11.2, main("#D9534F"), "#F3EEE7", o=1)
    windows_y(cv, y0 + 20, x0 + 25, x0 + 34, 4, 9, frame, lit_ratio(state), step_x=5, ww=3, wh=3.4)
    tw = OL(k=-0.6)
    n = int(hgt // 4)
    for k in range(n):
        r_ = 6 - k * 2.4 / n
        col = "#FFFFFF" if (k // 2) % 2 == 0 else main("#D9534F")
        tw.box(cx - r_, cy - r_, 1.2 + k * 4, 2 * r_, 2 * r_, 4, col, hl=False)
    cv.draw(tw)
    zt = 1.2 + n * 4
    lr = OL(k=-0.55)
    lr.box(cx - 4.4, cy - 4.4, zt, 8.8, 8.8, 1, "#2B2826", hl=False)
    cv.draw(lr)
    on = state in ("busy", "waiting", "failing")
    lamp = OL(outline=False, emissive=on)
    lamp.box(cx - 3, cy - 3, zt + 1, 6, 6, 5, "#FFE7A8" if on else "#8FA8C2", top=hexc("#FFF4CF") if on else hexc("#9DB3C8"), hl=False)
    cv.draw(lamp)
    cap = OL(k=-0.55)
    cap.box(cx - 3.6, cy - 3.6, zt + 6, 7.2, 7.2, 1.6, "#2B2826", hl=False)
    cap.box(cx - 1.6, cy - 1.6, zt + 7.6, 3.2, 3.2, 2, "#2B2826", hl=False)
    cv.draw(cap)
    if on:
        sx_, sy_ = _P(cx, cy, zt + 3.5)
        side = 1 if frame % 2 == 0 else -1
        length = 70
        for i in range(length):
            half = 1 + i * 0.18
            for k in range(int(-half), int(half) + 1):
                cv.blend(sx_ + side * i, sy_ - i * 0.15 + k, (255, 236, 160), 0.35 * (1 - i / length))


BUILDERS = {
    "personal": cottage,
    "team": office,
    "experiment": workshop,
    "client": shop,
    "backend": factory,
    "data": observatory,
    "docs": library,
    "ops": lighthouse,
}


def tile(kind, state="busy", tier=1, frame=0, w=170, h=150):
    town.setup(w, h, 46, 40)
    cv = room.Canvas()
    g = OL(outline=False)
    for sy in range(h):
        for sx in range(w):
            x, y = town.inv(sx, sy)
            c = C("#2E4836", "#8CBF6C")
            if town.h32(int(x * 1.4), int(y * 1.4)) % 7 == 0:
                c = lit(c, 0.08)
            g.put(sx, sy, c)
    cv.draw(g)
    BUILDERS[kind](cv, 26, 22, frame, state, tier)
    amb = 0.96 if room.DAY else 0.7
    cv.px = [[tuple(min(255, round(v * (1 if (x, y) in cv.emit else amb))) for v in c) if c else None for x, c in enumerate(row)] for y, row in enumerate(cv.px)]
    return cv
