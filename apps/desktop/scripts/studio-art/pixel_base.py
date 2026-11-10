"""Hand-rule isometric pixel-art sample: corner room + desk + seated robot + standing robot.

Rules applied (what makes reference-style pixel art read as "real"):
  1. native 1:1 pixels, 2:1 iso lines
  2. one light direction (top-left): top = light, left (+y) face = mid, right (+x) face = dark
  3. per-object selective outline (darker shade of the touching colour, not pure black)
  4. 1px highlight on lit front edges, rim on the front vertical corner
  5. material texture (wood grain, glass streaks, metal sheen, fabric)
  6. emissive accents (screens, LEDs) that tint what is in front of them
  7. contact shadows / ambient occlusion
"""
import os, struct, zlib, math, base64

HERE = os.path.dirname(os.path.abspath(__file__))
W, H = 252, 196
OX, OY = 112, 58


def hexc(s):
    s = s.lstrip("#")
    return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16))


def mix(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


def lit(c, k):
    return mix(c, (255, 255, 255), k) if k >= 0 else mix(c, (0, 0, 0), -k)


def P(x, y, z=0):
    return (OX + 2 * (x - y), OY + (x + y) - z)


def inside(px, py, poly):
    c = False
    n = len(poly)
    for i in range(n):
        x1, y1 = poly[i]
        x2, y2 = poly[(i + 1) % n]
        if (y1 > py) != (y2 > py):
            xi = x1 + (py - y1) * (x2 - x1) / (y2 - y1)
            if px < xi:
                c = not c
    return c


class Layer:
    """One object. Pixels carry colour + face tag; outlined on composite."""

    def __init__(self, outline=True, k=-0.55):
        self.px = {}
        self.tag = {}
        self.outline = outline
        self.k = k

    def put(self, x, y, c, tag=None):
        x, y = int(math.floor(x)), int(math.floor(y))
        if 0 <= x < W and 0 <= y < H:
            self.px[(x, y)] = c
            if tag:
                self.tag[(x, y)] = tag

    def poly(self, pts, colfn, tag=None):
        xs = [p[0] for p in pts]
        ys = [p[1] for p in pts]
        for y in range(int(min(ys)), int(math.ceil(max(ys))) + 1):
            for x in range(int(min(xs)), int(math.ceil(max(xs))) + 1):
                if inside(x + 0.5, y + 0.5, pts):
                    c = colfn(x, y) if callable(colfn) else colfn
                    if c:
                        self.put(x, y, c, tag)

    def box(self, x, y, z, w, d, h, base, top=None, left=None, right=None, hl=True, tex=None):
        base = hexc(base) if isinstance(base, str) else base
        T = top or lit(base, 0.22)
        L = left or base
        R = right or lit(base, -0.28)
        T = hexc(T) if isinstance(T, str) else T
        L = hexc(L) if isinstance(L, str) else L
        R = hexc(R) if isinstance(R, str) else R
        tid = id(self) + len(self.px)
        if h > 0:
            self.poly([P(x + w, y, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x + w, y, z + h)],
                      (lambda X, Y: tex(X, Y, R, "r")) if tex else R, ("r", tid))
            self.poly([P(x, y + d, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x, y + d, z + h)],
                      (lambda X, Y: tex(X, Y, L, "l")) if tex else L, ("l", tid))
        self.poly([P(x, y, z + h), P(x + w, y, z + h), P(x + w, y + d, z + h), P(x, y + d, z + h)],
                  (lambda X, Y: tex(X, Y, T, "t")) if tex else T, ("t", tid))
        if hl and h > 0:
            hc = lit(T, 0.35)
            rim = lit(L, 0.18)
            for (px, py), tg in list(self.tag.items()):
                if tg == ("t", tid):
                    below = self.tag.get((px, py + 1))
                    if below and below[1] == tid and below[0] in ("l", "r"):
                        self.px[(px, py)] = hc
                if tg == ("l", tid):
                    nb = self.tag.get((px + 1, py))
                    if nb == ("r", tid):
                        self.px[(px, py)] = rim

    def face_y(self, yp, x, z, dx, dz, col, tag=None):
        self.poly([P(x, yp, z), P(x + dx, yp, z), P(x + dx, yp, z + dz), P(x, yp, z + dz)], col, tag)

    def face_x(self, xp, y, z, dy, dz, col, tag=None):
        self.poly([P(xp, y, z), P(xp, y + dy, z), P(xp, y + dy, z + dz), P(xp, y, z + dz)], col, tag)

    def flat(self, x, y, z, w, d, col):
        self.poly([P(x, y, z), P(x + w, y, z), P(x + w, y + d, z), P(x, y + d, z)], col)


class Canvas:
    def __init__(self):
        self.px = [[None] * W for _ in range(H)]

    def get(self, x, y):
        if 0 <= x < W and 0 <= y < H:
            return self.px[y][x]
        return None

    def put(self, x, y, c):
        x, y = int(math.floor(x)), int(math.floor(y))
        if 0 <= x < W and 0 <= y < H:
            self.px[y][x] = c

    def blend(self, x, y, c, a):
        x, y = int(math.floor(x)), int(math.floor(y))
        o = self.get(x, y)
        if o:
            self.put(x, y, mix(o, c, a))

    def draw(self, layer):
        if layer.outline:
            out = {}
            for (x, y), c in layer.px.items():
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    q = (x + dx, y + dy)
                    if q not in layer.px and q not in out:
                        out[q] = lit(c, layer.k)
            for (x, y), c in out.items():
                self.put(x, y, c)
        for (x, y), c in layer.px.items():
            self.put(x, y, c)

    def png(self, frames_canvases=None):
        return self.px


def write_png(path, w, h, rows):
    raw = b""
    for row in rows:
        raw += b"\x00" + b"".join(bytes((c[0], c[1], c[2], 255)) if c else b"\x00\x00\x00\x00" for c in row)

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    data += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(path, "wb").write(data)
    return data


def h32(*a):
    v = 2166136261
    for n in a:
        v = ((v ^ (int(n) & 0xFFFFFFFF)) * 16777619) & 0xFFFFFFFF
    return v


# ------------------------------------------------------------------ scene
RX, RY, RH = 64, 52, 54


def scene(frame):
    cv = Canvas()

    # ---------- walls (inverse-mapped so texture follows the plane) ----------
    wall = Layer(outline=False)
    WL, WR = hexc("#E9DFD0"), hexc("#D8CBB8")
    for sy in range(H):
        for sx in range(W):
            # left wall plane x = 0
            y = (OX - sx) / 2
            if 0 <= y <= RY:
                z = OY + y - sy
                if 0 <= z <= RH:
                    c = WL
                    if z < 15:
                        c = lit(hexc("#D9C7AC"), 0)
                        if int(y) % 6 == 0:
                            c = lit(c, -0.08)
                    if 15 <= z < 16.5:
                        c = hexc("#C2A47F")
                    if z < 3:
                        c = hexc("#A88A66")
                    c = lit(c, -0.06 * (1 - z / RH))
                    wall.put(sx, sy, c)
            # right wall plane y = 0
            x = (sx - OX) / 2
            if 0 <= x <= RX:
                z = OY + x - sy
                if 0 <= z <= RH:
                    c = WR
                    if z < 15:
                        c = lit(hexc("#CCB898"), 0)
                        if int(x) % 6 == 0:
                            c = lit(c, -0.08)
                    if 15 <= z < 16.5:
                        c = hexc("#B39470")
                    if z < 3:
                        c = hexc("#9A7D5B")
                    c = lit(c, -0.06 * (1 - z / RH))
                    wall.put(sx, sy, c)
    cv.draw(wall)
    # wall caps
    cap = Layer(outline=False)
    cap.poly([P(-3, -3, RH), P(0, -3, RH), P(0, RY, RH), P(-3, RY, RH)], hexc("#6E5B47"))
    cap.poly([P(-3, -3, RH), P(RX, -3, RH), P(RX, 0, RH), P(0, 0, RH)], hexc("#6E5B47"))
    cap.face_y(RY, -3, 0, 3, RH, hexc("#8C7660"))
    cap.face_x(RX, -3, 0, 3, RH, hexc("#7A654F"))
    cv.draw(cap)

    # ---------- floor: oak planks ----------
    fl = Layer(outline=False)
    for sy in range(H):
        for sx in range(W):
            a = (sx + 0.5 - OX) / 2
            b = sy + 0.5 - OY
            x = (a + b) / 2
            y = (b - a) / 2
            if 0 <= x < RX and 0 <= y < RY:
                plank = int(x // 5)
                off = (h32(plank, 7) % 23)
                seg = int((y + off) // 21)
                tone = (h32(plank, seg) % 1000) / 1000
                c = mix(hexc("#C79A68"), hexc("#D9B283"), tone)
                if (x % 5) < 0.55:
                    c = lit(c, -0.22)
                elif ((y + off) % 21) < 0.6:
                    c = lit(c, -0.16)
                elif (h32(int(x * 3), int(y)) % 9) == 0:
                    c = lit(c, -0.06)
                # ambient occlusion along walls
                ao = max(0, 1 - min(x, y) / 7)
                c = lit(c, -0.18 * ao)
                fl.put(sx, sy, c)
    cv.draw(fl)

    # rug under chair
    rug = Layer(outline=True, k=-0.35)
    def rugtex(X, Y, c, f):
        return lit(c, -0.05) if (X + Y) % 3 == 0 else c
    rug.poly([P(12, 24), P(46, 24), P(46, 44), P(12, 44)], lambda X, Y: rugtex(X, Y, hexc("#5B6E8F"), "t"))
    rug.poly([P(14, 26), P(44, 26), P(44, 42), P(14, 42)], lambda X, Y: rugtex(X, Y, hexc("#7186A8"), "t"))
    cv.draw(rug)

    # ---------- wall decor ----------
    dec = Layer(outline=True, k=-0.5)
    # framed poster on left wall: our squad mascot
    dec.face_x(0, 10, 22, 16, 22, hexc("#3B3530"))
    dec.face_x(0, 11, 23, 14, 20, hexc("#F6F0E7"))
    dec.face_x(0, 13, 30, 10, 10, hexc("#E9875A"))
    dec.face_x(0, 14, 33, 8, 4, hexc("#2B2620"))
    dec.face_x(0, 15, 34, 1, 2, hexc("#FFF8E6"))
    dec.face_x(0, 20, 34, 1, 2, hexc("#FFF8E6"))
    dec.face_x(0, 17, 40, 2, 3, hexc("#E9875A"))
    dec.face_x(0, 13, 25, 10, 2, hexc("#34406B"))
    cv.draw(dec)
    # wall shelf on right wall with books
    shelf = Layer(outline=True)
    shelf.box(40, 0, 30, 18, 5, 2, "#8E6542")
    cv.draw(shelf)
    books = Layer(outline=True, k=-0.5)
    bx = 41
    cols = ["#5B7FD9", "#E9875A", "#3E9C8E", "#E8C766", "#D9719B", "#34406B"]
    for i, hgt in enumerate([9, 11, 8, 10, 7]):
        books.box(bx, 1, 32, 2, 4, hgt, cols[i % len(cols)], hl=True)
        bx += 2
    books.box(bx + 1, 1, 32, 5, 4, 3, "#F3EEE7")
    books.box(bx + 1.5, 1.5, 35, 4, 3, 2, "#C9605F")
    cv.draw(books)

    # floor plant in the back corner
    fp = Layer(outline=True, k=-0.5)
    fp.box(3, 42, 0, 6, 6, 7, "#C4703A", top=hexc("#5A3E26"))
    cv.draw(fp)
    fl2 = Layer(outline=True, k=-0.45)
    for (lx, ly, lz, col) in ((2, 42, 8, "#4F7A45"), (5, 41, 10, "#6E8B5E"), (3, 45, 11, "#5E8A50"), (6, 44, 13, "#7FA06C"),
                              (3, 43, 15, "#6E8B5E"), (5, 43, 18, "#88AD74"), (4, 45, 16, "#4F7A45")):
        fl2.box(lx, ly, lz, 3, 3, 3, col, hl=False)
    cv.draw(fl2)
    # ---------- desk: shadow, legs, glass top ----------
    for sy in range(H):
        for sx in range(W):
            a = (sx + 0.5 - OX) / 2
            b = sy + 0.5 - OY
            x = (a + b) / 2
            y = (b - a) / 2
            # soft contact shadow under desk
            if 13 <= x <= 49 and 7 <= y <= 25:
                d = min(x - 13, 49 - x, y - 7, 25 - y)
                cv.blend(sx, sy, (60, 40, 25), 0.28 * min(1, d / 3))
            # chair shadow
            if (x - 30) ** 2 / 50 + (y - 31) ** 2 / 26 < 1:
                cv.blend(sx, sy, (30, 30, 50), 0.22)
    legs = Layer(outline=True, k=-0.5)
    for (lx, ly) in ((15, 9), (46, 9), (15, 22), (46, 22)):
        legs.box(lx, ly, 0, 2, 2, 19, "#B9C1CC", top=lit(hexc("#B9C1CC"), 0.3))
    legs.box(15.5, 9.5, 8, 31, 1, 1, "#9AA3AE")
    cv.draw(legs)

    top = Layer(outline=True, k=-0.6)
    glass = hexc("#3F4F66")

    def glasstex(X, Y, c, f):
        if f == "t":
            s = (X - 2 * Y) % 23
            if s in (0, 1):
                return lit(c, 0.28)
            if s == 3:
                return lit(c, 0.14)
        return c

    top.box(14, 8, 19, 34, 16, 1, glass, top=hexc("#4A5C77"), left=hexc("#2E3B4E"), right=hexc("#263243"), tex=glasstex)
    cv.draw(top)

    # monitors (two, angled screens face +y)
    t = frame
    for mi, mx in enumerate((18, 33)):
        st = Layer(outline=True, k=-0.5)
        st.box(mx + 5, 11, 20, 3, 3, 1, "#2B2826")
        st.box(mx + 6, 11.5, 21, 1.5, 1.5, 5, "#3A3734")
        cv.draw(st)
        mon = Layer(outline=True, k=-0.55)
        mon.box(mx, 11, 25, 13, 2, 12, "#24211F", top=hexc("#3A3734"), left=hexc("#1C1A18"), right=hexc("#141311"))
        cv.draw(mon)
        scr = Layer(outline=False)
        bgc = hexc("#16243D") if mi == 0 else hexc("#14212F")
        scr.face_y(13, mx + 1, 26, 11, 10, bgc)
        cv.draw(scr)
        # screen content: drawn as pixels on the +y plane
        ui = Layer(outline=False)
        if mi == 0:
            palette = [hexc("#7FC4FF"), hexc("#A9DDD3"), hexc("#F2B45A"), hexc("#D2CAF0"), hexc("#F4C2D6")]
            for ln in range(8):
                row = (ln + t) % 8
                indent = (h32(row, 3) % 3)
                length = 2 + h32(row, 9) % 6
                zz = 34.5 - ln * 1.1
                ui.face_y(13, mx + 2 + indent, zz, length * 0.9, 0.55, palette[h32(row, 1) % 5])
            if t % 2 == 0:
                ui.face_y(13, mx + 2 + 1, 34.5 - 7 * 1.1, 0.8, 0.8, hexc("#FFFFFF"))
        else:
            ui.face_y(13, mx + 2, 33, 9, 2.2, hexc("#2B3A63"))
            ui.face_y(13, mx + 2.5, 33.6, 3, 1, hexc("#4FA36A"))
            for bi in range(5):
                bh = 1.5 + (h32(bi, t // 2) % 5)
                ui.face_y(13, mx + 2.2 + bi * 1.8, 27, 1.1, bh, hexc("#7FC4FF") if bi != 3 else hexc("#F2B45A"))
        cv.draw(ui)
        # screen glow on desk top in front
        for sy in range(H):
            for sx in range(W):
                a = (sx + 0.5 - OX) / 2
                b = sy + 0.5 - OY + 20
                x = (a + b) / 2
                y = (b - a) / 2
                if mx <= x <= mx + 13 and 13 <= y <= 19:
                    cv.blend(sx, sy, (120, 180, 255), 0.12 * (1 - (y - 13) / 6))
        led = Layer(outline=False)
        led.put(*P(mx + 11, 13, 25.5), hexc("#4FE08A") if (t + mi) % 3 else hexc("#1C1A18"))
        cv.draw(led)

    # keyboard + mouse + mug + plant
    kb = Layer(outline=True, k=-0.5)
    kb.box(22, 16, 20, 14, 4, 1, "#2E2B29", top=hexc("#3D3A37"))
    cv.draw(kb)
    keys = Layer(outline=False)
    for kx in range(14):
        for ky in range(3):
            if (kx + ky) % 2 == 0:
                c = hexc("#6B6762")
                if frame % 2 == 0 and (kx * 7 + ky + frame) % 11 == 0:
                    c = hexc("#7FC4FF")
                keys.put(*P(22.6 + kx, 16.6 + ky * 1.1, 21), c)
    cv.draw(keys)
    ms = Layer(outline=True, k=-0.5)
    ms.box(38, 17, 20, 2, 3, 1, "#ECE7E0")
    cv.draw(ms)
    mug = Layer(outline=True, k=-0.5)
    mug.box(42, 15, 20, 3, 3, 4, "#E9875A")
    mug.flat(42.5, 15.5, 24, 2, 2, hexc("#5A3A22"))
    mug.box(45, 16, 21, 1, 1, 2, "#D06E42", hl=False)
    cv.draw(mug)
    if frame % 2 == 0:
        sx, sy = P(43.5, 16.5, 27)
        cv.blend(sx, sy, (240, 240, 245), 0.6)
        cv.blend(sx + 1, sy - 2, (240, 240, 245), 0.45)
    else:
        sx, sy = P(43.5, 16.5, 28)
        cv.blend(sx + 1, sy, (240, 240, 245), 0.6)
        cv.blend(sx, sy - 2, (240, 240, 245), 0.45)
    pot = Layer(outline=True, k=-0.5)
    pot.box(15.5, 10, 20, 3, 3, 3, "#F3EEE7")
    cv.draw(pot)
    leaf = Layer(outline=True, k=-0.45)
    for (lx, ly, lz, col) in ((15, 10, 24, "#4F7A45"), (17, 9, 25, "#6E8B5E"), (16, 12, 26, "#5E8A50"), (16.5, 10.5, 28, "#88AD74")):
        leaf.box(lx, ly, lz, 2, 2, 2, col, hl=False)
    cv.draw(leaf)

    # ---------- chair (base + seat) ----------
    base = Layer(outline=True, k=-0.5)
    cx, cy = 30, 31
    for ang in range(5):
        a = ang / 5 * math.tau + 0.3
        for r in range(0, 7):
            base.put(*P(cx + math.cos(a) * r, cy + math.sin(a) * r, 1), hexc("#2B2826"))
        base.put(*P(cx + math.cos(a) * 7, cy + math.sin(a) * 7, 0), hexc("#111111"))
    base.box(cx - 0.5, cy - 0.5, 1, 1.5, 1.5, 8, "#4A4540", hl=False)
    base.box(cx - 5, cy - 4, 9, 10, 9, 2, "#2E3A5E", top=hexc("#3B4A75"))
    cv.draw(base)

    # ---------- seated robot (back 3/4, facing the monitors) ----------
    typing = frame % 2
    rb = Layer(outline=True, k=-0.6)
    O = hexc("#E9875A")
    rb.box(26, 27, 11, 8, 6, 8, O, tex=lambda X, Y, c, f: lit(c, -0.12) if f == "l" and (Y % 3 == 0) else c)
    # arms reaching to the keyboard
    rb.box(24.5, 21 + typing, 17, 1.5, 7, 1.5, O)
    rb.box(34, 21 + (1 - typing), 17, 1.5, 7, 1.5, O)
    rb.box(24.5, 20 + typing, 17.5, 1.5, 1, 1.5, "#F8C9AE", hl=False)
    rb.box(34, 20 + (1 - typing), 17.5, 1.5, 1, 1.5, "#F8C9AE", hl=False)
    cv.draw(rb)
    head = Layer(outline=True, k=-0.6)
    hb = 1 if frame in (1, 2) else 0
    head.box(26, 26, 19 + hb, 8, 7, 8, O, top=hexc("#F3A57E"))
    # back panel + vents on +y face
    head.face_y(33, 27.5, 21 + hb, 5, 5, hexc("#D06E42"))
    for v in range(3):
        head.face_y(33, 28.2, 22 + v * 1.5 + hb, 3.6, 0.6, hexc("#9C4F2C"))
    # visor wrap visible on the +x side near the front, ear bolt
    head.face_x(34, 26, 22.5 + hb, 2, 3, hexc("#2B2620"))
    head.face_x(34, 26.3, 23.5 + hb, 1, 1, hexc("#7FD8FF"))
    head.face_x(34, 29, 22.5 + hb, 2.5, 2.5, hexc("#B5530F"))
    cv.draw(head)
    # headphones band (accessory)
    hp = Layer(outline=True, k=-0.5)
    hp.box(25.5, 30.5, 27 + hb, 9, 1.5, 1, "#2B2620", hl=False)
    hp.box(24.5, 28, 21 + hb, 1.5, 4, 5, "#F2B45A")
    hp.box(34, 28, 21 + hb, 1.5, 4, 5, "#F2B45A")
    cv.draw(hp)
    ant = Layer(outline=True, k=-0.6)
    ant.box(29.5, 28.5, 27 + hb, 1, 1, 3, "#2B2620", hl=False)
    ant.box(29, 28, 30 + hb, 2, 2, 2, "#F8C9AE")
    cv.draw(ant)

    # chair backrest (in front of the robot, toward viewer)
    back = Layer(outline=True, k=-0.55)
    back.box(25, 34, 11, 10, 2, 10, "#2E3A5E", top=hexc("#3B4A75"), left=hexc("#33416A"))
    back.face_y(36, 29, 12, 2, 8, hexc("#E9875A"))
    cv.draw(back)
    arm = Layer(outline=True, k=-0.55)
    arm.box(23.5, 28, 13, 1.5, 6, 1, "#26324F")
    arm.box(35, 28, 13, 1.5, 6, 1, "#26324F")
    cv.draw(arm)

    # ---------- standing robot (front 3/4, waving) — shows the face ----------
    sx0, sy0 = 50, 34
    step = frame % 2
    q = Layer(outline=True, k=-0.6)
    Pk = hexc("#D9719B")
    # floor shadow
    for yy in range(-3, 4):
        for xx in range(-4, 5):
            if xx * xx / 16 + yy * yy / 9 <= 1:
                cv.blend(*P(sx0 + 4 + xx, sy0 + 3 + yy, 0), (50, 30, 20), 0.25)
    q.box(sx0 + 1, sy0 + 2 - step, 0, 2, 2, 6 + step, "#2B2620", hl=False)
    q.box(sx0 + 5, sy0 + 2 + step, 0, 2, 2, 6 + (1 - step), "#2B2620", hl=False)
    q.box(sx0, sy0, 6, 8, 6, 8, Pk)
    q.face_y(sy0 + 6, sx0 + 2, 8, 4, 4, hexc("#F4C2D6"))
    q.face_y(sy0 + 6, sx0 + 3.5, 9.5, 1, 1, hexc("#4FA36A") if frame % 2 else hexc("#2563B0"))
    # arms: left resting, right waving
    q.box(sx0 - 2, sy0 + 2, 8, 2, 2, 6, Pk)
    wave = [0, 2, 0, 2][frame]
    q.box(sx0 + 8, sy0 + 2, 13, 2, 2, 7 + wave, Pk)
    q.box(sx0 + 8, sy0 + 2, 20 + wave, 2, 2, 2, "#F2B45A")
    cv.draw(q)
    qh = Layer(outline=True, k=-0.6)
    qh.box(sx0 - 1, sy0 - 1, 14, 10, 8, 9, Pk, top=hexc("#E89BB9"))
    # visor on the +y (front) face with eyes
    qh.face_y(sy0 + 7, sx0, 16, 8, 5, hexc("#2B2620"))
    blink = frame == 3
    eye = hexc("#FFF8E6")
    if blink:
        qh.face_y(sy0 + 7, sx0 + 1.5, 18, 2, 0.8, eye)
        qh.face_y(sy0 + 7, sx0 + 4.5, 18, 2, 0.8, eye)
    else:
        qh.face_y(sy0 + 7, sx0 + 1.5, 17, 2, 3, eye)
        qh.face_y(sy0 + 7, sx0 + 4.5, 17, 2, 3, eye)
        qh.face_y(sy0 + 7, sx0 + 1.5, 19, 0.9, 0.9, hexc("#FFFFFF"))
    qh.face_x(sx0 + 9, sy0 + 1, 17, 3, 3, hexc("#B04C77"))
    # crown antenna
    qh.box(sx0 + 2, sy0 + 2, 23, 5, 3, 1, "#2B2620", hl=False)
    qh.box(sx0 + 1.5, sy0 + 1.5, 24, 6, 4, 2, Pk)
    cv.draw(qh)

    # trash bin with crumpled paper
    bin_ = Layer(outline=True, k=-0.5)
    bin_.box(6, 30, 0, 5, 5, 7, "#C9D3E0", top=hexc("#8496AD"))
    bin_.box(7, 31, 7, 2, 2, 1, "#FFFFFF", hl=False)
    bin_.box(8.5, 32, 7, 1.5, 1.5, 1, "#F3EEE7", hl=False)
    cv.draw(bin_)
    return cv


def main():
    frames = [scene(f) for f in range(4)]
    sheet = []
    for y in range(H):
        row = []
        for cv in frames:
            row.extend(cv.px[y])
        sheet.append(row)
    write_png(os.path.join(HERE, "sample_sheet.png"), W * 4, H, sheet)
    write_png(os.path.join(HERE, "sample_f0.png"), W, H, frames[0].px)
    # upscaled preview (x3) for viewing
    S = 3
    big = []
    for y in range(H):
        r = []
        for c in frames[0].px[y]:
            r.extend([c or (246, 239, 228)] * S)
        for _ in range(S):
            big.append(r)
    write_png(os.path.join(HERE, "sample_x3.png"), W * S, H * S, big)
    S2 = 6
    crop = []
    for y in range(40, 150):
        r = []
        for c in frames[0].px[y][60:200]:
            r.extend([c or (246, 239, 228)] * S2)
        for _ in range(S2):
            crop.append(r)
    write_png(os.path.join(HERE, "sample_crop.png"), 140 * S2, 110 * S2, crop)
    print("ok")


if __name__ == "__main__":
    main()
