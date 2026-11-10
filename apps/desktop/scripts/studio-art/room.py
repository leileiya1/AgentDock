"""Full studio room in the pixel-sample style (day / night), bigger floor for future expansion."""
import os, sys, math, json
import pixel_base as ps

HERE = os.path.dirname(os.path.abspath(__file__))
RX, RY, RH = 128, 104, 58
# the room sits like a diorama in a garden; the padding is outdoor scenery that fills the view
PADX, PADY = 96, 64
ps.W, ps.H = 2 * (RX + RY) + 24 + 2 * PADX, RH + RX + RY + 26 + 2 * PADY
ps.OX, ps.OY = 2 * RY + 14 + PADX, RH + 10 + PADY
W, H = ps.W, ps.H
P, Layer, hexc, lit, mix, h32 = ps.P, ps.Layer, ps.hexc, ps.lit, ps.mix, ps.h32
BAYER = [[0, 2], [3, 1]]
DAY = "night" not in sys.argv
VARIANT = next((a for a in sys.argv if a in ("sofa", "away", "desk")), "sofa")
# export.py sets these: draw no robots / light every desk screen
NO_ROBOTS = False
LIT_DESKS = None


def C(night, day):
    return hexc(day if DAY else night)


class Canvas(ps.Canvas):
    def __init__(self):
        super().__init__()
        self.emit = set()
        self.floor = set()

    current = None

    def put(self, x, y, c):
        super().put(x, y, c)
        if self.current is not None:
            if not hasattr(self, "owner"):
                self.owner = {}
            self.owner[(int(math.floor(x)), int(math.floor(y)))] = self.current

    def draw(self, layer):
        super().draw(layer)
        if getattr(layer, "floor", False):
            self.floor.update(layer.px.keys())
        else:
            for q in layer.px:
                self.floor.discard(q)
            if layer.outline:
                for (x, y) in layer.px:
                    for d in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                        self.floor.discard((x + d[0], y + d[1]))
        if getattr(layer, "emissive", False):
            self.emit.update(layer.px.keys())


class OL(Layer):
    """Layer with a world offset so furniture/robots can be placed anywhere."""

    def __init__(self, ox=0, oy=0, outline=True, k=-0.58, emissive=False):
        super().__init__(outline=outline, k=k)
        self.ox, self.oy, self.emissive = ox, oy, emissive

    def Q(self, x, y, z=0):
        return P(x + self.ox, y + self.oy, z)

    def box(self, x, y, z, *a, **kw):
        return super().box(x + self.ox, y + self.oy, z, *a, **kw)

    def face_x(self, xp, y, *a, **kw):
        return super().face_x(xp + self.ox, y + self.oy, *a, **kw)

    def face_y(self, yp, x, *a, **kw):
        return super().face_y(yp + self.oy, x + self.ox, *a, **kw)

    def flat(self, x, y, *a, **kw):
        return super().flat(x + self.ox, y + self.oy, *a, **kw)

    def dot(self, x, y, z, c):
        self.put(*self.Q(x, y, z), c)


def inv(sx, sy):
    a = (sx + 0.5 - ps.OX) / 2
    b = sy + 0.5 - ps.OY
    return (a + b) / 2, (b - a) / 2


FAM = {
    "claude": ("#E9875A", "#F8C9AE", "single"), "codex": ("#3E9C8E", "#A9DDD3", "twin"),
    "qoder": ("#D9719B", "#F4C2D6", "crown"), "gemini": ("#5B7FD9", "#BFD0F5", "single"),
    "qwen": ("#8D7BD6", "#D2CAF0", "crown"), "grok": ("#6E8B5E", "#C6D6B8", "bent"),
    "deepseek": ("#4A90B8", "#BFDDED", "twin"), "kimi": ("#C2A14A", "#EDDDA0", "bent"),
    "minimax": ("#C9605F", "#F0C0BF", "helmet"), "runner": ("#8496AD", "#C9D3E0", "helmet"),
    "neutral": ("#9A9184", "#D8D1C5", "helmet"),
}
PAL = [hexc("#7A5CFF"), hexc("#FF5CC8"), hexc("#5CD6FF"), hexc("#7AFFB0")]


# =============================================================== builders
def antenna(cv, ox, oy, x, y, z, kind, col, frame):
    """x,y = head top-left corner, head width 10, depth 8.5 (front robots)."""
    a = OL(ox, oy, k=-0.6)
    c = hexc(col)
    if kind == "crown":
        a.box(x + 2.5, y + 2, z, 5, 3, 1, "#1E1C1A", hl=False)
        a.box(x + 1.8, y + 1.4, z + 1, 6.4, 4.2, 2.2, col, top=lit(c, 0.3))
    elif kind == "single":
        a.box(x + 4.6, y + 3.6, z, 0.9, 0.9, 3, "#1E1C1A", hl=False)
        a.box(x + 4.1, y + 3.1, z + 3, 1.9, 1.9, 1.9, col, top=lit(c, 0.35))
    elif kind == "twin":
        for dx in (1.5, 7.5):
            a.box(x + dx, y + 3.6, z, 0.9, 0.9, 3.4, "#1E1C1A", hl=False)
            a.box(x + dx - 0.4, y + 3.2, z + 3.4, 1.7, 1.7, 1.5, col)
    elif kind == "bent":
        a.box(x + 6.5, y + 3.6, z, 0.9, 0.9, 2.6, "#1E1C1A", hl=False)
        a.box(x + 7.3, y + 3.6, z + 2.6, 2, 0.9, 0.9, "#1E1C1A", hl=False)
        a.box(x + 8.8, y + 3.2, z + 3.2, 1.7, 1.7, 1.7, col)
    elif kind == "helmet":
        a.box(x + 1, y + 1, z, 8, 6.5, 1.4, lit(c, 0.25), hl=False)
    cv.draw(a)


def _profile_acc(cv, ox, oy, head, facing, acc, frame):
    if any(a.startswith("p:") for a in acc):
        import robot_acc
        robot_acc.draw(cv, ox, oy, head, facing, acc, frame)


def robot_front(cv, x, y, fam, pose="idle", frame=0, z0=0, acc=(), back=False):
    """Standing (or seated when z0>0 and pose != walk) robot facing the viewer (+y)."""
    base, light, ant = FAM[fam]
    B, Lc = hexc(base), hexc(light)
    seated = z0 > 0 or pose == "sleep"
    step = frame % 2 if pose in ("walk", "wave", "hand", "work") else 0
    if not seated:
        for yy in range(-4, 5):
            for xx in range(-5, 6):
                if xx * xx / 25 + yy * yy / 16 <= 1:
                    cv.blend(*P(x + 4 + xx, y + 3 + yy, 0), (12, 10, 16), 0.28)
    q = OL(x, y, k=-0.62)
    zb = z0
    if not seated:
        q.box(1.5, 2 - step * (pose == "walk"), 0, 2, 2.4, 6.5, "#2B2620", top=hexc("#3A3530"), hl=False)
        q.box(5, 2 + step * (pose == "walk"), 0, 2, 2.4, 6.5, "#2B2620", top=hexc("#3A3530"), hl=False)
        q.box(1.2, 3.2, 0, 2.6, 1.6, 1.4, base, hl=False)
        q.box(4.7, 3.2, 0, 2.6, 1.6, 1.4, base, hl=False)
        zb = 6.5
    if pose == "sleep":
        zb = z0
    q.box(0, 0, zb, 8.5, 6, 8, base, top=lit(B, 0.3))
    if back:
        q.face_y(6, 2, zb + 2, 4.5, 4.4, lit(B, -0.15))
        q.face_y(6.02, 2.8, zb + 3.5, 3, 0.6, lit(B, -0.4))
        q.face_y(6.02, 2.8, zb + 5, 3, 0.6, lit(B, -0.4))
    else:
        q.face_y(6, 2, zb + 2, 4.5, 4.4, Lc)
        q.face_y(6.02, 3.6, zb + 3.5, 1.4, 1.2, hexc("#4FE08A") if pose in ("rest", "sleep") or frame % 2 else hexc("#3B82F6"))
    if "scarf" in acc:
        q.box(-0.3, -0.3, zb + 7, 9.1, 6.6, 1.2, "#D9534F", hl=False)
    # arms
    if pose in ("wave", "hand"):
        up = [0, 2, 0, 2][frame] if pose == "wave" else [0, 1, 0, 1][frame]
        q.box(-2, 2, zb + 2, 2, 2.4, 6, base)
        q.box(8.5, 2, zb + 6.5, 2, 2.4, 6.5 + up, base)
        q.box(8.5, 2, zb + 13 + up, 2, 2.4, 2, light, top=lit(Lc, 0.4))
    elif pose == "work":
        q.box(-2, 2, zb + 2, 2, 2.4, 6, base)
        q.box(8.5, 1 - step, zb + 5, 2, 2.4, 6, base)
        q.box(8.5, -1 - step, zb + 9, 2, 2.4, 2, light)
    elif pose == "cup":
        q.box(-2, 2, zb + 2, 2, 2.4, 6, base)
        q.box(8.5, 3, zb + 3, 2, 3, 4, base)
        q.box(8.6, 5.6, zb + 5, 2.4, 2.4, 3, "#F3EEE7", top=hexc("#FFFFFF"))
        q.flat(8.9, 5.9, zb + 8, 1.8, 1.8, hexc("#7A4A26"))
    else:
        q.box(-2, 2, zb + 1, 2, 2.4, 6, base)
        q.box(8.5, 2, zb + 1, 2, 2.4, 6, base)
    cv.draw(q)
    hz = zb + 8
    h = OL(x, y, k=-0.62)
    h.box(-1, -1, hz, 10.5, 8.5, 9, base, top=lit(B, 0.32))
    if back:
        h.face_y(7.5, 1.4, hz + 1.6, 6, 5.4, lit(B, -0.15))
        for v in range(3):
            h.face_y(7.52, 2.2, hz + 2.6 + v * 1.5, 4.4, 0.6, lit(B, -0.4))
        h.face_x(9.5, 4.5, hz + 2.5, 2.6, 3.4, hexc("#1E1C1A"))
    else:
        h.face_y(7.5, 0.2, hz + 1.9, 8.1, 5.2, hexc("#1E1C1A"))
        h.face_x(9.5, 1, hz + 2.5, 3, 3.4, lit(B, -0.3))
    cv.draw(h)
    if back:
        antenna(cv, x, y, -1, -1, hz + 9, ant, base, frame)
        _profile_acc(cv, x, y, (-1, -1, hz, 10.5, 8.5, 9), "-y", acc, frame)
        return hz + 12
    e = OL(x, y, outline=False, emissive=True)
    eye = hexc("#E9FBFF")
    closed = pose in ("rest", "sleep") or frame == 3
    if fam == "codex" and not closed:
        e.face_y(7.52, 1 + (frame % 3) * 2, hz + 3.8, 3, 1.2, hexc("#BFFFF4"))
    elif closed:
        e.face_y(7.52, 1.6, hz + 3.9, 2.2, 0.8, eye)
        e.face_y(7.52, 4.8, hz + 3.9, 2.2, 0.8, eye)
    else:
        e.face_y(7.52, 1.6, hz + 2.9, 2.2, 3.2, eye)
        e.face_y(7.52, 4.8, hz + 2.9, 2.2, 3.2, eye)
        e.face_y(7.54, 1.6, hz + 5.1, 0.9, 1, hexc("#FFFFFF"))
        e.face_y(7.54, 4.8, hz + 5.1, 0.9, 1, hexc("#FFFFFF"))
    cv.draw(e)
    if "glasses" in acc:
        g = OL(x, y, outline=False)
        g.face_y(7.6, 0.6, hz + 5.6, 7.4, 0.5, hexc("#2B2620"))
        cv.draw(g)
    antenna(cv, x, y, -1, -1, hz + 9, ant, base, frame)
    gl = OL(x, y, outline=False)
    gl.dot(0, -0.5, hz + 9, hexc("#FFFFFF"))
    cv.draw(gl)
    _profile_acc(cv, x, y, (-1, -1, hz, 10.5, 8.5, 9), "+y", acc, frame)
    return hz + 12


def robot_back(cv, cx, cy, fam, frame, acc=(), typing=True):
    """Seated, facing -y (toward monitors). cx,cy = chair centre."""
    base, light, ant = FAM[fam]
    O = hexc(base)
    ox, oy = cx - 34, cy - 31
    tp = frame % 2 if typing else 0
    hb = 1 if frame in (1, 2) else 0
    rb = OL(ox, oy, k=-0.62)
    rb.box(30, 27, 11, 8, 6, 8, base, tex=lambda X, Y, c, f: lit(c, -0.1) if f == "l" and (X + Y) % 4 == 0 else c)
    rb.box(28.4, 28, 16, 2, 3, 2.4, lit(O, -0.12))
    rb.box(38, 28, 16, 2, 3, 2.4, lit(O, -0.12))
    rb.box(28.6, 22 + tp, 18, 1.6, 6.5, 1.6, base)
    rb.box(38.2, 22 + (1 - tp), 18, 1.6, 6.5, 1.6, base)
    rb.box(28.4, 21 + tp, 18.4, 2, 1.4, 1.6, light, hl=False)
    rb.box(38, 21 + (1 - tp), 18.4, 2, 1.4, 1.6, light, hl=False)
    if "scarf" in acc:
        rb.box(29.6, 26.6, 18.6, 8.8, 6.8, 1.2, "#D9534F", hl=False)
    cv.draw(rb)
    hd = OL(ox, oy, k=-0.62)
    hd.box(29.5, 25.5, 19 + hb, 9, 8, 8.5, base, top=lit(O, 0.25))
    hd.face_y(33.5, 31.2, 21 + hb, 5.6, 5, lit(O, -0.15))
    for v in range(3):
        hd.face_y(33.52, 32, 22 + v * 1.5 + hb, 4, 0.6, lit(O, -0.4))
    hd.face_x(38.5, 25.6, 22.4 + hb, 2.2, 3.4, hexc("#1E1C1A"))
    hd.face_x(38.52, 26, 23.4 + hb, 1, 1.2, hexc("#7FD8FF"))
    cv.draw(hd)
    if "phones" in acc:
        hp = OL(ox, oy, k=-0.55)
        hp.box(29, 30.2, 27.5 + hb, 10, 1.4, 1, "#1E1C1A", hl=False)
        hp.box(28, 27.5, 21 + hb, 1.6, 4.4, 5, "#F2B45A", top=hexc("#FFD48A"))
        hp.box(38.5, 27.5, 21 + hb, 1.6, 4.4, 5, "#F2B45A", top=hexc("#FFD48A"))
        cv.draw(hp)
    a = OL(ox, oy, k=-0.6)
    if ant in ("single", "crown", "bent"):
        a.box(33.6, 29, 27.5 + hb, 0.9, 0.9, 3, "#1E1C1A", hl=False)
        cv.draw(a)
        tip = OL(ox, oy, k=-0.4, emissive=not DAY)
        tip.box(33.2, 28.6, 30.5 + hb, 1.8, 1.8, 1.8, lit(O, 0.3 if frame % 2 else 0.1), hl=False)
        cv.draw(tip)
    elif ant == "twin":
        for dx in (31, 36):
            a.box(dx, 29, 27.5 + hb, 0.9, 0.9, 3, "#1E1C1A", hl=False)
            a.box(dx - 0.4, 28.6, 30.5 + hb, 1.7, 1.7, 1.4, base)
        cv.draw(a)
    _profile_acc(cv, ox, oy, (29.5, 25.5, 19 + hb, 9, 8, 8.5), "-y", acc, frame)


def robot_side(cv, sx, sy, fam, frame, z=7, acc=(), mode="rest"):
    """Seated on the sofa facing -x (toward the TV on the left wall)."""
    base, light, ant = FAM[fam]
    B = hexc(base)
    r = OL(sx, sy, k=-0.62)
    r.box(-3, -3, z, 6, 6.5, 7, base)
    r.box(-6, -3.4, z + 4, 4, 1.6, 1.6, base)
    r.box(-6, 3.4, z + 4, 4, 1.6, 1.6, base)
    if "pad" in acc:
        r.box(-7.4, -1.6, z + 5 + frame % 2 * 0.5, 2, 4.8, 1, "#2A2C36", hl=False)
    cv.draw(r)
    hd = OL(sx, sy, k=-0.62)
    hd.box(-4.5, -4.5, z + 7, 8.5, 9.5, 8.5, base, top=lit(B, 0.28))
    hd.face_x(4, -2.6, z + 9, 5.6, 5, lit(B, -0.15))
    for v in range(3):
        hd.face_x(4.02, -1.8, z + 10 + v * 1.5, 4, 0.6, lit(B, -0.4))
    hd.face_y(5, -4, z + 10, 2.4, 3.4, hexc("#1E1C1A"))
    eye_on = mode != "sleep"
    hd.face_y(5.02, -3.6, z + 11.2, 1.1, 1.2 if eye_on else 0.5, hexc("#E9FBFF"))
    cv.draw(hd)
    if "flower" in acc:
        f = OL(sx, sy, k=-0.5)
        f.box(-1, 0, z + 15.5, 0.9, 0.9, 2, "#4F7A45", hl=False)
        f.box(-2, -1, z + 17.5, 2.8, 2.8, 1.2, "#E66A9A", top=hexc("#F7A8C6"))
        f.box(-1, 0, z + 18.7, 0.9, 0.9, 0.5, "#F2B45A", hl=False)
        cv.draw(f)
    else:
        a = OL(sx, sy, k=-0.6)
        a.box(-0.5, 0, z + 15.5, 0.9, 0.9, 2.6, "#1E1C1A", hl=False)
        a.box(-1, -0.5, z + 18, 1.8, 1.8, 1.6, base)
        cv.draw(a)
    _profile_acc(cv, sx, sy, (-4.5, -4.5, z + 7, 8.5, 9.5, 8.5), "-x", acc, frame)


def human_front(cv, x, y, z, frame):
    h = OL(x, y, k=-0.6)
    h.box(0.5, 0.5, z, 7, 5, 7, "#F2B45A", top=hexc("#FFD48A"))
    h.face_y(5.5, 3, z + 3, 2, 4, hexc("#FFFFFF"))
    h.box(-1.2, 1.5, z + 1, 1.8, 2.4, 5.5, "#F2B45A")
    h.box(8, 1.5, z + 1, 1.8, 2.4, 5.5, "#F2B45A")
    cv.draw(h)
    hd = OL(x, y, k=-0.6)
    hd.box(0.5, 0, z + 7, 7, 6, 7, "#F5CFA8", top=hexc("#6B4A33"))
    hd.box(0.2, -0.3, z + 12, 7.6, 6.6, 2.4, "#6B4A33", top=hexc("#8A6448"))
    hd.face_y(6, 0.5, z + 11, 7, 1.4, hexc("#6B4A33"))
    hd.face_y(6.02, 2, z + 9.2, 1, 1.2 if frame != 3 else 0.4, hexc("#2B2620"))
    hd.face_y(6.02, 5, z + 9.2, 1, 1.2 if frame != 3 else 0.4, hexc("#2B2620"))
    hd.face_y(6.02, 3.3, z + 7.8, 1.6, 0.6, hexc("#D98A6A"))
    cv.draw(hd)


def chair(cv, cx, cy, part):
    ox, oy = cx - 34, cy - 31
    if part == "seat":
        b = OL(ox, oy, k=-0.5)
        for ang in range(5):
            a = ang / 5 * math.tau + 0.3
            for r in range(0, 8):
                b.dot(34 + math.cos(a) * r, 31 + math.sin(a) * r, 1, hexc("#22242C"))
            b.dot(34 + math.cos(a) * 8, 31 + math.sin(a) * 8, 0, hexc("#0E0F12"))
        b.box(33.4, 30.4, 1, 1.4, 1.4, 8, "#5A5F6E", hl=False)
        b.box(28.5, 26.5, 9, 11, 9, 2.4, "#23293F", top=hexc("#2F3754"))
        b.flat(30, 27.5, 11.41, 8, 7, hexc("#363F60"))
        cv.draw(b)
    else:
        bk = OL(ox, oy, k=-0.6)
        bk.box(28.5, 34.2, 11, 11, 2.2, 11, "#23293F", top=hexc("#2F3754"), left=hexc("#2A3150"))
        bk.face_y(36.4, 32.5, 12, 3, 9.5, hexc("#E9875A"))
        cv.draw(bk)
        ar = OL(ox, oy, k=-0.55)
        ar.box(27, 28, 14, 1.6, 6, 1.2, "#1B2033", top=hexc("#2A3150"))
        ar.box(39.4, 28, 14, 1.6, 6, 1.2, "#1B2033", top=hexc("#2A3150"))
        cv.draw(ar)


def desk(cv, x0, y0, frame, lit_=True, lamp=False, seed=0):
    """Same geometry as the sample desk (x 14..54, y 7..25) moved to x0,y0."""
    ox, oy = x0 - 14, y0 - 7
    t = frame
    fr = OL(ox, oy, k=-0.6)
    legc = "#C9CED6" if DAY else "#2F333E"
    for (lx, ly) in ((16, 9), (50, 9), (16, 22), (50, 22)):
        fr.box(lx, ly, 0, 2, 2, 19, legc, top=lit(hexc(legc), 0.2))
    fr.box(16.5, 9.5, 17, 34, 1, 1.5, legc)
    fr.box(16.5, 22.5, 17, 34, 1, 1.5, legc)
    cv.draw(fr)

    def gtex(X, Y, c, f):
        if f == "t":
            s = (X - 2 * Y) % 29
            if s in (0, 1, 2):
                return lit(c, 0.18)
        return c

    tp = OL(ox, oy, k=-0.62)
    tp.box(14, 7, 19, 40, 18, 1, "#2C3E5C", top=C("#35507A", "#F4EEE6"), left=C("#4D6E9E", "#E2D9CC"), right=C("#3A5680", "#CFC5B6"), tex=gtex)
    cv.draw(tp)
    for mi, mx in enumerate((19.5, 35)):
        st = OL(ox, oy, k=-0.5)
        st.box(mx + 5.5, 10, 20, 4, 3, 0.8, "#2F333E")
        st.box(mx + 6.8, 10.6, 20.8, 1.4, 1.2, 4.6, "#3A3F4C")
        cv.draw(st)
        mon = OL(ox, oy, k=-0.6)
        mon.box(mx, 10.5, 25, 15, 1.6, 12.5, "#17181D", top=hexc("#2A2C34"), left=hexc("#101115"), right=hexc("#0C0D10"))
        cv.draw(mon)
        scr = OL(ox, oy, outline=False, emissive=lit_)
        if not lit_:
            scr.face_y(12.1, mx + 0.8, 25.8, 13.4, 10.9, hexc("#26282E"))
            scr.face_y(12.12, mx + 9, 33, 3, 2.6, hexc("#30333B"))
        elif mi == 0:
            scr.face_y(12.1, mx + 0.8, 25.8, 13.4, 10.9, hexc("#1A2238"))
            scr.face_y(12.12, mx + 0.8, 35.6, 13.4, 1.1, hexc("#2B3352"))
            scr.face_y(12.13, mx + 0.8, 25.8, 2.6, 9.8, hexc("#151C2F"))
            pal2 = [hexc("#7FC4FF"), hexc("#C792EA"), hexc("#F2B45A"), hexc("#A9DDD3"), hexc("#FF8BA7"), hexc("#E8EEFF")]
            for ln in range(8):
                row = ln + t + seed
                xs = mx + 3.8 + (h32(row, 3) % 3) * 0.8
                for sg in range(1 + h32(row, 5) % 3):
                    w_ = 1 + h32(row, sg, 9) % 4
                    scr.face_y(12.14, xs, 34.4 - ln * 1.12, w_ * 0.7, 0.5, pal2[h32(row, sg) % 6])
                    xs += w_ * 0.7 + 0.6
                    if xs > mx + 13.6:
                        break
        else:
            scr.face_y(12.1, mx + 0.8, 25.8, 13.4, 10.9, hexc("#121A2A"))
            scr.face_y(12.12, mx + 1.4, 33.2, 5.6, 2.8, hexc("#1F2B45"))
            scr.face_y(12.13, mx + 1.8, 34.4, 2.4, 0.8, hexc("#6BE08A"))
            for bi in range(7):
                bh = 1.2 + (h32(bi, t // 2 + 1 + seed) % 6) * 0.9
                scr.face_y(12.13, mx + 1.6 + bi * 1.75, 26.6, 1, bh, hexc("#7FC4FF") if bi != 4 else hexc("#FF8BA7"))
        cv.draw(scr)
    # keyboard, mouse, mug
    kb = OL(ox, oy, k=-0.6)
    kb.box(23, 16, 20.3, 15, 4.5, 1.2, "#2A2C36", top=hexc("#3A3D4A"))
    cv.draw(kb)
    keys = OL(ox, oy, outline=False)
    for kx in range(15):
        for ky in range(4):
            if (kx + ky) % 2 == 0:
                c = hexc("#6A6E7C")
                if lit_ and (kx * 7 + ky * 3 + t * 5 + seed) % 13 == 0:
                    c = hexc("#9ED0FF")
                keys.dot(23.5 + kx, 16.5 + ky, 21.5, c)
    cv.draw(keys)
    kl = OL(ox, oy, outline=False, emissive=lit_ and not DAY)
    for kx in range(15):
        kl.dot(23 + kx, 20.5, 20.4, PAL[(kx // 4 + t) % 4] if lit_ else hexc("#3A3D4A"))
    cv.draw(kl)
    ms = OL(ox, oy, k=-0.5)
    ms.box(40.5, 17, 20.3, 2, 3, 1.2, "#2A2C36", top=hexc("#4A4E5C"))
    cv.draw(ms)
    if lit_:
        mug = OL(ox, oy, k=-0.55)
        mug.box(45.5, 17.5, 20, 3, 3, 4, "#F3EEE7", top=hexc("#FFFFFF"))
        mug.flat(46, 18, 24, 2, 2, hexc("#4A2E1A"))
        mug.face_y(20.5, 46, 21, 2, 1.4, hexc("#E9875A"))
        cv.draw(mug)
    if lamp:
        lm = OL(ox, oy, k=-0.6)
        lm.box(15.5, 9, 20, 4, 4, 1, "#E9875A")
        for i in range(6):
            p_ = lm.Q(17.5, 11, 21 + i * 1.5)
            lm.put(*p_, hexc("#3A3F4C"))
            lm.put(p_[0] + 1, p_[1], hexc("#3A3F4C"))
        lm.box(17, 14, 28.5, 3.6, 3.6, 3, "#E9875A", top=hexc("#F5A57A"))
        cv.draw(lm)
        bulb = OL(ox, oy, outline=False, emissive=not DAY)
        bulb.face_y(17.6, 17.6, 28.6, 2.4, 1, hexc("#FFF2C4"))
        cv.draw(bulb)


def plant(cv, x, y, s=1.0, pot="#C4703A"):
    p = OL(x, y, k=-0.5)
    p.box(0, 0, 0, 7 * s, 7 * s, 9 * s, pot, top=hexc("#4E3622"))
    cv.draw(p)
    lv = OL(x, y, k=-0.5)
    for (lx, ly, lz, col) in ((-1, 1, 10, "#3F6B3A"), (3, -1, 12, "#5E8A50"), (0, 4, 13, "#4F7A45"), (4, 3, 15, "#6E9B5C"),
                              (1, 0, 17, "#5E8A50"), (3, 2, 20, "#7FA86B"), (0, 2, 22, "#4F7A45"), (2, 1, 25, "#88B474"), (4, 0, 23, "#5E8A50")):
        lv.box(lx * s, ly * s, lz * s, 3 * s, 3 * s, 2.5 * s, col, hl=False)
    cv.draw(lv)


# =============================================================== scene
def scene(frame):
    cv = Canvas()
    t = frame
    bubbles = []

    def bub(x, y, z, text, code=None, tone="run", dx=0):
        sx, sy = P(x, y, z)
        bubbles.append({"x": sx, "y": sy, "text": text, "code": code, "tone": tone, "dx": dx})


    # ---------------- outdoors: the room is a diorama in a garden ----------------
    def outside_dist(x, y):
        return max(-x, -y, x - RX, y - RY)

    POND = (RX + 34, 64, 15, 10)
    PLOT = (RX + 10, RX + 40, 8, 40)  # x0, x1, y0, y1 — the future second room

    ground = OL(outline=False)
    for sy in range(H):
        for sx in range(W):
            x, y = inv(sx, sy)
            if 0 <= x <= RX and 0 <= y <= RY:
                continue
            n = h32(int(x * 1.4), int(y * 1.4))
            c = C("#2E4836", "#8CBF6C")
            if n % 7 == 0:
                c = lit(c, 0.09)
            elif n % 11 == 0:
                c = lit(c, -0.1)
            elif n % 13 == 0:
                c = C("#3B5A44", "#A3CF7E")
            fl_n = h32(sx, sy, 17) % 211
            if fl_n == 0:
                c = hexc("#F2C94C") if DAY else hexc("#8A8A5A")
            elif fl_n == 1:
                c = hexc("#FFFFFF") if DAY else hexc("#7A8A9A")
            elif fl_n == 2:
                c = hexc("#E66A9A") if DAY else hexc("#7A4A60")
            elif h32(sx, sy // 2, 5) % 41 == 0:
                c = lit(c, -0.2)
            d = outside_dist(x, y)
            # paved apron around the foundation
            if 0 < d <= 5:
                c = C("#4A4C58", "#D8D0BF")
                if int(x) % 6 == 0 or int(y) % 6 == 0:
                    c = lit(c, -0.12)
            # stone path from the lounge corner out to the front
            if y > RY + 5 and 14 <= x <= 26 + (y - RY) * 0.15:
                c = C("#55576A", "#CBBFA8")
                if (int(x) // 3 + int(y) // 3) % 2 == 0:
                    c = lit(c, -0.07)
                if int(x) % 3 == 0 or int(y) % 3 == 0:
                    c = lit(c, -0.16)
            # flower bed along the front edge
            if RY + 5 < y <= RY + 9 and 34 <= x <= 120:
                c = C("#3A2C22", "#7A5536")
                f = h32(int(x * 2), int(y * 2)) % 9
                if f == 0:
                    c = hexc("#E66A9A")
                elif f == 1:
                    c = hexc("#F2C94C")
                elif f == 2:
                    c = hexc("#FFFFFF") if DAY else hexc("#C9C2D6")
                elif f in (3, 4):
                    c = C("#3F6B3A", "#5E8A50")
            # construction plot for the next room
            if PLOT[0] <= x <= PLOT[1] and PLOT[2] <= y <= PLOT[3]:
                c = C("#5A4636", "#B08A62")
                if h32(int(x * 2), int(y * 2)) % 5 == 0:
                    c = lit(c, -0.12)
                if int(x - PLOT[0]) % 8 == 0 or int(y - PLOT[2]) % 8 == 0:
                    c = C("#6B6F7A", "#C9CED6")
            # pond
            pd = ((x - POND[0]) / POND[2]) ** 2 + ((y - POND[1]) / POND[3]) ** 2
            if pd < 1.12:
                c = C("#55576A", "#B9B2A0")
            if pd < 1:
                c = C("#1F3E5C", "#5FA8D6")
                if (sx + sy * 2 + t * 3) % 23 == 0:
                    c = C("#5C8FBF", "#BFE6FA")
                if pd < 0.5 and (sx * 3 + sy + t * 5) % 31 == 0:
                    c = C("#5C8FBF", "#E6F6FF")
            ground.put(sx, sy, c)
    cv.draw(ground)
    # foundation slab: the room floor is raised a little above the garden
    slab = OL(outline=False)
    slab.box(-4, -4, -6, RX + 4, RY + 4, 6, "#8C7660", top=C("#3A3D48", "#B5A58E"), left=C("#3A3640", "#A08A70"), right=C("#2E2B34", "#8C7660"), hl=False)
    cv.draw(slab)

    def tree(x, y, s=1.0, kind=0):
        for yy in range(-5, 6):
            for xx in range(-7, 8):
                if xx * xx / 49 + yy * yy / 25 <= 1:
                    cv.blend(*P(x + xx * s, y + yy * s, 0), (10, 20, 10), 0.22)
        tr = OL(k=-0.55)
        tr.box(x - 1 * s, y - 1 * s, 0, 2 * s, 2 * s, 10 * s, "#6E4A2E", hl=False)
        cv.draw(tr)
        cn = OL(k=-0.45)
        greens = [("#2F5A30", "#4F7A45"), ("#3F6B3A", "#5E8A50"), ("#4F7A45", "#7FA86B")] if kind == 0 else [("#7A5A2E", "#C9873A"), ("#8A4A2E", "#D9734F"), ("#9A6B37", "#E9A25A")]
        for i, (dark, light) in enumerate(greens):
            r_ = (7 - i * 1.8) * s
            cn.box(x - r_, y - r_, (9 + i * 5) * s, 2 * r_, 2 * r_, 5 * s, C(dark, light), top=lit(C(dark, light), 0.15))
        cv.draw(cn)

    def bush(x, y, s=1.0, flower=None):
        b = OL(k=-0.45)
        b.box(x - 3 * s, y - 3 * s, 0, 6 * s, 6 * s, 3.5 * s, C("#2F5A30", "#5E8A50"), top=C("#3F6B3A", "#7FA86B"))
        b.box(x - 2 * s, y - 2 * s, 3.5 * s, 4 * s, 4 * s, 2 * s, C("#3F6B3A", "#6E9B5C"), hl=False)
        cv.draw(b)
        if flower:
            fl_ = OL(outline=False)
            for i in range(4):
                fl_.put(*P(x - 2 + (i * 1.7) % 4, y - 2 + (i * 2.3) % 4, 5.6 * s), hexc(flower))
            cv.draw(fl_)

    def lamp_post(x, y):
        lp = OL(k=-0.6)
        lp.box(x - 1.2, y - 1.2, 0, 2.4, 2.4, 1, "#2B2826", hl=False)
        lp.box(x - 0.4, y - 0.4, 1, 0.8, 0.8, 22, "#2B2826", hl=False)
        lp.box(x - 1.6, y - 1.6, 23, 3.2, 3.2, 3, "#2B2826", top=hexc("#3A3734"))
        cv.draw(lp)
        g = OL(outline=False, emissive=not DAY)
        g.face_y(y + 1.62, x - 1.2, 23.4, 2.4, 2.2, hexc("#FFE7A8") if not DAY else hexc("#E8E2D0"))
        g.face_x(x + 1.62, y - 1.2, 23.4, 2.4, 2.2, hexc("#FFE7A8") if not DAY else hexc("#D9D2BF"))
        cv.draw(g)

    def fence(x0, y0, x1, y1):
        f = OL(k=-0.55)
        n = int(max(abs(x1 - x0), abs(y1 - y0)) // 4)
        for i in range(n + 1):
            px = x0 + (x1 - x0) * i / max(1, n)
            py = y0 + (y1 - y0) * i / max(1, n)
            f.box(px - 0.6, py - 0.6, 0, 1.2, 1.2, 7, "#E3D3BC", top=hexc("#FFFFFF"), hl=False)
        if x1 == x0:
            f.box(x0 - 0.3, min(y0, y1), 4.5, 0.6, abs(y1 - y0), 0.8, "#E3D3BC", hl=False)
            f.box(x0 - 0.3, min(y0, y1), 2, 0.6, abs(y1 - y0), 0.8, "#E3D3BC", hl=False)
        else:
            f.box(min(x0, x1), y0 - 0.3, 4.5, abs(x1 - x0), 0.6, 0.8, "#E3D3BC", hl=False)
            f.box(min(x0, x1), y0 - 0.3, 2, abs(x1 - x0), 0.6, 0.8, "#E3D3BC", hl=False)
        cv.draw(f)

    # far tree line behind the house (shows above the back walls)
    for (tx, ty, s_, k_) in ((-70, 10, 1.4, 0), (-74, 46, 1.6, 1), (-68, 84, 1.3, 0), (-80, 120, 1.5, 0),
                             (10, -72, 1.5, 0), (52, -78, 1.3, 1), (92, -70, 1.6, 0), (136, -82, 1.4, 0)):
        tree(tx, ty, s_, k_)
    for (bx, by) in ((-60, 30), (-58, 66), (-62, 102), (30, -60), (72, -62), (116, -60), (-30, -36), (-96, 70), (160, -40)):
        bush(bx, by, 1.3)
    for (tx, ty, s_, k_) in ((-36, -50, 1.3, 1), (-110, 40, 1.4, 0), (150, -60, 1.2, 1), (-50, 150, 1.3, 0)):
        tree(tx, ty, s_, k_)
    rk = OL(k=-0.5)
    for (rx_, ry_, r_) in ((-40, 20, 2.4), (-20, -20, 1.8), (100, -40, 2.2), (RX + 70, 40, 2.6), (60, RY + 50, 2)):
        rk.box(rx_ - r_, ry_ - r_, 0, 2 * r_, 2 * r_, r_, C("#5A5D6A", "#B9B9B9"), top=C("#6B6F7A", "#D9D9D9"))
    cv.draw(rk)

    # ---------------- walls ----------------
    wall = OL(outline=False)
    WLc, WRc = C("#5A6176", "#EDE3D3"), C("#4A5064", "#DCCFBB")
    for sy in range(H):
        for sx in range(W):
            y = (ps.OX - sx) / 2
            if 0 <= y <= RY:
                z = ps.OY + y - sy
                if 0 <= z <= RH:
                    c = WLc
                    if int(y) % 8 == 0:
                        c = lit(c, -0.1)
                    elif int(y) % 8 == 1:
                        c = lit(c, 0.05)
                    if z < 4:
                        c = C("#2F3342", "#B89C7C")
                    elif z < 5:
                        c = C("#6B7389", "#F6EFE4")
                    wall.put(sx, sy, c)
            x = (sx - ps.OX) / 2
            if 0 <= x <= RX:
                z = ps.OY + x - sy
                if 0 <= z <= RH:
                    c = WRc
                    if int(x) % 8 == 0:
                        c = lit(c, -0.1)
                    elif int(x) % 8 == 1:
                        c = lit(c, 0.05)
                    if z < 4:
                        c = C("#2A2E3B", "#A88C6C")
                    elif z < 5:
                        c = C("#5D6479", "#EFE6D8")
                    wall.put(sx, sy, c)
    cv.draw(wall)
    cap = OL(outline=False)
    cap.poly([P(-3, -3, RH), P(0, -3, RH), P(0, RY, RH), P(-3, RY, RH)], C("#2B2F3B", "#7E6A55"))
    cap.poly([P(-3, -3, RH), P(RX, -3, RH), P(RX, 0, RH), P(0, 0, RH)], C("#2B2F3B", "#7E6A55"))
    cap.face_y(RY, -3, 0, 3, RH, C("#3B4050", "#9C8670"))
    cap.face_x(RX, -3, 0, 3, RH, C("#333847", "#8C7660"))
    cv.draw(cap)

    # ---------------- left-wall decor ----------------
    # whiteboard (plan)
    wb = OL(k=-0.5)
    wb.face_x(0, 4, 17, 24, 21, hexc("#9AA3AE"))
    wb.face_x(0, 4.8, 17.8, 22.4, 19.4, hexc("#FBFCFD"))
    wb.face_x(0, 6.5, 33, 8, 0.7, hexc("#5B7FD9"))
    wb.face_x(0, 6.5, 30.6, 12, 0.5, hexc("#A9B4D6"))
    wb.face_x(0, 6.5, 28.8, 9, 0.5, hexc("#A9B4D6"))
    wb.face_x(0, 6.5, 21, 4, 3.4, hexc("#E9875A"))
    wb.face_x(0, 11, 22.5, 2.6, 0.4, hexc("#555555"))
    wb.face_x(0, 14, 21, 4, 3.4, hexc("#3E9C8E"))
    wb.face_x(0, 21.5, 29.6, 4.4, 4.4, hexc("#F7E07A"))
    wb.face_x(0, 21.5, 23.8, 4.4, 4.4, hexc("#F4C2D6"))
    wb.box(0, 5, 16, 2.6, 21, 0.9, "#9AA3AE")
    cv.draw(wb)
    # two windows
    for wy in (32, 56):
        fr = OL(k=-0.4)
        fr.face_x(0, wy, 20, 20, 28, hexc("#C9CFDA") if not DAY else hexc("#FFFFFF"))
        cv.draw(fr)
        sk = OL(outline=False, emissive=not DAY)
        for sy in range(H):
            for sx in range(W):
                y = (ps.OX - sx) / 2
                z = ps.OY + y - sy
                if wy + 1 <= y <= wy + 19 and 21 <= z <= 47:
                    tt = (z - 21) / 26
                    c = mix(C("#3B4F86", "#D6ECFA"), C("#16213F", "#86BDEB"), tt)
                    if not DAY and h32(sx, sy) % 97 == 0 and z > 32:
                        c = hexc("#E8EEFF")
                    if DAY and 37 < z < 44 and ((y - wy - (z - 38) * 0.3) % 11) < 3.6 and abs(z - 40.5) < 2.4 - ((y - wy) % 11) * 0.22:
                        c = hexc("#FFFFFF")
                    bh = 24 + (h32(int(y // 2.3)) % 7)
                    if z < bh:
                        c = C("#1B2236", "#9DB3C8")
                        if (h32(sx, sy // 2) % 9 == 0) and z < bh - 1:
                            c = C("#F2C46B", "#DDE8F2")
                        if DAY and z < 24.5:
                            c = hexc("#7FA86B") if h32(sx, sy) % 3 else hexc("#6E9B5C")
                    sk.put(sx, sy, c)
        cv.draw(sk)
        bl = OL(outline=False)
        for i in range(6):
            bl.face_x(0.05, wy + 1, 47 - i * 1.6, 18, 0.55, hexc("#D9DEE8"))
        bl.face_x(0.05, wy + 10, 21, 0.6, 26, hexc("#FFFFFF") if DAY else hexc("#B9C0CE"))
        cv.draw(bl)
        sl = OL(k=-0.5)
        sl.box(0, wy - 1, 19, 3, 22, 1.2, "#D9DEE8")
        cv.draw(sl)
    if not DAY:
        moon = OL(outline=False, emissive=True)
        for dy in range(-2, 3):
            for dx in range(-2, 3):
                if dx * dx + dy * dy <= 5:
                    p_ = P(0, 70, 42)
                    moon.put(p_[0] + dx, p_[1] + dy, hexc("#FFF4CF"))
        cv.draw(moon)
    cact = OL(k=-0.5)
    cact.box(0.5, 50, 20.2, 2.5, 2.5, 2.2, "#C4703A")
    cact.box(1.1, 50.6, 22.4, 1.3, 1.3, 3.4, "#5E8A50")
    cv.draw(cact)
    # poster above TV
    po = OL(k=-0.5)
    po.face_x(0, 80, 26, 18, 22, hexc("#1E1C1A"))
    po.face_x(0, 81, 27, 16, 20, hexc("#F2E9DC"))
    po.face_x(0, 81, 27, 16, 6, hexc("#34406B"))
    for i, col in enumerate(("#E9875A", "#3E9C8E", "#D9719B")):
        po.face_x(0, 82.5 + i * 5, 35, 4, 4, hexc(col))
        po.face_x(0, 83 + i * 5, 36.3, 3, 1.4, hexc("#2B2620"))
        po.face_x(0, 83.5 + i * 5, 36.6, 0.7, 0.8, hexc("#FFF8E6"))
        po.face_x(0, 85 + i * 5, 36.6, 0.7, 0.8, hexc("#FFF8E6"))
    po.face_x(0, 83, 30, 12, 0.8, hexc("#F2B45A"))
    cv.draw(po)

    # ---------------- right-wall decor ----------------
    rs = OL(k=-0.55)
    rs.face_y(0, 25, 20, 20, 16, hexc("#24211E"))
    cv.draw(rs)
    rsc = OL(outline=False, emissive=True)
    rsc.face_y(0.02, 25.8, 20.8, 18.4, 14.4, hexc("#18233A"))
    rsc.face_y(0.03, 26.6, 33, 16.4, 1.6, hexc("#2B3A63"))
    st_ = [hexc("#4FA36A"), hexc("#4FA36A"), hexc("#3B82F6") if t % 2 else hexc("#7FB3F0"), hexc("#6F665A"), hexc("#F2B45A")]
    for i in range(5):
        rsc.face_y(0.04, 27.4 + i * 3.4, 26, 2.3, 2.8, st_[i])
        if i < 4:
            rsc.face_y(0.04, 29.7 + i * 3.4, 27.1, 1.1, 0.5, hexc("#4FA36A") if i < 2 else hexc("#5A5550"))
    rsc.face_y(0.04, 27.4, 22.6, 10, 0.6, hexc("#A9DDD3"))
    cv.draw(rsc)
    # neon (night) / clock (day)
    ne = OL(outline=False, emissive=not DAY and t != 2)
    if DAY:
        for a_ in range(48):
            ang = a_ / 48 * math.tau
            for r_ in (4.2, 4.7):
                ne.put(*P(35 + math.cos(ang) * r_, 0.05, 44 + math.sin(ang) * r_ * 0.9), hexc("#3B3530"))
        for xx in range(-4, 5):
            for zz in range(-4, 5):
                if xx * xx + zz * zz < 14:
                    ne.put(*P(35 + xx, 0.05, 44 + zz), hexc("#FBF7F1"))
        for i in range(4):
            ne.put(*P(35, 0.05, 44 + i * 0.8), hexc("#2B2620"))
            ne.put(*P(35 + i * 0.7, 0.05, 44), hexc("#B8322A"))
    else:
        nc = hexc("#FF7AB6") if t != 2 else hexc("#8A4A68")
        for (x0, z0, w_, h_) in ((30, 47, 10, 0.8), (30, 40, 10, 0.8), (30, 40, 0.8, 7.8), (39.2, 40, 0.8, 7.8),
                                 (32.5, 43.5, 1.4, 1.6), (36, 43.5, 1.4, 1.6), (34.6, 47.8, 0.8, 2.6)):
            ne.face_y(0.05, x0, z0, w_, h_, nc)
    cv.draw(ne)
    # wall shelf with props
    sh = OL(k=-0.55)
    sh.box(74, 0, 34, 12, 5, 1.4, "#7A5536", top=hexc("#9C7049"))
    cv.draw(sh)
    bk = OL(k=-0.5)
    x = 74.6
    for col, hh in (("#5B7FD9", 9), ("#E9875A", 10.5), ("#E8C766", 8)):
        bk.box(x, 1, 35.4, 1.8, 3.6, hh, col)
        x += 1.9
    bk.box(81, 1.5, 35.4, 3, 2.5, 1, "#8E6542")
    bk.box(81.6, 2, 36.4, 1.8, 1.5, 3, "#E6B54A", top=hexc("#FFE08A"))
    cv.draw(bk)
    # expansion archway (locked room)
    arch = OL(k=-0.5)
    arch.face_y(0, 88, 0, 14, 34, C("#22242C", "#8C7660"))
    arch.face_y(0.02, 89, 0, 12, 32.5, C("#14151A", "#5E5043"))
    for i in range(6):
        arch.face_y(0.04, 89 + i * 2, 12, 1, 2, hexc("#F2C94C"))
        arch.face_y(0.04, 90 + i * 2, 12, 1, 2, hexc("#2B2620"))
    arch.face_y(0.05, 91, 27, 8, 4, hexc("#34406B"))
    arch.face_y(0.06, 92, 28.4, 6, 1.2, hexc("#F2B45A"))
    cv.draw(arch)
    cone = OL(k=-0.5)
    cone.box(90, 4, 0, 3, 3, 1, "#E9875A")
    cone.box(90.7, 4.7, 1, 1.6, 1.6, 4, "#F28A3C", top=hexc("#FFFFFF"))
    cone.box(97, 5, 0, 3, 3, 1, "#E9875A")
    cone.box(97.7, 5.7, 1, 1.6, 1.6, 4, "#F28A3C", top=hexc("#FFFFFF"))
    cv.draw(cone)
    bub(95, 0, 38, "新房间 · 再合并 2 件解锁", None, "lock")
    # door behind your desk
    dr = OL(k=-0.5)
    dr.face_y(0, 106, 0, 15, 31, hexc("#6E3712"))
    dr.face_y(0.02, 107, 0, 13, 30, hexc("#B5651D"))
    for (dx, dz) in ((108, 16), (114, 16), (108, 3), (114, 3)):
        dr.face_y(0.04, dx, dz, 5, 11.5, hexc("#C47A35"))
    dr.face_y(0.05, 118, 13.5, 1.4, 1.6, hexc("#E6B54A"))
    cv.draw(dr)

    # ---------------- floor ----------------
    fl = OL(outline=False)
    fl.floor = True
    for sy in range(H):
        for sx in range(W):
            x, y = inv(sx, sy)
            if 0 <= x < RX and 0 <= y < RY:
                plank = int(x // 4)
                off = h32(plank, 7) % 23
                seg = int((y + off) // 19)
                tone = (h32(plank, seg) % 1000) / 1000
                c = mix(C("#8C6444", "#C99D6C"), C("#A87C55", "#DDB585"), tone)
                if (x % 4) < 0.5:
                    c = lit(c, -0.3)
                elif ((y + off) % 19) < 0.55:
                    c = lit(c, -0.22)
                elif h32(int(x * 4), int(y * 1.5)) % 7 == 0:
                    c = lit(c, -0.07)
                ao = max(0, 1 - min(x, y) / 6)
                c = lit(c, -0.3 * ao)
                fl.put(sx, sy, c)
    cv.draw(fl)

    # rugs
    def rug(cx_, cy_, rx_, ry_, inner, edge):
        r = OL(outline=False)
        r.floor = True
        for sy in range(H):
            for sx in range(W):
                x, y = inv(sx, sy)
                d = ((x - cx_) / rx_) ** 2 + ((y - cy_) / ry_) ** 2
                if d < 1:
                    c = inner if d < 0.82 else edge
                    if h32(sx, sy) % 5 == 0:
                        c = lit(c, 0.1)
                    elif h32(sx + 1, sy) % 6 == 0:
                        c = lit(c, -0.1)
                    r.put(sx, sy, c)
        cv.draw(r)

    rug(22, 90, 20, 13, C("#D9CBB4", "#A9BFA0"), C("#BFAE93", "#8FA886"))
    rug(34, 62, 17, 9, C("#3E4A6B", "#C9B8E6"), C("#2F3A57", "#A898D0"))
    rug(110, 59, 14, 11, C("#4B5E52", "#E8D2B0"), C("#3A4B41", "#CFB48C"))

    # ---------------- contact shadows ----------------
    def shade_rect(x0, x1, y0, y1, a=0.32):
        for sy in range(H):
            for sx in range(W):
                x, y = inv(sx, sy)
                if x0 <= x <= x1 and y0 <= y <= y1:
                    dd = min(x - x0, x1 - x, y - y0, y1 - y)
                    cv.blend(sx, sy, (15, 12, 20), a * min(1, dd / 3))

    for (x0, x1, y0, y1) in ((15, 53, 38, 56), (57, 95, 38, 56), (57, 95, 72, 90), (101, 123, 11, 22), (103, 121, 52, 65),
                             (4, 22, 0, 9), (47, 71, 0, 10), (0, 7, 77, 99), (23, 37, 73, 101), (101, 124, 93, 103), (5, 11, 40, 52)):
        shade_rect(x0, x1, y0, y1)

    # ---------------- depth-sorted objects ----------------
    items = []

    def add(k, fn, robot=False):
        items.append((k, fn, robot))

    # bookshelf
    def bookshelf():
        b = OL(k=-0.58)
        b.box(4, 0, 0, 18, 8, 40, "#8E6542")
        b.face_y(8, 5, 1.4, 16, 37.2, hexc("#4E3622"))
        cv.draw(b)
        bb = OL(k=-0.55)
        cols = ["#5B7FD9", "#E9875A", "#6E8B5E", "#E8C766", "#D9719B", "#3E9C8E", "#8D7BD6", "#C9605F", "#F3EEE7", "#34406B"]
        for s_, z in enumerate((1.4, 14, 26.6)):
            x, i = 5.3, s_ * 4
            while x < 20.2:
                w_ = 1.1 + ((i * 5) % 3) * 0.35
                hh = 7.5 + ((i * 7) % 4) * 0.9
                if (i + s_) % 9 == 4:
                    x += 2.6
                else:
                    bb.box(x, 1.5, z, w_, 6.3, hh, cols[i % len(cols)], hl=False)
                    x += w_ + 0.12
                i += 1
            bb.box(4.6, 0.6, z + 11.2, 16.8, 7.4, 1.4, "#8E6542")
        cv.draw(bb)
    add(16, bookshelf)
    add(4, lambda: plant(cv, 122, 1, 0.9))

    # racks + runner
    def racks():
        for ri, x in enumerate((48, 59)):
            r = OL(k=-0.6)
            r.box(x, 0, 0, 10, 9, 40, "#33302D")
            r.face_y(9, x + 0.6, 0.6, 8.8, 38.8, hexc("#24211F"))
            for u in range(8):
                z = 2.5 + u * 4.6
                r.face_y(9.02, x + 1, z, 8, 3.6, hexc("#3A3734"))
                for v in range(3):
                    r.face_y(9.04, x + 4.5, z + 0.8 + v * 0.9, 3.8, 0.25, hexc("#2A2725"))
            cv.draw(r)
            ld = OL(outline=False, emissive=True)
            for u in range(8):
                z = 2.5 + u * 4.6
                for j in range(3):
                    on = (u * 3 + j + ri + t) % 7
                    col = hexc("#3A3734") if on == 0 else (hexc("#F2B45A") if (u + j) % 5 == 2 else hexc("#4FE08A"))
                    ld.face_y(9.06, x + 1.5 + j, z + 1.4, 0.6, 0.6, col)
            cv.draw(ld)
    add(62, racks)
    add(78, lambda: robot_front(cv, 59, 13, "runner", "work", frame), True)
    bub(63, 16, 30, "验证机 · 跑测试 214/260")

    # Qoder at the whiteboard
    add(36, lambda: robot_front(cv, 7, 26, "qoder", "work", frame), True)
    bub(11, 29, 30, "Qoder · 拆步骤", "T-016", dx=-10)

    # desks row 1 and 2
    def desk_group(x0, y0, who, acc, lit_, lamp, seed, label):
        cx, cy = x0 + 20, y0 + 24
        k = x0 + 20 + y0 + 9
        add(k, lambda: desk(cv, x0, y0, frame, lit_ if LIT_DESKS is None else LIT_DESKS, lamp, seed))
        add(cx + cy - 0.5, lambda: chair(cv, cx, cy, "seat"))
        if who:
            add(cx + cy, lambda: robot_back(cv, cx, cy, who, frame, acc), True)
        add(cx + cy + 4, lambda: chair(cv, cx, cy, "back"))
        if label:
            bub(cx, cy - 4, 34 if who else 26, *label)

    desk_group(14, 38, "claude", ("phones",), True, True, 0, ("小橙 · 写代码", "T-013"))
    desk_group(56, 38, "kimi", ("scarf",), True, False, 3, ("Kimi · 改第 2 版", "T-015"))
    if VARIANT == "desk":
        desk_group(56, 72, "grok", (), True, False, 5, None)
    else:
        desk_group(56, 72, None, (), False, False, 0, ("空工位 · 等人派活", None, "idle"))

    # PC tower next to the empty desk
    def pc():
        p = OL(k=-0.6)
        p.box(5, 40, 0, 6, 12, 17, "#1C1D24", top=hexc("#2A2C36"), left=hexc("#24262F"), right=hexc("#15161C"))
        cv.draw(p)
        g = OL(outline=False, emissive=True)
        for i, zc in enumerate((5, 11)):
            col = PAL[(t + i) % 4]
            for a in range(16):
                ang = a / 16 * math.tau
                g.put(*P(8 + math.cos(ang) * 1.9, 52, zc + 2 + math.sin(ang) * 1.9), col)
        cv.draw(g)
    add(52, pc)

    # your desk + you, queue
    def boss_desk():
        d = OL(k=-0.6)
        d.box(102, 12, 0, 20, 9, 19, "#A9773F", top=hexc("#C8955C"))
        d.face_y(21, 103, 1, 18, 16, hexc("#9A6B37"))
        d.face_y(21.02, 109.5, 12, 5, 2.4, hexc("#E6B54A"))
        d.box(103, 13, 19, 4.5, 0.8, 5, "#26231F")
        d.box(115, 15, 19, 1.8, 1.8, 2.4, "#B8322A")
        d.box(115.5, 15.5, 21.4, 0.8, 0.8, 1.4, "#26231F", hl=False)
        d.box(108, 16, 19, 4, 2.6, 0.3, "#FFFFFF", hl=False)
        cv.draw(d)
        plant(cv, 118, 13, 0.35, "#E3D3BC")
    add(108, lambda: human_front(cv, 108, 5, 15, frame), True)
    add(130, boss_desk)
    bub(112, 8, 36, "你 · 拍板", None, "idle")
    add(124.5, lambda: robot_front(cv, 96, 25, "gemini", "hand", frame), True)
    add(122, lambda: robot_front(cv, 89, 29, "deepseek", "hand", (frame + 1) % 4), True)
    bub(101, 28, 30, "Gemini · 想装依赖", "T-009", "human", dx=24)

    # review table + codex
    add(157, lambda: robot_front(cv, 107, 46, "codex", "work", frame, z0=8, acc=("glasses",)), True)
    def table():
        tb = OL(k=-0.6)
        tb.box(111, 57, 0, 2, 2, 11, "#5A5550", hl=False)
        tb.box(108.5, 55, 0, 7, 6, 0.7, "#5A5550", hl=False)
        tb.box(104, 52, 11, 16, 12, 1.4, "#B07C47", top=hexc("#CF9C66"))
        tb.box(106, 54, 12.4, 4, 3, 0.25, "#FFFFFF", hl=False)
        tb.box(113, 57, 12.4, 3.4, 4, 0.25, "#FFFFFF", hl=False)
        tb.box(108, 61, 12.4, 1.4, 1.4, 1.8, "#FFFFFF")
        tb.box(117, 54, 12.4, 1.6, 2.2, 0.25, "#2B2620", hl=False)
        cv.draw(tb)
        st = OL(k=-0.55)
        st.box(109, 67, 0, 5, 5, 7, "#8E6542", top=hexc("#B08560"))
        cv.draw(st)
    add(170, table)
    bub(111, 49, 30, "Codex · 读改动", "T-012")

    # lounge: TV, sofa, robots, floor lamp
    def tv():
        u = OL(k=-0.58)
        u.box(0, 78, 0, 6, 21, 8, "#8E6542")
        u.face_x(6, 79.5, 1.5, 9, 5, hexc("#6E4A2E"))
        u.face_x(6, 89.5, 1.5, 8, 5, hexc("#6E4A2E"))
        u.box(1, 80, 10, 1.3, 18, 13, "#1C1A18")
        u.box(1.6, 87.5, 8, 1.6, 3, 2, "#26231F", hl=False)
        cv.draw(u)
        s = OL(outline=False, emissive=True)
        tvs = [("#5B7FD9", "#9DC3F0"), ("#8D7BD6", "#E3D7FF"), ("#3E9C8E", "#B5E8DE"), ("#E9875A", "#FFD9A8")]
        a_, b_ = tvs[t % 4]
        s.face_x(2.32, 80.6, 10.6, 16.8, 11.8, hexc(b_))
        s.face_x(2.34, 80.6, 10.6, 16.8, 4, hexc(a_))
        s.face_x(2.36, 90, 16, 2.4, 2.4, hexc("#F7E07A"))
        cv.draw(s)
        u2 = OL(k=-0.5)
        u2.box(2, 95, 8, 3.2, 2.2, 1.2, "#3A3632")
        cv.draw(u2)
    add(80, tv)

    def floor_lamp():
        l = OL(k=-0.6)
        l.box(6, 70, 0, 3, 3, 0.6, "#3A3632", hl=False)
        l.box(7.2, 71.2, 0.6, 0.6, 0.6, 26, "#3A3632", hl=False)
        l.box(5.2, 69.2, 26, 4.6, 4.6, 5, "#F3E3C3", top=hexc("#FFF7E6"))
        cv.draw(l)
        if not DAY:
            g = OL(outline=False, emissive=True)
            g.face_y(73.8, 5.4, 26.2, 4.2, 1, hexc("#FFF2C4"))
            cv.draw(g)
    add(76, floor_lamp)

    def sofa(part):
        s = OL(k=-0.6)
        if part == "seat":
            s.box(24, 75, 0, 9, 24, 6, "#7D6CC4")
            s.box(24.6, 75.6, 6, 7.6, 11, 1.6, "#9988DA", top=hexc("#A99AE4"))
            s.box(24.6, 87.3, 6, 7.6, 11, 1.6, "#9988DA", top=hexc("#A99AE4"))
            s.box(24, 73, 0, 12.5, 2.2, 9.5, "#6D5CB4")
        elif part == "back":
            s.box(32.5, 75, 0, 4, 24, 13.5, "#6D5CB4", left=hexc("#7867BF"))
            s.box(32, 77, 9, 1.4, 6, 4, "#F2B45A", hl=False)
        else:
            s.box(24, 99, 0, 12.5, 2.2, 9.5, "#6D5CB4")
        cv.draw(s)
    add(100, lambda: sofa("seat"))
    if VARIANT == "sofa":
        add(110, lambda: robot_side(cv, 29, 82, "grok", frame, z=7.6), True)
    add(120, lambda: robot_side(cv, 29, 92, "claude", frame, z=7.6, acc=("flower", "pad")), True)
    add(128, lambda: sofa("back"))
    add(132, lambda: sofa("arm"))
    if VARIANT == "sofa":
        bub(27, 79, 33, "Grok · 看电影", None, "idle", dx=26)
    bub(27, 92, 33, "阿花 · 打一局", None, "idle", dx=-34)

    # beanbag + minimax + dock
    def beanbag():
        for sy in range(H):
            for sx in range(W):
                x, y = inv(sx, sy)
                if ((x - 46) / 6.5) ** 2 + ((y - 95) / 6) ** 2 < 1:
                    cv.blend(sx, sy, (15, 12, 20), 0.3)
        bb = OL(k=-0.55)
        bb.box(41, 90, 0, 10, 10, 3, "#D06E42", top=hexc("#E9875A"))
        bb.box(42, 91, 3, 8, 8, 2, "#E9875A", top=hexc("#F2A07A"))
        cv.draw(bb)
    add(136, beanbag)
    add(142, lambda: robot_front(cv, 42, 93, "minimax", "sleep", frame, z0=4.5), True)
    bub(47, 96, 28, "MiniMax · 充电中 4%", None, "warn", dx=38)

    def dock():
        d = OL(k=-0.6)
        d.box(53, 96, 0, 4, 4, 16, "#2B2826")
        d.face_y(100, 53.6, 2.6, 2.8, 11, hexc("#F3EEE7"))
        cv.draw(d)
        e = OL(outline=False, emissive=not DAY)
        e.face_y(100.02, 54.1, 3.4, 1.8, 1.3, hexc("#D9534F") if t % 3 else hexc("#F0C0BF"))
        e.face_y(100.02, 53.6, 0.6, 2.8, 0.8, hexc("#4FE08A"))
        cv.draw(e)
    add(152, dock)

    # tea bar + qwen
    def teabar():
        tb = OL(k=-0.6)
        tb.box(102, 94, 0, 22, 8, 13, "#A9773F", top=hexc("#F1EBE2"))
        tb.face_y(102, 103, 1.5, 6, 9.5, hexc("#9A6B37"))
        tb.face_y(102, 110.5, 1.5, 6, 9.5, hexc("#9A6B37"))
        tb.face_y(102, 118, 1.5, 5, 9.5, hexc("#9A6B37"))
        tb.box(104.5, 95.5, 13, 3, 3, 3.2, "#3E9C8E")
        tb.box(109, 96.5, 13, 1.5, 1.5, 1.6, "#FFFFFF")
        tb.box(111.5, 96, 13, 1.5, 1.5, 1.6, "#F4C2D6")
        tb.box(114, 95.5, 13, 1.6, 2.4, 2.6, "#E8C766")
        tb.box(118, 95, 13, 4, 4, 4, "#E9875A", top=hexc("#F5A57A"))
        cv.draw(tb)
        for i in range(3):
            ph = (t + i) % 4
            sx_, sy_ = P(106, 97, 17.5 + ph * 1.6)
            cv.blend(sx_ + (i % 2), sy_, (235, 235, 245), 0.55 - ph * 0.1)
    add(196, teabar)
    add(186, lambda: robot_front(cv, 95, 89, "qwen", "cup", frame), True)
    bub(99, 92, 30, "Qwen · 喝茶", None, "idle", dx=-10)

    # bin + front plant
    def bin_():
        b = OL(k=-0.55)
        b.box(6, 54, 0, 6, 6, 8, "#3A3F4C", top=hexc("#1E2028"))
        for i in range(3):
            b.face_y(60, 7 + i * 1.8, 1, 0.6, 6, hexc("#4E5566"))
        b.box(7, 55, 8, 2.4, 2.4, 1.2, "#F3EEE7", hl=False)
        cv.draw(b)
    add(60, bin_)
    add(222, lambda: plant(cv, 121, 99, 0.9))


    # ================= decorations =================
    def vine(l, xp, y, z, n, side="x"):
        cols = ["#4F7A45", "#6E9B5C", "#88B474", "#5E8A50"]
        for i in range(n):
            zz = z - i * 1.3
            off = math.sin(i * 1.3) * 0.8
            if side == "x":
                l.face_x(xp + 0.3, y + off, zz, 1.4, 1.2, hexc(cols[i % 4]))
            else:
                l.face_y(xp + 0.3, y + off, zz, 1.4, 1.2, hexc(cols[i % 4]))

    def wall_decor():
        # string lights along the top of both walls
        sl = OL(outline=False)
        for y in range(2, RY - 2, 1):
            sl.put(*P(0.1, y, 52 + math.sin(y / 3.2) * 1.2), hexc("#5A4A3A"))
        for x in range(2, RX - 2, 1):
            sl.put(*P(x, 0.1, 52 + math.sin(x / 3.2) * 1.2), hexc("#5A4A3A"))
        cv.draw(sl)
        bulbs = OL(outline=False, emissive=not DAY)
        bc = [hexc("#FFD27A"), hexc("#FF9E7A"), hexc("#FFE9A8"), hexc("#9EE6FF")]
        for i, y in enumerate(range(4, RY - 2, 5)):
            on = DAY or (i + t) % 5 != 0
            c = bc[i % 4] if on else hexc("#6B5A3A")
            p_ = P(0.1, y, 50.6 + math.sin(y / 3.2) * 1.2)
            bulbs.put(*p_, c)
            bulbs.put(p_[0], p_[1] + 1, c)
        for i, x in enumerate(range(4, RX - 2, 5)):
            on = DAY or (i + t + 2) % 5 != 0
            c = bc[(i + 1) % 4] if on else hexc("#6B5A3A")
            p_ = P(x, 0.1, 50.6 + math.sin(x / 3.2) * 1.2)
            bulbs.put(*p_, c)
            bulbs.put(p_[0], p_[1] + 1, c)
        cv.draw(bulbs)
        # hanging planters on the left wall (between whiteboard/window and between windows)
        for hy in (29.2, 53):
            h = OL(k=-0.55)
            h.box(0, hy - 0.4, 40, 3, 3, 3, "#E3D3BC", top=hexc("#4E3622"))
            for d in (0, 1.6):
                h.put(*P(1.5, hy + 1, 43 + d), hexc("#8C7660"))
            cv.draw(h)
            v = OL(k=-0.45)
            vine(v, 0.2, hy, 40, 8)
            vine(v, 0.2, hy + 1.6, 39, 6)
            v.box(-0.2, hy - 0.8, 42.6, 3.6, 3.6, 1.4, "#6E9B5C", hl=False)
            cv.draw(v)
        # framed pictures above the server racks + plant shelf with trailing pothos
        fr = OL(k=-0.5)
        for (x0, w_, col) in ((49, 8, "#5B7FD9"), (59, 10, "#E9875A")):
            fr.face_y(0, x0, 44, w_, 9, hexc("#3B3530"))
            fr.face_y(0.02, x0 + 0.8, 44.8, w_ - 1.6, 7.4, hexc("#F6F0E7"))
            fr.face_y(0.04, x0 + 1.6, 45.6, w_ - 3.2, 3.2, hexc(col))
            fr.face_y(0.05, x0 + 2 + (w_ - 4) / 2, 48.6, 1.4, 1.4, hexc("#F7E07A"))
        fr.face_y(0, 74, 41, 12, 8, hexc("#3B3530"))
        fr.face_y(0.02, 74.7, 41.7, 10.6, 6.6, hexc("#FBF7F1"))
        for i, col in enumerate(("#E9875A", "#3E9C8E", "#D9719B", "#5B7FD9", "#C2A14A")):
            fr.face_y(0.04, 75.4 + i * 2, 42.6 + (i % 2), 1.4, 1.4, hexc(col))
        fr.face_y(0.04, 75.4, 46, 9.4, 0.6, hexc("#34406B"))
        cv.draw(fr)
        # wall calendar by the door
        cal = OL(k=-0.5)
        cal.face_y(0, 102, 24, 3.6, 5, hexc("#FFFFFF"))
        cal.face_y(0.02, 102, 28, 3.6, 1, hexc("#B8322A"))
        for r in range(3):
            for c_ in range(3):
                cal.put(*P(102.6 + c_ * 1.1, 0.04, 26.8 - r * 1.1), hexc("#8C8C8C") if (r, c_) != (1, 1) else hexc("#B8322A"))
        cv.draw(cal)

    add(-50, wall_decor)

    # trailing pothos on top of the bookshelf
    def shelf_top():
        p_ = OL(k=-0.5)
        p_.box(6, 2, 40, 3.4, 3.4, 3, "#E3D3BC", top=hexc("#4E3622"))
        p_.box(15, 2, 40, 4, 3, 2.4, "#C9605F")
        p_.box(15.5, 2.2, 42.4, 3, 2.6, 1.6, "#5B7FD9")
        cv.draw(p_)
        v = OL(k=-0.45)
        for i in range(9):
            v.box(6.4 + math.sin(i) * 0.6, 5.4, 41 - i * 1.6, 1.4, 1.2, 1.4, ["#4F7A45", "#6E9B5C", "#88B474"][i % 3], hl=False)
        v.box(5.8, 1.6, 43, 4.2, 4.2, 2, "#6E9B5C", hl=False)
        cv.draw(v)
    add(17, shelf_top)

    # aquarium under window 2 (left wall)
    def aquarium():
        a = OL(k=-0.58)
        a.box(0, 58, 0, 6, 16, 9, "#8E6542", top=hexc("#A9773F"))
        a.face_x(6, 59.5, 1.5, 6, 6, hexc("#6E4A2E"))
        a.face_x(6, 66.5, 1.5, 6, 6, hexc("#6E4A2E"))
        cv.draw(a)
        g = OL(k=-0.5, emissive=not DAY)
        water = C("#2F6FA0", "#7FC4E8")
        g.box(0.6, 58.8, 9, 4.8, 14.4, 8.5, "#2B2620", top=lit(water, 0.25), left=water, right=lit(water, -0.15), hl=False)
        g.face_y(73.22, 1, 9.4, 4, 1.2, hexc("#E8C766"))
        cv.draw(g)
        f = OL(outline=False, emissive=True)
        for i, (fy0, fz, col) in enumerate(((61, 14, "#FF8A4C"), (66, 12, "#F2C94C"), (69, 15.5, "#FF6B9A"))):
            fy_ = fy0 + ((t + i * 2) % 4) * 0.8
            f.face_y(73.24, 1 + ((t + i) % 3) * 0.6, fz, 1.6, 0.9, hexc(col))
            f.face_x(5.42, fy_, fz, 1.6, 0.9, hexc(col))
        for i in range(3):
            f.put(*P(5.42, 63 + i * 2.5, 10 + ((t + i) % 4) * 1.8), hexc("#D6F1FF"))
        f.face_x(5.42, 60, 9.2, 1.2, 3, hexc("#4FA36A"))
        f.face_x(5.42, 71, 9.2, 1.2, 4, hexc("#6E9B5C"))
        cv.draw(f)
    add(66, aquarium)

    # cat sleeping on window sill 2 (tail flicks)
    def cat():
        c_ = OL(k=-0.6)
        c_.box(0.6, 61, 20.2, 3, 5, 2.2, "#E9A25A", top=hexc("#F2BC7E"))
        c_.box(0.6, 60, 20.2, 3, 2.2, 3, "#E9A25A", top=hexc("#F2BC7E"))
        c_.box(0.8, 60.1, 23.2, 0.8, 0.8, 0.8, "#E9A25A", hl=False)
        c_.box(2.4, 60.1, 23.2, 0.8, 0.8, 0.8, "#E9A25A", hl=False)
        tail = (0, 0.6, 1.2, 0.6)[t]
        c_.box(1.2, 66, 20.2, 1, 2.6, 0.8 + tail, "#D98A4A", hl=False)
        cv.draw(c_)
        st = OL(outline=False)
        for dz in (21, 22.4):
            st.face_x(3.62, 61.6, dz, 3, 0.4, hexc("#C97A3A"))
        st.face_y(62.22, 1.2, 21.6, 0.8, 0.4, hexc("#2B2620"))
        st.face_y(62.22, 2.4, 21.6, 0.8, 0.4, hexc("#2B2620"))
        cv.draw(st)
    add(67, cat)

    # tall monstera by the door + coat rack
    def monstera():
        p_ = OL(k=-0.5)
        p_.box(121, 24, 0, 6, 6, 7, "#34406B", top=hexc("#4E3622"))
        cv.draw(p_)
        lv = OL(k=-0.45)
        for (lx, ly, lz, w_, col) in ((119, 23, 9, 5, "#3F6B3A"), (123, 21, 12, 5, "#5E8A50"), (120, 27, 15, 5, "#4F7A45"),
                                      (124, 25, 18, 4.5, "#6E9B5C"), (121, 22, 21, 5, "#5E8A50"), (122, 25, 25, 4, "#7FA86B"),
                                      (120, 24, 28, 3.5, "#4F7A45"), (123, 23, 31, 3, "#88B474")):
            lv.box(lx, ly, lz, w_, w_ * 0.8, 1.6, col, hl=False)
            lv.flat(lx + w_ * 0.45, ly, lz + 1.61, 0.3, w_ * 0.8, lit(hexc(col), -0.25))
        for z in range(7, 30, 3):
            lv.put(*P(124, 27, z), hexc("#3F5A2E"))
        cv.draw(lv)
    add(150, monstera)

    def coat_rack():
        r_ = OL(k=-0.6)
        r_.box(102, 3, 0, 4, 4, 0.8, "#3A3632", hl=False)
        r_.box(103.6, 4.6, 0.8, 0.8, 0.8, 30, "#6E4A2E", hl=False)
        r_.box(101.8, 4.6, 27, 4.4, 0.8, 0.6, "#6E4A2E", hl=False)
        r_.box(101.4, 4, 21, 2, 1.8, 6, "#E9875A", top=hexc("#F5A57A"))
        r_.box(105, 4.2, 25, 2.4, 2.4, 2.2, "#34406B", top=hexc("#46528A"))
        r_.box(105.4, 4.6, 27.2, 1.6, 1.6, 0.8, "#34406B", hl=False)
        cv.draw(r_)
    add(108.5, coat_rack)

    # water cooler next to the shelf
    def cooler():
        w_ = OL(k=-0.58)
        w_.box(76, 3, 0, 5, 5, 13, "#E3E6EC", top=hexc("#F4F6F9"))
        w_.face_y(8, 77, 7, 3, 2, hexc("#B9C0CE"))
        w_.put(*P(77.8, 8.02, 9.6), hexc("#5B7FD9"))
        w_.put(*P(79.2, 8.02, 9.6), hexc("#D9534F"))
        cv.draw(w_)
        b_ = OL(k=-0.45)
        b_.box(76.8, 3.8, 13, 3.4, 3.4, 6, "#9ED0F0", top=hexc("#C6E6FA"), left=hexc("#8CC4EA"), right=hexc("#7AB4DC"))
        cv.draw(b_)
    add(84, cooler)

    # mini fridge by the tea bar + coffee machine on the bar
    def fridge():
        f_ = OL(k=-0.58)
        f_.box(124, 82, 0, 4, 9, 15, "#F3EEE7", top=hexc("#FFFFFF"))
        f_.face_x(128, 82.6, 8, 7.8, 0.4, hexc("#C9C2B6"))
        f_.face_x(128.02, 83, 10, 0.6, 3, hexc("#8C8C8C"))
        f_.face_x(128.02, 83, 4, 0.6, 2.4, hexc("#8C8C8C"))
        f_.face_x(128.02, 86, 11, 3, 2, hexc("#F7E07A"))
        cv.draw(f_)
    add(212, fridge)

    def coffee():
        m = OL(k=-0.6)
        m.box(118, 95, 13, 4.5, 4.5, 6, "#2B2826", top=hexc("#3A3734"))
        m.face_y(99.5, 119, 15.5, 2.5, 2, hexc("#1A1816"))
        m.box(119.4, 97.4, 13, 1.6, 1.6, 1.6, "#FFFFFF", hl=False)
        cv.draw(m)
        e = OL(outline=False, emissive=True)
        e.put(*P(121.6, 99.52, 18.5), hexc("#4FE08A"))
        cv.draw(e)
    add(214, coffee)

    # pendant lamp over the meeting table
    def pendant():
        pd = OL(k=-0.6)
        for z in range(45, RH, 2):
            pd.put(*P(112, 58, z), hexc("#3A3632"))
        pd.box(109.5, 55.5, 41, 5, 5, 4, "#2B2826", top=hexc("#3A3734"))
        cv.draw(pd)
        b = OL(outline=False, emissive=not DAY)
        b.face_y(60.52, 110.2, 41, 3.6, 0.8, hexc("#FFE7B0") if not DAY else hexc("#E8DCC4"))
        cv.draw(b)
    add(240, pendant)

    # books stacked on the floor + floor cushion + small cactus by the arch
    def floor_bits():
        bk = OL(k=-0.55)
        for i, col in enumerate(("#5B7FD9", "#E9875A", "#E8C766", "#3E9C8E")):
            bk.box(24 + (i % 2) * 0.4, 4 + (i % 2) * 0.3, i * 1.3, 5, 3.5, 1.3, col, hl=False)
        bk.box(25, 4.6, 5.2, 2.4, 2.4, 2, "#F3EEE7", top=hexc("#FFFFFF"))
        bk.flat(25.4, 5, 7.21, 1.6, 1.6, hexc("#4A2E1A"))
        cv.draw(bk)
        cu = OL(k=-0.55)
        cu.box(7, 88, 0, 6, 6, 2.6, "#D9719B", top=hexc("#F4C2D6"))
        cu.flat(9.5, 88.4, 2.61, 0.5, 5.2, hexc("#C9608A"))
        cv.draw(cu)
        ca = OL(k=-0.5)
        ca.box(84, 6, 0, 3, 3, 2.5, "#C4703A")
        ca.box(84.8, 6.8, 2.5, 1.4, 1.4, 4.5, "#5E8A50", top=hexc("#7FA86B"))
        ca.box(84.2, 7.2, 4.2, 0.8, 0.8, 1.6, "#5E8A50", hl=False)
        ca.box(86.2, 7.2, 5, 0.8, 0.8, 1.4, "#5E8A50", hl=False)
        ca.put(*P(85.5, 7.5, 7.2), hexc("#E66A9A"))
        cv.draw(ca)
    add(30, lambda: None)
    items.append((32, floor_bits, False))

    # succulent on desk 2 + plant on the tea bar
    def desk_bits():
        d = OL(k=-0.5)
        d.box(92, 41, 20, 2.2, 2.2, 1.8, "#F3EEE7")
        d.box(91.8, 40.8, 21.8, 2.6, 2.6, 1.6, "#6E9B5C", hl=False)
        d.box(92.4, 41.4, 23.4, 1.4, 1.4, 1, "#88B474", hl=False)
        cv.draw(d)
    add(96, desk_bits)


    # ================= fun props =================
    def walkway():
        w_ = OL(outline=False)
        w_.floor = True
        for x in range(18, 70, 4):
            w_.flat(x, 102.6, 0.02, 2, 0.6, hexc("#F2C94C"))
        for y in range(86, 101, 4):
            w_.flat(14.4, y, 0.02, 0.6, 2, hexc("#F2C94C"))
        # arrow at the empty desk
        w_.flat(71, 102.2, 0.02, 1.4, 1.4, hexc("#F2C94C"))
        cv.draw(w_)
        m = OL(outline=False)
        m.floor = True
        m.flat(108, 1, 0.02, 12, 5, hexc("#6E4A2E"))
        m.flat(109, 2, 0.03, 10, 3, hexc("#8E6542"))
        for i in range(4):
            m.flat(110 + i * 2.2, 3, 0.04, 1.2, 1, hexc("#F2B45A"))
        cv.draw(m)
    add(-40, walkway)

    def hoop():
        h = OL(k=-0.55)
        h.face_y(0, 92, 44, 10, 7, hexc("#FFFFFF"))
        h.face_y(0.02, 95, 45, 4, 3, hexc("#D9534F"))
        h.face_y(0.03, 95.6, 45.6, 2.8, 1.8, hexc("#FFFFFF"))
        h.box(94.5, 0, 43.5, 5, 3.5, 0.6, "#E9532F", hl=False)
        cv.draw(h)
        n = OL(outline=False)
        for i in range(5):
            for z in range(4):
                n.put(*P(95 + i, 2.5 - z * 0.15, 43 - z), hexc("#F3EEE7"))
        cv.draw(n)
        b = OL(k=-0.55)
        bz = (0, 1.5, 3, 1.5)[t]
        b.box(98, 8, bz, 3.2, 3.2, 3.2, "#E9873A", top=hexc("#F5A55A"))
        b.flat(99.4, 8, bz + 3.21, 0.4, 3.2, hexc("#7A3A10"))
        cv.draw(b)
    add(70, hoop)

    def dartboard():
        d = OL(outline=False)
        for r_, col in ((3.6, "#2B2620"), (3, "#E9DCC0"), (2.2, "#2B2620"), (1.4, "#D9534F"), (0.6, "#3E9C8E")):
            for a in range(36):
                for rr in range(int(r_ * 4)):
                    rad = rr / 4
                    d.put(*P(0.05, 76 + math.cos(a / 36 * math.tau) * rad, 36 + math.sin(a / 36 * math.tau) * rad), hexc(col))
        cv.draw(d)
        dt = OL(k=-0.5)
        dt.box(0, 75.6, 37, 1.6, 0.4, 0.4, "#F2C94C", hl=False)
        cv.draw(dt)
    add(-45, dartboard)

    def arcade():
        a = OL(k=-0.6)
        a.box(40, 70, 0, 7, 6, 21, "#34406B", top=hexc("#46528A"))
        a.face_x(47, 70.5, 2, 5, 17, hexc("#E9875A"))
        a.box(40, 75, 9, 7, 2.4, 1.2, "#2B2620", top=hexc("#3A3734"))
        a.box(41.5, 75.6, 10.2, 0.8, 0.8, 1.4, "#2B2620", hl=False)
        a.box(41.2, 75.3, 11.6, 1.4, 1.4, 1, "#D9534F", hl=False)
        cv.draw(a)
        sc = OL(outline=False, emissive=True)
        sc.face_y(76.02, 40.8, 12, 5.4, 6.6, hexc("#1A1530"))
        cols = ["#FF5CC8", "#5CD6FF", "#7AFFB0", "#F2C94C"]
        for i in range(4):
            sc.face_y(76.04, 41.4 + ((t + i) % 4) * 1.2, 13 + i * 1.4, 1, 0.8, hexc(cols[i]))
        sc.face_y(76.04, 40.8, 18.8, 5.4, 1, hexc("#FF5CC8") if t % 2 else hexc("#F2C94C"))
        sc.box(44.2, 75.8, 10.2, 1, 1, 0.5, cols[t % 4], hl=False)
        cv.draw(sc)
        nt = OL(outline=False, emissive=not DAY)
        for i in range(2):
            ph = (t + i * 2) % 4
            x_, z_ = 46 + i * 2, 24 + ph * 2
            nt.put(*P(x_, 76, z_), hexc("#FF9ED8"))
            nt.put(*P(x_, 76, z_ + 1), hexc("#FF9ED8"))
            nt.put(*P(x_ + 0.6, 76, z_ + 1.6), hexc("#FF9ED8"))
        cv.draw(nt)
    add(123, arcade)

    def vacuum():
        vx = (66, 69, 72, 69)[t]
        v = OL(k=-0.6)
        v.box(vx, 67.5, 0, 4, 4, 1.4, "#3A3D4A", top=hexc("#4E5266"))
        v.flat(vx + 1.2, 68.7, 1.41, 1.6, 1.6, hexc("#2B2E3A"))
        cv.draw(v)
        e = OL(outline=False, emissive=True)
        e.put(*P(vx + 2, 69.5, 1.45), hexc("#4FE08A") if t % 2 else hexc("#7FB3F0"))
        cv.draw(e)
    add(137.5, vacuum)

    def desk_toys():
        d = OL(k=-0.55)
        for (x, y) in ((44, 43), (86, 43)):
            d.box(x, y, 20, 2.2, 2.6, 1.6, "#F2C94C", top=hexc("#FFE08A"))
            d.box(x + 0.3, y + 0.3, 21.6, 1.6, 1.4, 1.4, "#F2C94C", top=hexc("#FFE08A"))
            d.box(x + 0.6, y + 1.6, 22, 1, 0.8, 0.5, "#E9873A", hl=False)
        cv.draw(d)
        # balloon over desk 1 (bobs)
        bz = 40 + (0, 0.8, 1.6, 0.8)[t]
        st_ = OL(outline=False)
        for z in range(21, int(bz)):
            st_.put(*P(51 + math.sin(z / 3) * 0.4, 40, z), hexc("#8C8C8C"))
        cv.draw(st_)
        bl = OL(k=-0.5)
        bl.box(49.4, 38.4, bz, 3.4, 3.4, 4.2, "#D9534F", top=hexc("#F07A6E"))
        bl.box(49.9, 38.9, bz + 4.2, 2.4, 2.4, 0.8, "#D9534F", hl=False)
        bl.put(*P(50, 39.4, bz + 3.6), hexc("#FFFFFF"))
        cv.draw(bl)
    add(97, desk_toys)

    def table_toys():
        tt = OL(k=-0.55)
        tt.box(114, 59.5, 12.4, 5, 3, 0.8, "#F2E9DC", top=hexc("#FFF6E8"))
        tt.flat(114.6, 60, 13.21, 1.6, 1, hexc("#D9534F"))
        tt.box(105.5, 58.5, 12.4, 2, 2, 2.8, "#BFE3FF", top=hexc("#E9F6FF"), left=hexc("#A9D4F2"), right=hexc("#93C2E4"))
        tt.box(105.7, 58.7, 15.2, 1.6, 1.6, 0.6, "#8E6542", hl=False)
        cv.draw(tt)
        bug = OL(outline=False)
        bug.put(*P(106.5, 60.5, 13.6 + (t % 2) * 0.6), hexc("#2B8A3E"))
        bug.put(*P(106.5, 60.5, 14.4 + (t % 2) * 0.6), hexc("#1E6B2E"))
        cv.draw(bug)
    add(171, table_toys)

    def boss_toys():
        bt = OL(k=-0.6)
        bt.box(119, 17.5, 19, 2.6, 2.6, 1, "#F2C94C", top=hexc("#FFE08A"))
        bt.box(119.4, 17.9, 20, 1.8, 1.8, 1.2, "#D9534F", top=hexc("#F07A6E"))
        bt.box(103, 17, 19, 3, 3.4, 1.4, "#8E6542", hl=False)
        bt.box(103.3, 17.3, 20.4, 2.4, 2.8, 0.3, "#FFFFFF", hl=False)
        bt.box(103.3, 17.3, 20.7, 2.4, 2.8, 0.3, "#F3EEE7", hl=False)
        bt.box(113, 18, 19, 1.8, 1.8, 1, "#2B2826", hl=False)
        cv.draw(bt)
        lv = OL(k=-0.5, emissive=not DAY)
        lv.box(113.3, 18.3, 20, 1.2, 1.2, 4, "#B98AFF", top=hexc("#D9C2FF"), hl=False)
        cv.draw(lv)
        bl = OL(outline=False, emissive=True)
        for i in range(2):
            bl.put(*P(113.8, 19.5, 20.6 + ((t + i * 2) % 4) * 0.9), hexc("#FF7AB6"))
        cv.draw(bl)
    add(131, boss_toys)


    # ---------------- outdoor props in front of the house ----------------
    def front_props():
        # construction plot for the next room: scaffolding, planks, cones, sign
        x0, x1, y0, y1 = PLOT
        sc = OL(k=-0.55)
        for (px, py) in ((x0 + 2, y0 + 2), (x1 - 2, y0 + 2), (x0 + 2, y1 - 2), (x1 - 2, y1 - 2)):
            sc.box(px - 0.6, py - 0.6, 0, 1.2, 1.2, 26, "#C9CED6", hl=False)
        sc.box(x0 + 1.4, y0 + 1.4, 12, x1 - x0 - 2.8, 1.2, 1, "#C9CED6", hl=False)
        sc.box(x0 + 1.4, y0 + 1.4, 24, x1 - x0 - 2.8, 1.2, 1, "#C9CED6", hl=False)
        sc.box(x0 + 1.4, y0 + 1.4, 12, 1.2, y1 - y0 - 2.8, 1, "#C9CED6", hl=False)
        sc.box(x0 + 1.4, y0 + 1.4, 24, 1.2, y1 - y0 - 2.8, 1, "#C9CED6", hl=False)
        sc.box(x0 + 6, y0 + 16, 0, 12, 4, 1.2, "#C99A6B")
        sc.box(x0 + 6.5, y0 + 16.5, 1.2, 11, 3, 1.2, "#B98A5E")
        sc.box(x0 + 18, y0 + 6, 0, 5, 5, 5, "#9C8670", top=hexc("#B5A58E"))
        for (cx_, cy_) in ((x0 + 4, y1 + 2), (x0 + 14, y1 + 2), (x0 + 24, y1 + 2)):
            sc.box(cx_, cy_, 0, 3, 3, 1, "#E9875A")
            sc.box(cx_ + 0.7, cy_ + 0.7, 1, 1.6, 1.6, 4, "#F28A3C", top=hexc("#FFFFFF"))
        cv.draw(sc)
        sg = OL(k=-0.55)
        sg.box(x0 + 4, y1 - 1, 0, 0.8, 0.8, 12, "#6E4A2E", hl=False)
        sg.box(x0 + 12, y1 - 1, 0, 0.8, 0.8, 12, "#6E4A2E", hl=False)
        sg.box(x0 + 3, y1 - 1.2, 9, 11, 1, 6, "#34406B", left=hexc("#34406B"))
        sg.face_y(y1 - 0.18, x0 + 4.2, 11.6, 8.6, 1.2, hexc("#F2B45A"))
        sg.face_y(y1 - 0.18, x0 + 4.2, 13.4, 5, 0.8, hexc("#FFFFFF"))
        cv.draw(sg)
        # bench, mailbox, lamp posts, bushes, fences, trees
        bn = OL(k=-0.55)
        bn.box(36, RY + 16, 0, 1, 6, 4, "#3A3632", hl=False)
        bn.box(46, RY + 16, 0, 1, 6, 4, "#3A3632", hl=False)
        bn.box(35, RY + 15, 4, 13, 7, 1.2, "#C99A6B", top=hexc("#D9B48A"))
        bn.box(35, RY + 21, 5.2, 13, 1, 6, "#B98A5E")
        cv.draw(bn)
        mb = OL(k=-0.55)
        mb.box(30.5, RY + 6.5, 0, 0.8, 0.8, 10, "#6E4A2E", hl=False)
        mb.box(29, RY + 5, 10, 4, 4, 4, "#D9534F", top=hexc("#F07A6E"))
        mb.face_y(RY + 9.02, 30, 11.5, 2, 0.6, hexc("#FFFFFF"))
        cv.draw(mb)
        lamp_post(RX + 8, RY + 8)
        lamp_post(10, RY + 12)
        lamp_post(RX + 8, -2)
        for (bx, by, fc) in ((RX + 9, 30, "#E66A9A"), (RX + 9, 50, None), (60, RY + 14, "#F2C94C"), (100, RY + 15, None), (RX + 26, RY + 26, "#FFFFFF")):
            bush(bx, by, 1.1, fc)
        fence(4, RY + 34, 120, RY + 34)
        fence(RX + 58, 0, RX + 58, 120)
        tree(RX + 30, RY + 14, 1.2, 1)
        tree(RX + 52, 100, 1.1, 0)
        tree(RX + 50, -20, 1.3, 0)
        tree(-20, RY + 26, 1.2, 0)
        tree(76, RY + 28, 1.0, 0)
        # lily pads + duck on the pond
        lp_ = OL(outline=False)
        for (lx, ly) in ((POND[0] - 6, POND[1] - 2), (POND[0] + 5, POND[1] + 3)):
            lp_.flat(lx, ly, 0.05, 2.4, 2, C("#2F5A30", "#5E8A50"))
        dx_ = (0, 0.6, 1.2, 0.6)[t]
        lp_.box(POND[0] - 1 + dx_, POND[1] - 3, 0, 2.6, 1.8, 1.6, "#FFFFFF", top=hexc("#FFFFFF"), hl=False)
        lp_.box(POND[0] - 1.6 + dx_, POND[1] - 3, 1.4, 1, 1, 1, "#FFFFFF", hl=False)
        lp_.put(*P(POND[0] - 2 + dx_, POND[1] - 2.6, 2.1), hexc("#F2B45A"))
        cv.draw(lp_)
        if not DAY:
            ff = OL(outline=False, emissive=True)
            for i in range(9):
                fx_ = (h32(i, 3) % 200) - 40
                fy_ = RY + 8 + (h32(i, 5) % 30)
                if i % 2:
                    fx_, fy_ = RX + 8 + (h32(i, 7) % 40), (h32(i, 9) % 120) - 10
                if (i + t) % 3:
                    ff.put(*P(fx_ + (t % 2) * 0.6, fy_, 6 + (i + t) % 4), hexc("#F7F2A0"))
            cv.draw(ff)
    add(400, front_props)

    items.sort(key=lambda it: it[0])
    drawn = []
    for k, fn, robot in items:
        if robot and NO_ROBOTS:
            continue
        cv.current = len(drawn)
        drawn.append(k)
        fn()
        cv.current = None
    cv.item_keys = drawn

    # ---------------- lighting ----------------
    lights = []

    def L_(x, y, z, r, inten, col):
        sx, sy = P(x, y, z)
        lights.append((sx, sy, r, inten, col))

    if DAY:
        L_(0, 42, 30, 120, 0.2, (255, 236, 200))
        L_(0, 66, 30, 120, 0.2, (255, 236, 200))
        amb = 0.9
    else:
        for (x0, y0) in ((14, 38), (56, 38)):
            L_(x0 + 13, y0 + 9, 22, 70, 0.5, (110, 160, 255))
            L_(x0 + 28, y0 + 9, 22, 70, 0.45, (110, 160, 255))
        L_(19, 50, 26, 62, 0.7, (255, 190, 120))
        L_(0, 42, 30, 80, 0.3, (120, 150, 255))
        L_(0, 66, 30, 80, 0.3, (120, 150, 255))
        L_(35, 0, 44, 50, 0.45 if t != 2 else 0.15, (255, 110, 190))
        L_(2, 89, 16, 60, 0.45, (150, 170, 255))
        L_(7, 72, 28, 70, 0.6, (255, 200, 140))
        L_(35, 0, 28, 40, 0.3, (110, 160, 255))
        L_(8, 52, 9, 30, 0.35, tuple(PAL[t % 4]))
        L_(56, 10, 20, 60, 0.3, (120, 255, 170))
        L_(110, 14, 26, 70, 0.4, (255, 220, 170))
        L_(112, 96, 24, 60, 0.4, (255, 200, 150))
        L_(110, 70, 30, 120, 0.2, (255, 225, 190))
        L_(3, 66, 14, 40, 0.4, (90, 170, 255))
        L_(43, 76, 15, 40, 0.45, (255, 110, 210))
        L_(114, 19, 22, 26, 0.3, (190, 140, 255))
        L_(112, 58, 34, 60, 0.55, (255, 210, 150))
        L_(0, 40, 50, 70, 0.18, (255, 210, 140))
        L_(60, 0, 50, 70, 0.18, (255, 210, 140))
        L_(RX + 8, RY + 8, 24, 90, 0.6, (255, 220, 150))
        L_(10, RY + 12, 24, 90, 0.6, (255, 220, 150))
        L_(RX + 8, -2, 24, 80, 0.5, (255, 220, 150))
        amb = 0.62
    out = [[None] * W for _ in range(H)]
    for sy in range(H):
        for sx in range(W):
            c = cv.px[sy][sx]
            if c is None:
                continue
            if (sx, sy) in cv.emit:
                out[sy][sx] = c
                continue
            lv = amb
            tint = [0.0, 0.0, 0.0]
            for (lx, ly, r, inten, col) in lights:
                dx, dy = sx - lx, (sy - ly) * 1.5
                d = math.hypot(dx, dy) / r
                if d < 1:
                    k = inten * (1 - d) ** 1.6
                    lv += k
                    tint[0] += col[0] * k * 0.35
                    tint[1] += col[1] * k * 0.35
                    tint[2] += col[2] * k * 0.35
            if DAY and (sx, sy) in cv.floor:
                fx_, fy_ = inv(sx, sy)
                for wy in (32, 56):
                    if 0 <= fx_ <= 34 and wy + fx_ * 0.55 <= fy_ <= wy + 19 + fx_ * 0.55:
                        lv += 0.28 * (1 - fx_ / 40)
                        tint[0] += 40
                        tint[1] += 30
                        tint[2] += 5
            vx_, vy_ = (sx - W / 2) / (W / 2), (sy - H / 2) / (H / 2)
            vr = math.hypot(vx_, vy_)
            if vr > 0.7:
                lv *= 1 - 0.22 * min(1, (vr - 0.7) / 0.6)
            step_ = 0.07
            bias = (BAYER[sy % 2][sx % 2] / 4 - 0.375) * step_
            lq = min(math.floor((lv + bias) / step_) * step_, 1.25)
            out[sy][sx] = tuple(max(0, min(255, round(c[i] * lq + tint[i] * 0.45))) for i in range(3))
    cv.px = out
    return cv, bubbles


def main():
    tag = ("room_day" if DAY else "room_night") + "_" + VARIANT
    frames = []
    bubbles = None
    for f in range(4):
        cv, b = scene(f)
        frames.append(cv)
        bubbles = bubbles or b
    sheet = []
    for y in range(H):
        row = []
        for c in frames:
            row.extend(c.px[y])
        sheet.append(row)
    ps.write_png(os.path.join(HERE, tag + "_sheet.png"), W * 4, H, sheet)
    S = 2
    big = []
    bg = (246, 239, 228) if DAY else (40, 42, 52)
    for y in range(H):
        r = []
        for c in frames[0].px[y]:
            r.extend([c or bg] * S)
        for _ in range(S):
            big.append(r)
    ps.write_png(os.path.join(HERE, tag + "_x2.png"), W * S, H * S, big)
    json.dump({"W": W, "H": H, "bubbles": bubbles}, open(os.path.join(HERE, tag + ".json"), "w"), ensure_ascii=False)
    print("ok", W, H)


if __name__ == "__main__":
    main()
