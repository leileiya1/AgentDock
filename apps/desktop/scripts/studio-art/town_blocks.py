"""City blocks for the AgentFlow town: one 96 x 96 ground tile per block (half a road on every side),
project lots and public facilities. Block-local coordinates: x, y in 0..96, lawn inside 11..85.
"""
import math

import town

room, ps = town.room, town.ps
hexc, lit, mix, h32, C, OL, P = town.hexc, town.lit, town.mix, town.h32, town.C, town.OL, town.P
BLOCK = 96
TOP = 170  # headroom above the tile for tall things (ferris wheel, stands)
TILE_W, TILE_H = 4 * BLOCK + 8, TOP + 2 * BLOCK + 8
TILE_OX, TILE_OY = 2 * BLOCK + 4, TOP + 4


def new_tile():
    ps.W, ps.H, ps.OX, ps.OY = TILE_W, TILE_H, TILE_OX, TILE_OY
    room.W, room.H = TILE_W, TILE_H
    return room.Canvas()


def inv(sx, sy):
    a = (sx + 0.5 - ps.OX) / 2
    b = sy + 0.5 - ps.OY
    return (a + b) / 2, (b - a) / 2


# ------------------------------------------------------------------ ground
def ground(cv, frame, lawn="grass"):
    t = frame
    g = OL(outline=False)
    g.floor = True
    for sy in range(TILE_H):
        for sx in range(TILE_W):
            x, y = inv(sx, sy)
            if not (0 <= x < BLOCK and 0 <= y < BLOCK):
                continue
            road = x < 8 or x >= 88 or y < 8 or y >= 88
            walk = x < 11 or x >= 85 or y < 11 or y >= 85
            if road:
                c = C("#2B2D36", "#6B6F7A")
                if h32(int(x * 2), int(y * 2)) % 9 == 0:
                    c = lit(c, 0.06)
                inter_x = x < 8 or x >= 88
                inter_y = y < 8 or y >= 88
                if inter_x and not inter_y and (x < 0.7 or x > 95.3) and int(y) % 8 < 4:
                    c = hexc("#F2C94C")
                if inter_y and not inter_x and (y < 0.7 or y > 95.3) and int(x) % 8 < 4:
                    c = hexc("#F2C94C")
                # zebra crossings next to the corners
                if inter_y and not inter_x and (11 <= x <= 16 or 80 <= x <= 85) and int(y) % 3 == 0:
                    c = C("#BFBFBF", "#E8E8E8")
            elif walk:
                c = C("#4A4C58", "#D8D0BF")
                if int(x) % 4 == 0 or int(y) % 4 == 0:
                    c = lit(c, -0.1)
            else:
                n = h32(int(x * 1.4), int(y * 1.4))
                if lawn == "dirt":
                    c = C("#5A4636", "#B08A62")
                elif lawn == "paved":
                    c = C("#55576A", "#CBBFA8")
                    if (int(x) // 3 + int(y) // 3) % 2 == 0:
                        c = lit(c, -0.06)
                else:
                    c = C("#2E4836", "#8CBF6C")
                    if n % 7 == 0:
                        c = lit(c, 0.08)
                    elif n % 11 == 0:
                        c = lit(c, -0.1)
                    fl_n = h32(sx, sy, 17) % 251
                    if fl_n == 0:
                        c = hexc("#F2C94C")
                    elif fl_n == 1:
                        c = hexc("#FFFFFF") if room.DAY else hexc("#7A8A9A")
            g.put(sx, sy, c)
    cv.draw(g)


def lamp(cv, x, y):
    town.lamp(cv, x, y)


def bench(cv, x, y, facing="y"):
    b = OL(k=-0.55)
    if facing == "y":
        b.box(x, y, 0, 1, 4, 3, "#3A3632", hl=False)
        b.box(x + 8, y, 0, 1, 4, 3, "#3A3632", hl=False)
        b.box(x - 1, y - 1, 3, 10, 5, 1, "#C99A6B", top=hexc("#D9B48A"))
        b.box(x - 1, y - 1, 4, 10, 1, 4, "#B98A5E")
    else:
        b.box(x, y, 0, 4, 1, 3, "#3A3632", hl=False)
        b.box(x, y + 8, 0, 4, 1, 3, "#3A3632", hl=False)
        b.box(x - 1, y - 1, 3, 5, 10, 1, "#C99A6B", top=hexc("#D9B48A"))
        b.box(x - 1, y - 1, 4, 1, 10, 4, "#B98A5E")
    cv.draw(b)


def flowerbed(cv, x0, y0, w, d, frame=0):
    f = OL(k=-0.45)
    f.box(x0, y0, 0, w, d, 1.4, "#7A5536", top=C("#3A2C22", "#6E4A2E"))
    cv.draw(f)
    dots = OL(outline=False)
    cols = ["#E66A9A", "#F2C94C", "#FFFFFF", "#E9875A", "#8D7BD6"]
    for i in range(int(w * d / 3)):
        fx = x0 + 0.6 + (h32(i, x0, 3) % 100) / 100 * (w - 1.2)
        fy = y0 + 0.6 + (h32(i, y0, 5) % 100) / 100 * (d - 1.2)
        dots.put(*P(fx, fy, 1.6 + (i + frame) % 2 * 0.4), hexc(cols[i % len(cols)]))
    cv.draw(dots)


def hedge(cv, x0, y0, x1, y1, gap=None):
    h = OL(k=-0.45)
    if y0 == y1:
        segs = [(x0, x1)] if not gap else [(x0, gap[0]), (gap[1], x1)]
        for a, b in segs:
            if b > a:
                h.box(a, y0, 0, b - a, 2.4, 3, C("#2F5A30", "#4F7A45"), top=C("#3F6B3A", "#6E9B5C"), hl=False)
    else:
        h.box(x0, y0, 0, 2.4, y1 - y0, 3, C("#2F5A30", "#4F7A45"), top=C("#3F6B3A", "#6E9B5C"), hl=False)
    cv.draw(h)


def run_items(cv, items):
    items.sort(key=lambda it: it[0])
    for _, fn in items:
        fn()


def with_lamps(cv, items):
    for (lx, ly) in ((9.5, 9.5), (86.5, 9.5), (9.5, 86.5), (86.5, 86.5)):
        items.append((lx + ly + 0.5, lambda a=lx, b=ly: lamp(cv, a, b)))


# ------------------------------------------------------------------ project lot (building drawn separately)
def project_lot(cv, frame):
    ground(cv, frame)
    items = []
    with_lamps(cv, items)
    items.append((10, lambda: hedge(cv, 14, 14, 82, 14)))
    items.append((50, lambda: hedge(cv, 14, 14, 14, 82)))
    items.append((90, lambda: hedge(cv, 82, 14, 82, 82)))
    items.append((160, lambda: hedge(cv, 14, 82, 82, 82, gap=(42, 54))))

    def path():
        p = OL(outline=False)
        p.floor = True
        for i in range(10):
            p.flat(44.5, 58 + i * 2.6, 0.03, 7, 1.8, C("#55576A", "#CBBFA8"))
        cv.draw(p)
    items.append((-5, path))
    items.append((140, lambda: flowerbed(cv, 18, 76, 20, 4, frame)))
    items.append((150, lambda: flowerbed(cv, 58, 76, 20, 4, frame)))

    def sign():
        sg = OL(k=-0.6)
        sx0, sy0 = 22, 70
        sg.box(sx0 + 1, sy0, 0, 0.8, 0.8, 17, "#6E4A2E", hl=False)
        sg.box(sx0 + 17, sy0, 0, 0.8, 0.8, 17, "#6E4A2E", hl=False)
        sg.box(sx0, sy0 - 0.2, 10, 19, 1, 8, "#34406B", left=hexc("#34406B"), top=hexc("#46528A"))
        cv.draw(sg)
    items.append((92, sign))
    items.append((126, lambda: bench(cv, 64, 66)))
    run_items(cv, items)


SIGN_ANCHOR = (31.5, 70.8, 14)  # project name label (block-local x, y, z)
MAILBOX = (60, 62)
YARD = [(26, 60), (34, 64), (62, 60), (70, 62), (30, 76), (66, 72), (40, 62), (56, 66), (24, 68), (72, 70)]


# ------------------------------------------------------------------ facilities
def hall_block(cv, frame):
    ground(cv, frame, "paved")
    items = []
    with_lamps(cv, items)
    items.append((70, lambda: town.town_hall(cv, 26, 16, frame)))

    def statue():
        s = OL(k=-0.55)
        s.box(44, 62, 0, 8, 8, 4, "#C9C2B6", top=hexc("#E3DCD0"))
        cv.draw(s)
        room.robot_front(cv, 43.75, 63, "runner", "done", 0, z0=4)
    items.append((120, statue))
    items.append((110, lambda: bench(cv, 22, 64)))
    items.append((150, lambda: bench(cv, 66, 72)))
    for (fx, fy) in ((18, 76), (70, 58)):
        items.append((fx + fy, lambda a=fx, b=fy: flowerbed(cv, a, b, 8, 6, frame)))
    run_items(cv, items)


def park_block(cv, frame):
    ground(cv, frame)
    t = frame
    w = OL(outline=False)
    w.floor = True
    for sy in range(TILE_H):
        for sx in range(TILE_W):
            x, y = inv(sx, sy)
            d = ((x - 48) / 24) ** 2 + ((y - 50) / 16) ** 2
            if d < 1.12 and 11 <= x < 85 and 11 <= y < 85:
                c = C("#55576A", "#B9B2A0")
                if d < 1:
                    c = C("#1F3E5C", "#5FA8D6")
                    if (sx + sy * 2 + t * 5) % 19 == 0:
                        c = C("#5C8FBF", "#BFE6FA")
                w.put(sx, sy, c)
    cv.draw(w)
    items = []
    with_lamps(cv, items)

    def fountain():
        f = OL(k=-0.5)
        f.box(44, 46, 0, 8, 8, 2, "#C9C2B6", top=C("#2F6FA0", "#7FC4E8"))
        f.box(46.5, 48.5, 2, 3, 3, 6, "#C9C2B6", hl=False)
        f.box(45.5, 47.5, 8, 5, 5, 1, "#C9C2B6", top=C("#2F6FA0", "#7FC4E8"), hl=False)
        cv.draw(f)
        dr = OL(outline=False, emissive=not room.DAY)
        for i in range(8):
            a = i / 8 * math.tau + t * 0.4
            r_ = 3 + (t % 2)
            dr.put(*P(48 + math.cos(a) * r_, 50 + math.sin(a) * r_, 9 - (t % 2) * 2), C("#9FD4FF", "#E6F6FF"))
        dr.put(*P(48, 50, 11 + (t % 2)), C("#9FD4FF", "#FFFFFF"))
        cv.draw(dr)
    items.append((98, fountain))

    def ducks():
        d = OL(k=-0.5)
        for i, (dx, dy) in enumerate(((36, 54), (60, 46))):
            o = (0, 0.8)[(t + i) % 2]
            d.box(dx + o, dy, 0, 2.6, 1.8, 1.6, "#FFFFFF", hl=False)
            d.box(dx - 0.6 + o, dy, 1.4, 1, 1, 1, "#FFFFFF", hl=False)
            d.put(*P(dx - 1 + o, dy + 0.4, 2.1), hexc("#F2B45A"))
        cv.draw(d)
    items.append((96, ducks))
    for (tx, ty, k) in ((18, 20, 0), (32, 16, 1), (70, 18, 0), (80, 30, 0), (18, 40, 0), (78, 72, 1), (20, 78, 0)):
        items.append((tx + ty, lambda a=tx, b=ty, kk=k: town.tree(cv, a, b, 0.9, kk)))
    items.append((140, lambda: bench(cv, 40, 72)))
    items.append((100, lambda: bench(cv, 74, 46, "x")))
    items.append((150, lambda: flowerbed(cv, 54, 74, 12, 6, t)))
    run_items(cv, items)


def amusement_block(cv, frame):
    ground(cv, frame, "paved")
    t = frame
    items = []
    with_lamps(cv, items)

    def wheel():
        # the wheel faces the camera: horizontal axis (1, -1, 0), vertical axis z
        cx, cy, cz, R = 42, 38, 46, 38
        a = R / 2.83

        def at(c, s_):
            return cx + c * a * 0.707, cy - c * a * 0.707, cz + s_ * R

        sp = OL(k=-0.55)
        for side in (-1, 1):
            for k in range(20):
                u = k / 20
                bx, by, _ = at(side * 0.55 * (1 - u), 0)
                sp.box(bx - 0.6, by + 2.5 - 0.6, u * cz, 1.2, 1.2, cz / 20 + 0.2, "#9AA3AE", hl=False)
                sp.box(bx - 0.6, by - 2.5 - 0.6, u * cz, 1.2, 1.2, cz / 20 + 0.2, "#9AA3AE", hl=False)
        sp.box(cx - 2, cy - 2, cz - 2, 4, 4, 4, "#3A3632", hl=False)
        cv.draw(sp)
        rim = OL(outline=False, emissive=not room.DAY)
        for i in range(720):
            ang = i / 720 * math.tau
            x, y, z = at(math.cos(ang), math.sin(ang))
            rim.put(*P(x, y, z), hexc("#E9875A"))
            x2, y2, z2 = at(math.cos(ang) * 0.96, math.sin(ang) * 0.96)
            rim.put(*P(x2, y2, z2), hexc("#F5A57A"))
        for k in range(12):
            ang = k / 12 * math.tau + t * math.pi / 12
            for r_ in range(0, 60):
                x, y, z = at(math.cos(ang) * r_ / 60, math.sin(ang) * r_ / 60)
                rim.put(*P(x, y, z), hexc("#F2C94C") if room.DAY else hexc("#FFE08A"))
        cv.draw(rim)
        cab = OL(k=-0.5, emissive=not room.DAY)
        cols = ["#D9534F", "#5B7FD9", "#3E9C8E", "#F2C94C", "#8D7BD6", "#E9875A", "#D9719B", "#6E8B5E", "#5B7FD9", "#F2C94C", "#D9534F", "#3E9C8E"]
        for k in range(12):
            ang = k / 12 * math.tau + t * math.pi / 12
            x, y, z = at(math.cos(ang), math.sin(ang))
            cab.box(x - 1.4, y - 1.4, z - 5, 2.8, 2.8, 3.6, cols[k], hl=False)
        cv.draw(cab)
    items.append((76, wheel))

    def carousel():
        c = OL(k=-0.55)
        cx, cy = 66, 64
        c.box(cx - 9, cy - 9, 0, 18, 18, 1.5, "#C9C2B6", top=hexc("#E3DCD0"))
        for k in range(8):
            ang = k / 8 * math.tau + t * 0.3
            c.box(cx + math.cos(ang) * 6.5 - 0.3, cy + math.sin(ang) * 6.5 - 0.3, 1.5, 0.6, 0.6, 9, "#F2C94C", hl=False)
            c.box(cx + math.cos(ang) * 6.5 - 1.2, cy + math.sin(ang) * 6.5 - 0.8, 4 + (k + t) % 2, 2.4, 1.6, 2, ["#FFFFFF", "#E9875A"][k % 2], hl=False)
        for k in range(6):
            r_ = 10 - k * 1.7
            c.box(cx - r_, cy - r_, 10.5 + k * 1.4, 2 * r_, 2 * r_, 1.4, "#D9534F" if k % 2 == 0 else "#FFFFFF", hl=False)
        c.box(cx - 0.5, cy - 0.5, 19, 1, 1, 3, "#F2C94C", hl=False)
        cv.draw(c)
    items.append((130, carousel))

    def booth_balloons():
        b = OL(k=-0.55)
        b.box(16, 68, 0, 8, 6, 9, "#5B7FD9", top=hexc("#FFFFFF"))
        b.face_y(74.02, 17, 4, 6, 3, C("#FFD98A", "#BFE3FF"))
        cv.draw(b)
        bl = OL(outline=False)
        for i, col in enumerate(("#D9534F", "#F2C94C", "#5B7FD9", "#3E9C8E", "#D9719B")):
            bx, by = 28 + i * 1.6, 74
            bz = 16 + (i * 3) % 5 + (t + i) % 2
            for z in range(4, int(bz)):
                bl.put(*P(bx, by, z), hexc("#8C8C8C"))
            for dx in (-1, 0, 1):
                for dz in (-1, 0, 1):
                    if dx * dx + dz * dz < 2:
                        bl.put(*P(bx + dx * 0.6, by, bz + dz), hexc(col))
        cv.draw(bl)
    items.append((142, booth_balloons))
    run_items(cv, items)


def stadium_block(cv, frame):
    ground(cv, frame)
    t = frame
    f = OL(outline=False)
    f.floor = True
    for sy in range(TILE_H):
        for sx in range(TILE_W):
            x, y = inv(sx, sy)
            if 18 <= x <= 78 and 30 <= y <= 78:
                c = C("#2F5A30", "#5FA34A") if int((x - 18) // 6) % 2 else C("#3A6A38", "#6DB356")
                if abs(x - 48) < 0.5 or x < 18.6 or x > 77.4 or y < 30.6 or y > 77.4:
                    c = hexc("#FFFFFF")
                if abs(math.hypot(x - 48, y - 54) - 7) < 0.4:
                    c = hexc("#FFFFFF")
                f.put(sx, sy, c)
    cv.draw(f)
    items = []
    with_lamps(cv, items)

    def stands():
        s = OL(k=-0.55)
        for k in range(4):
            s.box(16, 14 + k * 3, 0, 64, 3, 8 - k * 2, "#C9CED6" if k % 2 else "#B9C0CE", top=hexc(["#D9534F", "#5B7FD9", "#F2C94C", "#3E9C8E"][k]), hl=False)
        cv.draw(s)
        crowd = OL(outline=False)
        for i in range(60):
            x = 17 + (i * 7) % 62
            row = i % 4
            crowd.put(*P(x, 15 + row * 3, 9 - row * 2 + (t + i) % 2), hexc(["#E9875A", "#FFFFFF", "#5B7FD9", "#F2C94C"][i % 4]))
        cv.draw(crowd)
    items.append((40, stands))

    def goals():
        g = OL(k=-0.5)
        for gx in (18, 76):
            g.box(gx, 50, 0, 2, 0.6, 5, "#FFFFFF", hl=False)
            g.box(gx, 58, 0, 2, 0.6, 5, "#FFFFFF", hl=False)
            g.box(gx, 50, 5, 2, 8.6, 0.6, "#FFFFFF", hl=False)
        bx = 44 + (0, 2, 4, 2)[t % 4]
        g.box(bx, 55, 0, 1.4, 1.4, 1.4, "#FFFFFF", hl=False)
        cv.draw(g)
        for (px, py) in ((48, 46), (40, 60)):
            room.robot_front(cv, px + (t % 2), py, "deepseek" if px == 48 else "qoder", "walk", t)
    items.append((110, goals))

    def floodlights():
        fl = OL(k=-0.55)
        for (x, y) in ((14, 28), (82, 28)):
            fl.box(x, y, 0, 1, 1, 34, "#9AA3AE", hl=False)
            fl.box(x - 2, y - 1, 34, 5, 2, 3, "#3A3632", hl=False)
        cv.draw(fl)
        if not room.DAY:
            g = OL(outline=False, emissive=True)
            for (x, y) in ((14, 28), (82, 28)):
                g.face_y(y + 1.02, x - 1.6, 34.4, 4.2, 2.2, hexc("#FFF4CF"))
            cv.draw(g)
    items.append((60, floodlights))
    run_items(cv, items)


def market_block(cv, frame):
    ground(cv, frame, "paved")
    t = frame
    items = []
    with_lamps(cv, items)
    stalls = [(18, 20, "#D9534F"), (42, 20, "#3E9C8E"), (66, 20, "#F2C94C"), (18, 52, "#5B7FD9"), (42, 52, "#E9875A"), (66, 52, "#8D7BD6")]
    for (x0, y0, col) in stalls:
        def stall(x0=x0, y0=y0, col=col):
            s = OL(k=-0.55)
            s.box(x0, y0, 0, 14, 8, 6, "#B98A5E", top=hexc("#D9B48A"))
            for k in range(4):
                s.box(x0 + 1 + k * 3.2, y0 + 1, 6, 2.6, 2.6, 1.4, ["#D9534F", "#F2C94C", "#6E9B5C", "#E9875A"][(k + x0) % 4], hl=False)
            s.box(x0, y0 - 1, 0, 0.8, 0.8, 13, "#6E4A2E", hl=False)
            s.box(x0 + 13.2, y0 - 1, 0, 0.8, 0.8, 13, "#6E4A2E", hl=False)
            cv.draw(s)
            aw = OL(k=-0.5)
            n = 7
            for i in range(n):
                xa = x0 - 1 + i * 16 / n
                aw.poly([P(xa, y0 - 1, 14), P(xa + 16 / n, y0 - 1, 14), P(xa + 16 / n, y0 + 9, 11), P(xa, y0 + 9, 11)], hexc(col if i % 2 == 0 else "#FFFFFF"))
            cv.draw(aw)
        items.append((x0 + y0 + 6, stall))

    def bunting():
        b = OL(outline=False)
        for i in range(70):
            x = 14 + i
            z = 20 - abs(math.sin(i / 70 * math.pi)) * 4
            b.put(*P(x, 40, z), hexc("#8C8C8C"))
            if i % 4 == 0:
                b.put(*P(x, 40, z - 1), hexc(["#D9534F", "#F2C94C", "#5B7FD9", "#3E9C8E"][(i // 4 + t) % 4]))
                b.put(*P(x + 0.5, 40, z - 2), hexc(["#D9534F", "#F2C94C", "#5B7FD9", "#3E9C8E"][(i // 4 + t) % 4]))
        cv.draw(b)
    items.append((80, bunting))
    for (cx, cy) in ((34, 76), (60, 78)):
        items.append((cx + cy, lambda a=cx, b=cy: _crates(cv, a, b)))
    run_items(cv, items)


def _crates(cv, x, y):
    c = OL(k=-0.55)
    c.box(x, y, 0, 4, 4, 3, "#C99A6B", top=hexc("#D9B48A"))
    c.box(x + 4.4, y, 0, 4, 4, 3, "#C99A6B", top=hexc("#D9B48A"))
    c.box(x + 2, y + 0.5, 3, 4, 3, 3, "#B98A5E", top=hexc("#D9B48A"))
    cv.draw(c)


def garden_block(cv, frame):
    ground(cv, frame)
    t = frame
    items = []
    with_lamps(cv, items)

    def greenhouse():
        g = OL(k=-0.5, emissive=not room.DAY)
        x0, y0, w, d, h = 18, 18, 30, 22, 14
        glass = C("#3A5A66", "#CFE9F2")
        g.box(x0, y0, 0, w, d, h, "#FFFFFF", top=glass, left=glass, right=lit(glass, -0.1))
        cv.draw(g)
        fr = OL(outline=False)
        for i in range(0, w + 1, 5):
            fr.face_y(y0 + d + 0.02, x0 + i - 0.2, 0, 0.4, h, hexc("#FFFFFF"))
        for i in range(0, d + 1, 5):
            fr.face_x(x0 + w + 0.02, y0 + i - 0.2, 0, 0.4, h, hexc("#FFFFFF"))
        fr.poly([P(x0, y0 + d, h), P(x0 + w, y0 + d, h), P(x0 + w, y0 + d / 2, h + 6), P(x0, y0 + d / 2, h + 6)], glass)
        cv.draw(fr)
        pl = OL(outline=False)
        for i in range(12):
            pl.face_y(y0 + d + 0.03, x0 + 2 + i * 2.3, 1, 1.2, 3 + (i % 3), C("#3F6B3A", "#6E9B5C"))
        cv.draw(pl)
    items.append((48, greenhouse))

    def windmill():
        m = OL(k=-0.55)
        cx, cy = 68, 30
        for k in range(10):
            r_ = 4 - k * 0.2
            m.box(cx - r_, cy - r_, k * 3, 2 * r_, 2 * r_, 3, "#F3EEE7", top=hexc("#FFFFFF"), hl=False)
        m.box(cx - 2.6, cy - 2.6, 30, 5.2, 5.2, 3, "#C9605F", hl=False)
        cv.draw(m)
        bl = OL(outline=False)
        for k in range(4):
            ang = k / 4 * math.tau + t * math.pi / 8
            for r_ in range(1, 16):
                for w_ in (-0.6, 0, 0.6):
                    bl.put(*P(cx + math.cos(ang) * r_ - math.sin(ang) * w_, cy + 2.8, 31 + math.sin(ang) * r_ + math.cos(ang) * w_), hexc("#E3D3BC" if r_ > 3 else "#6E4A2E"))
        cv.draw(bl)
    items.append((98, windmill))
    for i in range(4):
        items.append((60 + i * 6, lambda k=i: flowerbed(cv, 18, 50 + k * 7, 28, 4, t + k)))
    for (tx, ty) in ((66, 60), (78, 74)):
        items.append((tx + ty, lambda a=tx, b=ty: town.tree(cv, a, b, 0.8, 0)))
    items.append((150, lambda: bench(cv, 56, 74)))
    run_items(cv, items)


def lake_block(cv, frame):
    ground(cv, frame)
    t = frame
    w = OL(outline=False)
    w.floor = True
    for sy in range(TILE_H):
        for sx in range(TILE_W):
            x, y = inv(sx, sy)
            d = ((x - 48) / 32) ** 2 + ((y - 48) / 30) ** 2
            island = ((x - 54) / 9) ** 2 + ((y - 44) / 8) ** 2 < 1
            if d < 1.06 and 11 <= x < 85 and 11 <= y < 85 and not island:
                c = C("#55576A", "#B9B2A0")
                if d < 1:
                    c = C("#1B3A58", "#4F9ACB")
                    if (sx * 2 + sy + t * 7) % 23 == 0:
                        c = C("#5C8FBF", "#BFE6FA")
                w.put(sx, sy, c)
    cv.draw(w)
    items = []
    with_lamps(cv, items)

    def bridge():
        b = OL(k=-0.5)
        for i in range(12):
            z = math.sin(i / 11 * math.pi) * 3
            b.box(54, 52 + i * 2.2, z, 4, 2.2, 0.8, "#C9605F", top=hexc("#D9734F"), hl=False)
        cv.draw(b)
    items.append((110, bridge))

    def pavilion():
        p = OL(k=-0.55)
        cx, cy = 54, 44
        p.box(cx - 6, cy - 6, 0, 12, 12, 1.4, "#C9C2B6", top=hexc("#E3DCD0"))
        for (dx, dy) in ((-5, -5), (4, -5), (-5, 4), (4, 4)):
            p.box(cx + dx, cy + dy, 1.4, 1, 1, 9, "#B8322A", hl=False)
        for k in range(5):
            r_ = 8 - k * 1.6
            p.box(cx - r_, cy - r_, 10.4 + k * 1.6, 2 * r_, 2 * r_, 1.6, "#2F6B5A" if k % 2 == 0 else "#3E8C76", hl=False)
        p.box(cx - 0.6, cy - 0.6, 18.4, 1.2, 1.2, 3, "#F2C94C", hl=False)
        cv.draw(p)
        if not room.DAY:
            g = OL(outline=False, emissive=True)
            g.box(cx - 0.8, cy + 4.4, 7, 1.6, 0.6, 2, "#FF8A4C", hl=False)
            cv.draw(g)
    items.append((98, pavilion))

    def boats():
        b = OL(k=-0.5)
        for i, (bx, by, col) in enumerate(((28, 40, "#F2C94C"), (40, 66, "#5B7FD9"))):
            o = (0, 0.8)[(t + i) % 2]
            b.box(bx + o, by, 0, 6, 3, 1.4, col, top=lit(hexc(col), 0.2))
            b.box(bx + 2 + o, by + 0.5, 1.4, 2, 2, 1.6, "#FFFFFF", hl=False)
        cv.draw(b)
    items.append((90, boats))
    for (tx, ty) in ((16, 16), (80, 18), (16, 80), (82, 80)):
        items.append((tx + ty, lambda a=tx, b=ty: town.tree(cv, a, b, 0.8, 1 if a > 50 else 0)))
    run_items(cv, items)


def forest_block(cv, frame):
    ground(cv, frame)
    t = frame
    items = []
    with_lamps(cv, items)
    for i in range(14):
        tx = 16 + (h32(i, 11) % 64)
        ty = 16 + (h32(i, 13) % 64)
        if abs(tx - 54) < 12 and abs(ty - 60) < 12:
            continue
        items.append((tx + ty, lambda a=tx, b=ty, k=i: town.tree(cv, a, b, 0.75 + (k % 3) * 0.1, 1 if k % 5 == 0 else 0)))

    def camp():
        c = OL(k=-0.55)
        c.poly([P(46, 56, 0), P(46, 64, 0), P(50, 60, 8)], hexc("#E9875A"))
        c.poly([P(46, 64, 0), P(56, 64, 0), P(54, 60, 8), P(50, 60, 8)], hexc("#D06E42"))
        c.box(58, 64, 0, 4, 4, 0.8, "#6E4A2E", hl=False)
        cv.draw(c)
        f = OL(outline=False, emissive=True)
        for k in range(3):
            f.put(*P(60 + k * 0.5, 66, 1 + k + t % 2), hexc(["#FF8A4C", "#F2C94C", "#FFE7A8"][k]))
        cv.draw(f)
    items.append((120, camp))
    run_items(cv, items)


def lawn_block(cv, frame):
    ground(cv, frame)
    items = []
    with_lamps(cv, items)
    for (tx, ty, k) in ((24, 26, 0), (70, 24, 1), (30, 70, 0), (72, 68, 0)):
        items.append((tx + ty, lambda a=tx, b=ty, kk=k: town.tree(cv, a, b, 0.9, kk)))

    def picnic():
        p = OL(outline=False)
        p.floor = True
        for i in range(8):
            for j in range(6):
                p.flat(42 + i * 1.5, 46 + j * 1.5, 0.05, 1.5, 1.5, hexc("#D9534F" if (i + j) % 2 else "#FFFFFF"))
        cv.draw(p)
        b = OL(k=-0.5)
        b.box(47, 48, 0, 3, 2, 2, "#C99A6B", top=hexc("#D9B48A"))
        cv.draw(b)
    items.append((5, picnic))
    items.append((140, lambda: bench(cv, 50, 70)))
    run_items(cv, items)


FACILITIES = {
    "hall": ("市政厅广场", hall_block),
    "park": ("中央公园", park_block),
    "amusement": ("游乐园", amusement_block),
    "stadium": ("体育场", stadium_block),
    "market": ("集市", market_block),
    "garden": ("植物园", garden_block),
    "lake": ("湖心亭", lake_block),
    "forest": ("营地森林", forest_block),
    "lawn": ("草坪", lawn_block),
}
