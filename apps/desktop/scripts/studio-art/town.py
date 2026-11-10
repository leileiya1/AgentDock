"""AgentFlow town sketch: one house per project, a town hall, streets. Iso 2:1, same pixel rules as the room."""
import json, math, os, sys

ARGS = sys.argv[1:]
sys.argv = [sys.argv[0]]
import room  # noqa: E402

ps = room.ps
HERE = os.path.dirname(os.path.abspath(__file__))
hexc, lit, mix, h32, C, OL = room.hexc, room.lit, room.mix, room.h32, room.C, room.OL
P = ps.P
BAYER = [[0, 2], [3, 1]]


def setup(w, h, cx, cy):
    ps.W, ps.H = w, h
    ps.OX = w // 2 - 2 * (cx - cy)
    ps.OY = h // 2 - (cx + cy)
    room.W, room.H = w, h


def inv(sx, sy):
    a = (sx + 0.5 - ps.OX) / 2
    b = sy + 0.5 - ps.OY
    return (a + b) / 2, (b - a) / 2


# ------------------------------------------------------------------ building blocks
def house(cv, x0, y0, w, d, hgt, wall, roof, state, frame, label_out=None, name=None, signs=True):
    """state: busy | waiting | failing | quiet | archived"""
    t = frame
    DAY = room.DAY
    sl = OL(k=-0.5)
    sl.box(x0 - 2, y0 - 2, 0, w + 4, d + 4, 1.2, "#B5A58E", top=C("#5A5D6A", "#D8D0BF"), hl=False)
    cv.draw(sl)
    b = OL(k=-0.6)
    b.box(x0, y0, 1.2, w, d, hgt, wall, top=lit(hexc(wall), 0.1))
    cv.draw(b)
    lit_win = state in ("busy", "waiting", "failing") and (not DAY or state == "busy")
    win = OL(outline=False, emissive=lit_win and not DAY)
    glass_on = C("#FFD98A", "#CFE6F7") if state != "quiet" else C("#2A2C36", "#9DB3C8")
    glass_off = C("#2A2C36", "#9DB3C8")
    for i in range(int((w - 6) // 9)):
        xx = x0 + 4 + i * 9
        if abs(xx + 3 - (x0 + w / 2)) < 5:
            continue
        on = lit_win and (i + t) % 5 != 0
        win.face_y(y0 + d + 0.02, xx, 6, 6, 7, hexc("#FFFFFF"))
        win.face_y(y0 + d + 0.04, xx + 0.8, 6.8, 4.4, 5.4, glass_on if on or state == "busy" else glass_off)
        win.face_y(y0 + d + 0.06, xx + 2.8, 6.8, 0.6, 5.4, hexc("#FFFFFF"))
        if state == "archived":
            win.face_y(y0 + d + 0.08, xx - 0.5, 8, 7, 1.2, hexc("#9C7049"))
            win.face_y(y0 + d + 0.08, xx - 0.5, 10.6, 7, 1.2, hexc("#8E6542"))
    for i in range(int((d - 6) // 9)):
        yy = y0 + 4 + i * 9
        on = lit_win and (i + t + 2) % 5 != 0
        win.face_x(x0 + w + 0.02, yy, 6, 6, 7, hexc("#FFFFFF"))
        win.face_x(x0 + w + 0.04, yy + 0.8, 6.8, 4.4, 5.4, glass_on if on or state == "busy" else glass_off)
        if state == "archived":
            win.face_x(x0 + w + 0.08, yy - 0.5, 8, 7, 1.2, hexc("#9C7049"))
    cv.draw(win)
    dr = OL(k=-0.55)
    dx = x0 + w / 2 - 3
    dr.face_y(y0 + d + 0.03, dx, 1.2, 6, 10, hexc("#6E3712"))
    dr.face_y(y0 + d + 0.05, dx + 0.7, 1.2, 4.6, 9.3, hexc("#B5651D"))
    dr.face_y(y0 + d + 0.07, dx + 3.8, 5.6, 0.6, 0.8, hexc("#E6B54A"))
    dr.box(dx - 1, y0 + d, 0, 8, 2.5, 1.2, "#B5A58E", hl=False)
    cv.draw(dr)
    # gable roof (ridge along x)
    rf = OL(k=-0.6)
    zt = 1.2 + hgt
    rh = d * 0.55
    o = 2
    R = hexc(roof)
    rf.poly([P(x0 - o, y0 + d + o, zt - 0.5), P(x0 + w + o, y0 + d + o, zt - 0.5), P(x0 + w + o, y0 + d / 2, zt + rh), P(x0 - o, y0 + d / 2, zt + rh)], lambda X, Y: lit(R, -0.08) if (Y % 3 == 0) else R)
    rf.poly([P(x0 + w, y0, zt), P(x0 + w, y0 + d, zt), P(x0 + w, y0 + d / 2, zt + rh)], lit(hexc(wall), -0.12))
    rf.poly([P(x0 + w + o, y0 - o, zt - 0.5), P(x0 + w + o, y0 + d / 2, zt + rh), P(x0 + w + o + 0.01, y0 + d / 2, zt + rh - 0.01)], lit(R, -0.3))
    cv.draw(rf)
    # chimney + smoke
    ch = OL(k=-0.55)
    chx, chy = x0 + w * 0.72, y0 + d * 0.25
    ch.box(chx, chy, zt + rh * 0.4, 4, 4, rh * 0.9, "#9C5B3E", top=hexc("#5A3A2A"))
    cv.draw(ch)
    if state in ("busy", "waiting"):
        for i in range(4):
            ph = (t + i) % 4
            sx_, sy_ = P(chx + 2 + ph * 0.8, chy + 2 - ph * 0.4, zt + rh * 1.3 + 3 + ph * 4 + i * 1.5)
            r_ = 2 + ph
            for yy in range(-r_, r_ + 1):
                for xx in range(-r_ * 2, r_ * 2 + 1):
                    if xx * xx / 4 + yy * yy <= r_ * r_:
                        cv.blend(sx_ + xx, sy_ + yy, (235, 235, 240), 0.55 - ph * 0.1)
    # state props
    if state == "waiting":
        mb = OL(k=-0.55)
        mx, my = x0 + w + 4, y0 + d + 4
        mb.box(mx, my, 0, 0.8, 0.8, 9, "#6E4A2E", hl=False)
        mb.box(mx - 1.5, my - 1.5, 9, 4, 4, 3.6, "#D9534F", top=hexc("#F07A6E"))
        up = (0, 1, 0, 1)[t]
        mb.box(mx + 2.5, my - 0.5, 10 + up, 0.6, 0.6, 5, "#2B2620", hl=False)
        mb.box(mx + 2.5, my - 0.5, 14 + up, 0.6, 2.6, 2, "#F2C94C", hl=False)
        cv.draw(mb)
    if state == "failing":
        cl = OL(k=-0.35)
        cz = zt + rh + 26
        for (ox, oy, r_, dz) in ((w * 0.3, d * 0.5, 6, 0), (w * 0.55, d * 0.4, 8, 2), (w * 0.8, d * 0.55, 5, 0), (w * 0.5, d * 0.6, 6, 5)):
            for k in range(3):
                rr = r_ * (1 - k * 0.28)
                cl.box(x0 + ox - rr, y0 + oy - rr * 0.7, cz + dz + k * 2.2, rr * 2, rr * 1.4, 2.4, "#8496AD", top=hexc("#C9D3E0"), left=hexc("#A9B6C8"), hl=False)
        cv.draw(cl)
        rn = OL(outline=False)
        for i in range(14):
            rx_ = x0 + 4 + (i * 7.3) % (w - 4)
            ry_ = y0 + 4 + (i * 5.1) % (d - 4)
            rz = cz - 4 - ((t * 5 + i * 3) % 16)
            for k in range(3):
                rn.put(*P(rx_, ry_, rz - k * 1.5), hexc("#5B7FD9"))
                rn.put(*P(rx_ + 0.5, ry_, rz - k * 1.5), hexc("#7FA4F0"))
        cv.draw(rn)
    if state == "quiet":
        lv = OL(outline=False)
        lv.floor = True
        for i in range(26):
            lv.put(*P(x0 - 2 + (h32(i, 3) % int(w + 10)), y0 + d + 2 + (h32(i, 5) % 14), 0.1), hexc(["#E9873A", "#C9873A", "#D9734F"][i % 3]))
        cv.draw(lv)
    # sign in front (left of the door)
    sx0, sy0 = x0 + 2, y0 + d + 8
    if signs:
        sg = OL(k=-0.6)
        sg.box(sx0 + 1, sy0, 0, 0.8, 0.8, 17, "#6E4A2E", hl=False)
        sg.box(sx0 + 17, sy0, 0, 0.8, 0.8, 17, "#6E4A2E", hl=False)
        sg.box(sx0, sy0 - 0.2, 10, 19, 1, 8, "#34406B", left=hexc("#34406B"), top=hexc("#46528A"))
        cv.draw(sg)
        if label_out is not None:
            px, py = P(sx0 + 9.5, sy0 + 0.8, 14)
            label_out.append({"name": name, "x": px, "y": py, "state": state})


def town_hall(cv, x0, y0, frame, label_out=None):
    t = frame
    w, d, hgt = 44, 30, 26
    b = OL(k=-0.6)
    b.box(x0 - 3, y0 - 3, 0, w + 6, d + 6, 2, "#B5A58E", top=C("#5A5D6A", "#E3DCD0"))
    for i in range(3):
        b.box(x0 + 12 + i * 2, y0 + d + 3 + i * 1.6, 0, w - 24 - i * 4, 1.6, 2 - i * 0.6 + 0.6, "#C9C2B6", hl=False)
    b.box(x0, y0, 2, w, d, hgt, "#F3EEE7", top=hexc("#FFFFFF"))
    cv.draw(b)
    cols = OL(k=-0.5)
    for i in range(6):
        cols.box(x0 + 3 + i * 7.6, y0 + d + 0.5, 2, 2.4, 2.4, hgt - 4, "#FFFFFF", hl=False)
    cols.box(x0 - 1, y0 + d - 1, hgt - 2, w + 2, 4, 4, "#E3DCD0", top=hexc("#FFFFFF"))
    cv.draw(cols)
    win = OL(outline=False, emissive=not room.DAY)
    for i in range(5):
        win.face_y(y0 + d + 0.02, x0 + 6 + i * 7.6, 8, 3.4, 10, C("#FFD98A", "#9DB3C8"))
    for i in range(3):
        win.face_x(x0 + w + 0.02, y0 + 5 + i * 8.5, 8, 4, 10, C("#FFD98A", "#9DB3C8"))
    cv.draw(win)
    # pediment + clock
    rf = OL(k=-0.6)
    zt = 2 + hgt
    rf.poly([P(x0 - 2, y0 + d + 2, zt), P(x0 + w + 2, y0 + d + 2, zt), P(x0 + w + 2, y0 + d / 2, zt + 12), P(x0 - 2, y0 + d / 2, zt + 12)], hexc("#34406B"))
    rf.poly([P(x0 + w, y0, zt), P(x0 + w, y0 + d, zt), P(x0 + w, y0 + d / 2, zt + 12)], hexc("#E3DCD0"))
    cv.draw(rf)
    ck = OL(outline=False)
    for a in range(40):
        for rr in range(14):
            ck.put(*P(x0 + w / 2 + math.cos(a / 40 * math.tau) * rr / 3.5, y0 + d + 0.1, zt + 4.5 + math.sin(a / 40 * math.tau) * rr / 3.5), hexc("#FFFFFF") if rr < 12 else hexc("#2B2620"))
    for i in range(3):
        ck.put(*P(x0 + w / 2, y0 + d + 0.2, zt + 4.5 + i * 0.8), hexc("#2B2620"))
        ck.put(*P(x0 + w / 2 + i * 0.6, y0 + d + 0.2, zt + 4.5), hexc("#B8322A"))
    cv.draw(ck)
    fl = OL(k=-0.5)
    fl.box(x0 + w / 2 - 0.4, y0 + d / 2 - 0.4, zt + 12, 0.8, 0.8, 16, "#3A3632", hl=False)
    wave = (0, 0.6, 1.2, 0.6)[t]
    fl.box(x0 + w / 2 + 0.4, y0 + d / 2 - 0.4, zt + 22 + wave, 8, 0.4, 5, "#E9875A", left=hexc("#E9875A"), hl=False)
    cv.draw(fl)
    if label_out is not None:
        px, py = P(x0 + w / 2, y0 + d + 6, 8)
        label_out.append({"name": "市政厅", "x": px, "y": py, "state": "hall"})
    # newspaper kiosk
    k = OL(k=-0.55)
    k.box(x0 + w + 6, y0 + d - 2, 0, 7, 6, 9, "#3E9C8E", top=hexc("#6FC3B5"))
    k.face_y(y0 + d + 4.02, x0 + w + 7, 3, 5, 4, hexc("#F3EEE7"))
    k.box(x0 + w + 5, y0 + d - 3, 9, 9, 8, 1.2, "#E9875A", top=hexc("#F5A57A"))
    cv.draw(k)


def tree(cv, x, y, s=1.0, kind=0):
    for yy in range(-5, 6):
        for xx in range(-7, 8):
            if xx * xx / 49 + yy * yy / 25 <= 1:
                cv.blend(*P(x + xx * s, y + yy * s, 0), (10, 20, 10), 0.22)
    tr = OL(k=-0.55)
    tr.box(x - 1 * s, y - 1 * s, 0, 2 * s, 2 * s, 9 * s, "#6E4A2E", hl=False)
    cv.draw(tr)
    cn = OL(k=-0.45)
    greens = [("#2F5A30", "#4F7A45"), ("#3F6B3A", "#5E8A50"), ("#4F7A45", "#7FA86B")] if kind == 0 else [("#7A5A2E", "#C9873A"), ("#8A4A2E", "#D9734F"), ("#9A6B37", "#E9A25A")]
    for i, (dark, light) in enumerate(greens):
        r_ = (6 - i * 1.6) * s
        cn.box(x - r_, y - r_, (7 + i * 4) * s, 2 * r_, 2 * r_, 4.5 * s, C(dark, light), top=lit(C(dark, light), 0.15))
    cv.draw(cn)


def lamp(cv, x, y):
    lp = OL(k=-0.6)
    lp.box(x - 1, y - 1, 0, 2, 2, 1, "#2B2826", hl=False)
    lp.box(x - 0.4, y - 0.4, 1, 0.8, 0.8, 18, "#2B2826", hl=False)
    lp.box(x - 1.4, y - 1.4, 19, 2.8, 2.8, 2.4, "#2B2826", top=hexc("#3A3734"))
    cv.draw(lp)
    g = OL(outline=False, emissive=not room.DAY)
    g.face_y(y + 1.42, x - 1, 19.3, 2, 1.8, C("#FFE7A8", "#E8E2D0"))
    cv.draw(g)


def fence_rect(cv, x0, y0, x1, y1, gap=None):
    f = OL(k=-0.55)
    pts = []
    for x in range(int(x0), int(x1) + 1, 4):
        pts += [(x, y0), (x, y1)]
    for y in range(int(y0), int(y1) + 1, 4):
        pts += [(x0, y), (x1, y)]
    for (px, py) in pts:
        if gap and gap[0] <= px <= gap[1] and py == y1:
            continue
        f.box(px - 0.5, py - 0.5, 0, 1, 1, 5, "#E3D3BC", top=hexc("#FFFFFF"), hl=False)
    cv.draw(f)


# ------------------------------------------------------------------ the town
PROJECTS = [
    # name, plot centre, wall, roof, state, kind (building style)
    ("acme-web", (150, 150), "#F3E3C3", "#C9605F", "waiting", "client"),
    ("agentdock-desktop", (150, 48), "#E3EEF3", "#34406B", "busy", "team"),
    ("mobile-app", (48, 150), "#F4E1E8", "#8D7BD6", "failing", "experiment"),
    ("data-pipeline", (226, 150), "#E8E2D8", "#6E8B5E", "quiet", "data"),
]


def state_props(cv, x0, y0, w, d, top_z, state, frame, labels, name):
    """Mailbox flag / rain cloud / leaves + name sign, shared by every building style."""
    t = frame
    if state == "waiting":
        mb = OL(k=-0.55)
        mx, my = x0 + w + 4, y0 + d + 4
        mb.box(mx, my, 0, 0.8, 0.8, 9, "#6E4A2E", hl=False)
        mb.box(mx - 1.5, my - 1.5, 9, 4, 4, 3.6, "#D9534F", top=hexc("#F07A6E"))
        up = (0, 1, 0, 1)[t]
        mb.box(mx + 2.5, my - 0.5, 10 + up, 0.6, 0.6, 5, "#2B2620", hl=False)
        mb.box(mx + 2.5, my - 0.5, 14 + up, 0.6, 2.6, 2, "#F2C94C", hl=False)
        cv.draw(mb)
    if state == "failing":
        cl = OL(k=-0.35)
        cz = top_z + 22
        for (ox, oy, r_, dz) in ((w * 0.3, d * 0.5, 6, 0), (w * 0.55, d * 0.4, 8, 2), (w * 0.8, d * 0.55, 5, 0), (w * 0.5, d * 0.6, 6, 5)):
            for k in range(3):
                rr = r_ * (1 - k * 0.28)
                cl.box(x0 + ox - rr, y0 + oy - rr * 0.7, cz + dz + k * 2.2, rr * 2, rr * 1.4, 2.4, "#8496AD", top=hexc("#C9D3E0"), left=hexc("#A9B6C8"), hl=False)
        cv.draw(cl)
        rn = OL(outline=False)
        for i in range(14):
            rx_ = x0 + 4 + (i * 7.3) % (w - 4)
            ry_ = y0 + 4 + (i * 5.1) % (d - 4)
            rz = cz - 4 - ((t * 5 + i * 3) % 16)
            for k in range(3):
                rn.put(*P(rx_, ry_, rz - k * 1.5), hexc("#5B7FD9"))
                rn.put(*P(rx_ + 0.5, ry_, rz - k * 1.5), hexc("#7FA4F0"))
        cv.draw(rn)
    if state == "quiet":
        lv = OL(outline=False)
        lv.floor = True
        for i in range(26):
            lv.put(*P(x0 - 2 + (h32(i, 3) % int(w + 10)), y0 + d + 2 + (h32(i, 5) % 14), 0.1), hexc(["#E9873A", "#C9873A", "#D9734F"][i % 3]))
        cv.draw(lv)
    sx0, sy0 = x0 - 2, y0 + d + 10
    sg = OL(k=-0.6)
    sg.box(sx0 + 1, sy0, 0, 0.8, 0.8, 17, "#6E4A2E", hl=False)
    sg.box(sx0 + 17, sy0, 0, 0.8, 0.8, 17, "#6E4A2E", hl=False)
    sg.box(sx0, sy0 - 0.2, 10, 19, 1, 8, "#34406B", left=hexc("#34406B"), top=hexc("#46528A"))
    cv.draw(sg)
    px, py = P(sx0 + 9.5, sy0 + 0.8, 14)
    labels.append({"name": name, "x": px, "y": py, "state": state})


def scene(frame, w=640, h=420, cx=112, cy=112):
    setup(w, h, cx, cy)
    cv = room.Canvas()
    t = frame
    DAY = room.DAY
    g = OL(outline=False)
    g.floor = True
    for sy in range(h):
        for sx in range(w):
            x, y = inv(sx, sy)
            n = h32(int(x * 1.4), int(y * 1.4))
            c = C("#2E4836", "#8CBF6C")
            if n % 7 == 0:
                c = lit(c, 0.08)
            elif n % 11 == 0:
                c = lit(c, -0.1)
            fl_n = h32(sx, sy, 17) % 241
            if fl_n == 0:
                c = hexc("#F2C94C")
            elif fl_n == 1:
                c = hexc("#FFFFFF") if DAY else hexc("#7A8A9A")
            # roads: along y at x 96..108 and 184..196, along x at y 96..108 and 184..196
            road_x = (96 <= x <= 108) or (184 <= x <= 196)
            road_y = (96 <= y <= 108) or (184 <= y <= 196)
            walk_x = (92 <= x <= 112) or (180 <= x <= 200)
            walk_y = (92 <= y <= 112) or (180 <= y <= 200)
            if walk_x or walk_y:
                c = C("#4A4C58", "#D8D0BF")
                if int(x) % 5 == 0 or int(y) % 5 == 0:
                    c = lit(c, -0.1)
            if road_x or road_y:
                c = C("#2B2D36", "#6B6F7A")
                if h32(int(x * 2), int(y * 2)) % 9 == 0:
                    c = lit(c, 0.06)
                if road_x and not road_y and 101.6 <= x <= 102.4 and int(y) % 8 < 4:
                    c = hexc("#F2C94C")
                if road_x and not road_y and 189.6 <= x <= 190.4 and int(y) % 8 < 4:
                    c = hexc("#F2C94C")
                if road_y and not road_x and 101.6 <= y <= 102.4 and int(x) % 8 < 4:
                    c = hexc("#F2C94C")
                if road_y and not road_x and 189.6 <= y <= 190.4 and int(x) % 8 < 4:
                    c = hexc("#F2C94C")
                # zebra crossing near the square
                if road_y and 110 <= x <= 116 and int(y) % 3 == 0 and 96 < y < 108:
                    c = hexc("#E8E8E8")
            # town square (paved circle) in front of the hall
            sq = ((x - 70) / 18) ** 2 + ((y - 70) / 18) ** 2
            if sq < 1 and not (road_x or road_y):
                c = C("#55576A", "#CBBFA8")
                if (int(x) // 3 + int(y) // 3) % 2 == 0:
                    c = lit(c, -0.06)
            # construction plot (front-left, new project)
            if 126 <= x <= 174 and 206 <= y <= 250:
                c = C("#5A4636", "#B08A62")
                if h32(int(x * 2), int(y * 2)) % 5 == 0:
                    c = lit(c, -0.12)
            # pond in the park
            pd = ((x - 230) / 16) ** 2 + ((y - 56) / 10) ** 2
            if pd < 1.12:
                c = C("#55576A", "#B9B2A0")
            if pd < 1:
                c = C("#1F3E5C", "#5FA8D6")
                if (sx + sy * 2 + t * 3) % 23 == 0:
                    c = C("#5C8FBF", "#BFE6FA")
            g.put(sx, sy, c)
    cv.draw(g)

    labels = []
    items = []

    def add(k, fn):
        items.append((k, fn))

    # town hall + square
    add(48 + 48, lambda: town_hall(cv, 26, 22, frame, labels))
    def statue():
        s = OL(k=-0.55)
        s.box(66, 66, 0, 8, 8, 4, "#C9C2B6", top=hexc("#E3DCD0"))
        s.box(67.5, 67.5, 4, 5, 5, 3, "#B9B2A0")
        room.robot_front(cv, 65.75, 67, "runner", "done", 0, z0=7)
        cv.draw(s)
    add(140, statue)

    # houses
    import buildings as BLD
    SIZE = {"client": (40, 24, 24), "team": (34, 26, 62), "experiment": (44, 26, 21), "data": (36, 30, 34)}
    for name, (pcx, pcy), wall, roof, state, kind in PROJECTS:
        bw, bd, bt = SIZE[kind]
        def build(n=name, a=pcx, b=pcy, st=state, kd=kind, ww=bw, dd=bd, tt=bt):
            x0, y0 = a - ww / 2, b - dd / 2 - 4
            BLD.BUILDERS[kd](cv, x0, y0, frame, st, 2)
            state_props(cv, x0, y0, ww, dd, tt, st, frame, labels, n)
        add(pcx + pcy, build)
        add(pcx + pcy - 40, lambda a=pcx, b=pcy: fence_rect(cv, a - 34, b - 34, a + 34, b + 32, gap=(a - 8, a + 8)))
        for (tx, ty, k_) in ((pcx - 28, pcy - 26, 0), (pcx + 26, pcy - 24, 1 if state == "quiet" else 0)):
            add(tx + ty, lambda a=tx, b=ty, kk=k_: tree(cv, a, b, 0.9, kk))

    # construction plot for the new project
    def construction():
        x0, y0 = 128, 208
        sc = OL(k=-0.55)
        sc.box(x0 + 6, y0 + 6, 0, 34, 26, 2, "#9C8670", top=hexc("#B5A58E"))
        for (px, py) in ((x0 + 6, y0 + 6), (x0 + 40, y0 + 6), (x0 + 6, y0 + 32), (x0 + 40, y0 + 32)):
            sc.box(px - 0.6, py - 0.6, 0, 1.2, 1.2, 26, "#C9CED6", hl=False)
        sc.box(x0 + 5.4, y0 + 5.4, 13, 35.2, 1.2, 1, "#C9CED6", hl=False)
        sc.box(x0 + 5.4, y0 + 5.4, 25, 35.2, 1.2, 1, "#C9CED6", hl=False)
        sc.box(x0 + 5.4, y0 + 5.4, 13, 1.2, 27.2, 1, "#C9CED6", hl=False)
        sc.box(x0 + 5.4, y0 + 5.4, 25, 1.2, 27.2, 1, "#C9CED6", hl=False)
        sc.box(x0 + 10, y0 + 10, 2, 20, 4, 8, "#E3D3BC")
        sc.box(x0 + 16, y0 + 18, 2, 12, 4, 1.4, "#C99A6B")
        cv.draw(sc)
        crane = OL(k=-0.55)
        crane.box(x0 + 44, y0 + 2, 0, 3, 3, 52, "#F2C94C", hl=False)
        swing = (0, 2, 4, 2)[t]
        crane.box(x0 + 22 + swing, y0 + 2.6, 52, 30, 1.8, 2, "#F2C94C", hl=False)
        crane.box(x0 + 26 + swing, y0 + 3, 30, 0.4, 0.4, 22, "#3A3632", hl=False)
        crane.box(x0 + 24.5 + swing, y0 + 1.5, 26, 3.4, 3.4, 4, "#C99A6B")
        cv.draw(crane)
        cones = OL(k=-0.5)
        for i in range(4):
            cones.box(x0 + 8 + i * 9, y0 + 36, 0, 3, 3, 1, "#E9875A")
            cones.box(x0 + 8.7 + i * 9, y0 + 36.7, 1, 1.6, 1.6, 4, "#F28A3C", top=hexc("#FFFFFF"))
        cv.draw(cones)
        sg = OL(k=-0.6)
        sg.box(x0 + 2, y0 + 40, 0, 0.8, 0.8, 17, "#6E4A2E", hl=False)
        sg.box(x0 + 18, y0 + 40, 0, 0.8, 0.8, 17, "#6E4A2E", hl=False)
        sg.box(x0 + 1, y0 + 39.8, 10, 19, 1, 8, "#E9875A", left=hexc("#E9875A"))
        cv.draw(sg)
        px, py = P(x0 + 10.5, y0 + 40.6, 14)
        labels.append({"name": "docs-site", "x": px, "y": py, "state": "new"})
    add(128 + 208 + 40, construction)

    # park: trees around the pond, benches, lamps along the streets
    for (tx, ty, k_) in ((212, 36, 0), (250, 40, 1), (246, 76, 0), (208, 74, 0), (36, 210, 0), (60, 236, 1), (232, 226, 0), (20, 100, 0), (100, 20, 1)):
        add(tx + ty, lambda a=tx, b=ty, kk=k_: tree(cv, a, b, 1.0, kk))
    for (lx, ly) in ((92, 60), (92, 140), (112, 60), (112, 140), (60, 92), (140, 92), (60, 112), (140, 112), (180, 140), (200, 60), (140, 180), (60, 180)):
        add(lx + ly, lambda a=lx, b=ly: lamp(cv, a, b))
    def bench(x, y):
        bn = OL(k=-0.55)
        bn.box(x, y, 0, 1, 5, 3, "#3A3632", hl=False)
        bn.box(x + 9, y, 0, 1, 5, 3, "#3A3632", hl=False)
        bn.box(x - 1, y - 1, 3, 12, 6, 1.2, "#C99A6B", top=hexc("#D9B48A"))
        bn.box(x - 1, y + 4, 4.2, 12, 1, 5, "#B98A5E")
        cv.draw(bn)
    add(276, lambda: bench(216, 64))
    add(150, lambda: bench(72, 82))

    # robots on the streets: Claude walking from acme-web toward agentdock, a mail robot to the hall
    def walker():
        st = (0, 1.5, 3, 4.5)[t]
        room.robot_front(cv, 150 + 0, 100 - 18 - st, "claude", "walk", t, back=True)
    add(232, walker)
    def mailbot():
        st = (0, 1.5, 3, 4.5)[t]
        mx, my = 98, 140 - 30 - st
        room.robot_front(cv, mx, my, "runner", "walk", t, back=True)
        lt = OL(k=-0.5)
        lt.box(mx + 1, my + 2, 24, 6, 4, 3, "#FFFFFF", top=hexc("#F3EEE7"))
        lt.box(mx + 3.4, my + 6, 25, 1.2, 0.05, 1.2, "#D9534F", hl=False)
        cv.draw(lt)
    add(206, mailbot)
    if not DAY:
        ff = OL(outline=False, emissive=True)
        for i in range(14):
            fx_ = (h32(i, 3) % 260)
            fy_ = (h32(i, 5) % 260)
            if (i + t) % 3:
                ff.put(*P(fx_, fy_, 6 + (i + t) % 4), hexc("#F7F2A0"))
        cv.draw(ff)

    items.sort(key=lambda it: it[0])
    for _, fn in items:
        fn()

    # lighting
    lights = []
    if not DAY:
        for (lx, ly) in ((92, 60), (92, 140), (112, 60), (112, 140), (60, 92), (140, 92), (60, 112), (140, 112), (180, 140), (200, 60), (140, 180), (60, 180)):
            sx_, sy_ = P(lx, ly, 20)
            lights.append((sx_, sy_ + 16, 46, 0.55, (255, 220, 150)))
        for _, (pcx, pcy), _, _, st, _k in PROJECTS:
            if st != "quiet":
                sx_, sy_ = P(pcx, pcy + 14, 8)
                lights.append((sx_, sy_, 60, 0.35, (255, 210, 140)))
        sx_, sy_ = P(48, 52, 10)
        lights.append((sx_, sy_ + 10, 80, 0.4, (255, 220, 160)))
    amb = 0.95 if DAY else 0.6
    out = [[None] * w for _ in range(h)]
    for sy in range(h):
        for sx in range(w):
            c = cv.px[sy][sx]
            if c is None:
                continue
            if (sx, sy) in cv.emit:
                out[sy][sx] = c
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
            vx_, vy_ = (sx - w / 2) / (w / 2), (sy - h / 2) / (h / 2)
            vr = math.hypot(vx_, vy_)
            if vr > 0.75:
                lv *= 1 - 0.2 * min(1, (vr - 0.75) / 0.6)
            step_ = 0.07
            bias = (BAYER[sy % 2][sx % 2] / 4 - 0.375) * step_
            lq = min(math.floor((lv + bias) / step_) * step_, 1.25)
            out[sy][sx] = tuple(max(0, min(255, round(c[i] * lq + tint[i] * 0.45))) for i in range(3))
    cv.px = out
    return cv, labels


def legend_tile(state, frame=0):
    """A single house on a small plot, for the state legend."""
    w, h = 150, 120
    setup(w, h, 40, 40)
    cv = room.Canvas()
    g = OL(outline=False)
    for sy in range(h):
        for sx in range(w):
            x, y = inv(sx, sy)
            c = C("#2E4836", "#8CBF6C")
            if h32(int(x * 1.4), int(y * 1.4)) % 7 == 0:
                c = lit(c, 0.08)
            g.put(sx, sy, c)
    cv.draw(g)
    if state == "new":
        sc = OL(k=-0.55)
        sc.box(22, 26, 0, 30, 22, 2, "#9C8670", top=hexc("#B5A58E"))
        for (px, py) in ((22, 26), (52, 26), (22, 48), (52, 48)):
            sc.box(px - 0.6, py - 0.6, 0, 1.2, 1.2, 22, "#C9CED6", hl=False)
        sc.box(21.4, 25.4, 11, 31.2, 1.2, 1, "#C9CED6", hl=False)
        sc.box(21.4, 25.4, 21, 1.2, 23.2, 1, "#C9CED6", hl=False)
        sc.box(26, 30, 2, 16, 4, 7, "#E3D3BC")
        for i in range(3):
            sc.box(24 + i * 9, 52, 0, 3, 3, 1, "#E9875A")
            sc.box(24.7 + i * 9, 52.7, 1, 1.6, 1.6, 4, "#F28A3C", top=hexc("#FFFFFF"))
        cv.draw(sc)
    else:
        house(cv, 20, 24, 36, 24, 20, "#F3E3C3", "#C9605F", state, frame, signs=False)
    cv.px = [[tuple(min(255, round(v * (0.95 if room.DAY else 0.7))) for v in c) if c else None for c in row] for row in cv.px]
    return cv


if __name__ == "__main__":
    room.DAY = "night" not in ARGS
    tag = "town_day" if room.DAY else "town_night"
    frames, labels = [], None
    for f in range(4):
        cv, lb = scene(f)
        frames.append(cv)
        labels = labels or lb
    W, H = ps.W, ps.H
    sheet = []
    for y in range(H):
        row = []
        for c in frames:
            row.extend(c.px[y])
        sheet.append(row)
    ps.write_png(os.path.join(HERE, tag + "_sheet.png"), W * 4, H, sheet)
    big = []
    for y in range(H):
        r = []
        for c in frames[0].px[y]:
            r.extend([c or (0, 0, 0)] * 2)
        big.append(r)
        big.append(r)
    ps.write_png(os.path.join(HERE, tag + "_x2.png"), W * 2, H * 2, big)
    json.dump({"W": W, "H": H, "labels": labels}, open(os.path.join(HERE, tag + ".json"), "w"), ensure_ascii=False)
    if room.DAY:
        tiles = {}
        for st in ("busy", "waiting", "failing", "quiet", "new", "archived"):
            fr = [legend_tile(st, f) for f in range(4)]
            rows = []
            for y in range(ps.H):
                row = []
                for c in fr:
                    row.extend(c.px[y])
                rows.append(row)
            ps.write_png(os.path.join(HERE, f"tile_{st}.png"), ps.W * 4, ps.H, rows)
    print("ok", W, H, len(labels))
