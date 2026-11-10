"""Yard decorations the owner can put in front of a project's house (block-local coordinates).

Each item has its own fixed spot on the project lot, clear of the path, sign, bench, mailbox and
the slots robots wander between, so any combination of items fits.
"""
import math

import town_blocks as tb

room, OL, P, hexc, lit, C = tb.room, tb.OL, tb.P, tb.hexc, tb.lit, tb.C


def fountain(cv, f):
    x, y, r = 74, 59, 3.4
    b = OL(k=-0.5)
    for k in range(10):
        a = k / 10 * math.tau
        b.box(x + math.cos(a) * r - 0.7, y + math.sin(a) * r - 0.7, 0, 1.4, 1.4, 1.6, "#C9C2B6", hl=False)
    cv.draw(b)
    w = OL(outline=False)
    for yy in range(-3, 4):
        for xx in range(-3, 4):
            if xx * xx + yy * yy <= 7:
                w.put(*P(x + xx, y + yy, 1.2), C("#3B5B8A", "#7FC3E8") if (xx + yy + f) % 3 else C("#5B7FAA", "#BFE3FF"))
    cv.draw(w)
    jet = OL(k=-0.4)
    jet.box(x - 0.4, y - 0.4, 1.2, 0.8, 0.8, 3 + f, "#BFE3FF", hl=False)
    cv.draw(jet)


def flag(cv, f):
    x, y = 80.5, 56
    p = OL(k=-0.55)
    p.box(x, y, 0, 1.2, 1.2, 1, "#9C9690", hl=False)
    p.box(x + 0.2, y + 0.2, 1, 0.7, 0.7, 20, "#E3E6EC", hl=False)
    cv.draw(p)
    fl = OL(k=-0.5)
    wave = (0, 0.6)[f]
    fl.box(x + 0.6, y + 0.4, 15.5 + wave, 0.3, 6, 3.6, "#D9534F", hl=False)
    fl.box(x + 0.6, y + 0.4, 15.5 + wave, 0.3, 2, 3.6, "#F2C94C", hl=False)
    cv.draw(fl)


def statue(cv, f):
    x, y = 78, 69
    s = OL(k=-0.55)
    s.box(x, y, 0, 5, 5, 3, "#C9C2B6", top=hexc("#E3DCD0"))
    s.box(x + 1, y + 1, 3, 3, 3, 5, "#D9D2C6", top=hexc("#EFEAE2"))
    s.box(x + 0.6, y + 0.6, 8, 3.8, 3.8, 3.4, "#D9D2C6", top=hexc("#F3EEE7"))
    s.box(x + 2.1, y + 2.1, 11.4, 0.8, 0.8, 2.4, "#C9C2B6", hl=False)
    cv.draw(s)


def garden(cv, f):
    x, y = 72, 40
    g = OL(k=-0.45)
    g.box(x, y, 0, 9, 6, 1.2, "#7A5536", top=C("#3A2C22", "#6E4A2E"))
    cv.draw(g)
    v = OL(outline=False)
    for i in range(3):
        for j in range(2):
            cx_, cy_ = x + 1.5 + i * 3, y + 1.5 + j * 3
            v.put(*P(cx_, cy_, 1.6 + (i + j + f) % 2 * 0.3), hexc("#5E8A50"))
            v.put(*P(cx_ + 0.6, cy_, 2.2), hexc("#7FB069"))
            if (i + j) % 2 == 0:
                v.put(*P(cx_, cy_ + 0.6, 2.6), hexc("#E9873A"))
    cv.draw(v)


def bench(cv, f):
    tb.bench(cv, 17, 66, facing="x")


def pet(cv, f):
    """A small dog by the gate; wags its tail."""
    x, y = 52, 79
    d = OL(k=-0.55)
    d.box(x, y, 1.2, 4, 2, 2.2, "#C99A6B", top=hexc("#D9B48A"))
    for (lx, ly) in ((0.2, 0.2), (3, 0.2), (0.2, 1.4), (3, 1.4)):
        d.box(x + lx, y + ly, 0, 0.6, 0.5, 1.2, "#8E6542", hl=False)
    d.box(x + 3.4, y - 0.2, 2.4, 2.2, 2.4, 2.2, "#C99A6B", top=hexc("#D9B48A"))
    d.box(x + 4.2, y - 0.4, 4.2, 0.6, 0.6, 0.9, "#6E4A2E", hl=False)
    d.box(x + 3.8, y + 1.8, 4.2, 0.6, 0.6, 0.9, "#6E4A2E", hl=False)
    d.box(x - 0.6, y + 0.8 + (-0.4, 0.4)[f], 2.8, 0.8, 0.5, 1.2, "#C99A6B", hl=False)
    cv.draw(d)


YARD_ITEMS = {
    "fountain": fountain,
    "flag": flag,
    "statue": statue,
    "garden": garden,
    "bench": bench,
    "pet": pet,
}
