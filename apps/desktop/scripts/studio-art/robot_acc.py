"""Accessories an owner can give an agent (Agent 档案 · 配件), drawn on any robot view.

Called by room.robot_front / robot_back / robot_side with the head box in the robot's own layer
frame and the direction the face points. Only profile accessories (prefixed "p:") are drawn here;
the room's decorative props (scarf, glasses, phones, pad, flower) stay in room.py.
"""
import room

hexc, lit, OL = room.hexc, room.lit, room.OL

PROFILE = ("cap", "headphones", "glasses", "bowtie", "antenna", "scarf")


def draw(cv, ox, oy, head, facing, acc, frame=0):
    """head = (hx, hy, hz, w, d, h); facing in {"+y", "-y", "-x"}."""
    wanted = [a[2:] for a in acc if a.startswith("p:")]
    if not wanted:
        return
    hx, hy, hz, w, d, h = head
    top = hz + h
    for name in wanted:
        L = OL(ox, oy, k=-0.55)
        if name == "cap":
            L.box(hx + 0.4, hy + 0.4, top, w - 0.8, d - 0.8, 2, "#D9534F", top=hexc("#F07A6E"))
            if facing == "+y":
                L.box(hx + 1, hy + d - 0.4, top, w - 2, 3.4, 0.7, "#B23A36", hl=False)
            elif facing == "-y":
                L.box(hx + 1, hy - 3, top, w - 2, 3.4, 0.7, "#B23A36", hl=False)
            else:
                L.box(hx - 3, hy + 1, top, 3.4, d - 2, 0.7, "#B23A36", hl=False)
            L.box(hx + w / 2 - 0.6, hy + d / 2 - 0.6, top + 2, 1.2, 1.2, 0.6, "#F3EEE7", hl=False)
        elif name == "headphones":
            band, cup = "#2B2826", "#5B7FD9"
            if facing == "-x":
                L.box(hx + w / 2 - 0.7, hy - 0.8, top, 1.4, d + 1.6, 0.9, band, hl=False)
                L.box(hx + w / 2 - 2, hy - 1.8, hz + h * 0.3, 4, 1.8, 4.4, cup, top=lit(hexc(cup), 0.3))
                L.box(hx + w / 2 - 2, hy + d, hz + h * 0.3, 4, 1.8, 4.4, cup, top=lit(hexc(cup), 0.3))
            else:
                L.box(hx - 0.8, hy + d / 2 - 0.7, top, w + 1.6, 1.4, 0.9, band, hl=False)
                L.box(hx - 1.8, hy + d / 2 - 2, hz + h * 0.3, 1.8, 4, 4.4, cup, top=lit(hexc(cup), 0.3))
                L.box(hx + w, hy + d / 2 - 2, hz + h * 0.3, 1.8, 4, 4.4, cup, top=lit(hexc(cup), 0.3))
        elif name == "glasses":
            g = OL(ox, oy, outline=False)
            ink = hexc("#2B2620")
            if facing == "+y":
                fy = hy + d + 0.1
                g.face_y(fy, hx + 1.6, hz + h * 0.62, w - 3.2, 0.6, ink)
                g.face_y(fy, hx + 2.2, hz + h * 0.22, 2.6, 0.5, ink)
                g.face_y(fy, hx + w - 4.8, hz + h * 0.22, 2.6, 0.5, ink)
                g.face_y(fy, hx + 2.2, hz + h * 0.22, 0.5, h * 0.42, ink)
                g.face_y(fy, hx + w - 2.7, hz + h * 0.22, 0.5, h * 0.42, ink)
            elif facing == "-x":
                g.face_y(hy + d + 0.08, hx - 0.2, hz + h * 0.55, 3.4, 0.6, ink)
            else:
                g.face_x(hx + w + 0.08, hy + d * 0.25, hz + h * 0.55, d * 0.6, 0.5, ink)
            cv.draw(g)
            continue
        elif name == "bowtie":
            if facing != "+y":
                continue
            cx = hx + w / 2
            L.box(cx - 3, hy + d - 1.2, hz - 1.6, 2.4, 0.8, 2, "#D9534F", hl=False)
            L.box(cx + 0.6, hy + d - 1.2, hz - 1.6, 2.4, 0.8, 2, "#D9534F", hl=False)
            L.box(cx - 0.7, hy + d - 1, hz - 1.3, 1.4, 0.8, 1.4, "#8E2A26", hl=False)
        elif name == "antenna":
            ax, ay = hx + w * 0.22, hy + d * 0.5
            L.box(ax, ay, top, 0.6, 0.6, 2.2, "#2B2826", hl=False)
            L.box(ax + 0.4, ay, top + 2.2, 0.6, 0.6, 2.2, "#2B2826", hl=False)
            L.box(ax, ay, top + 4.4, 0.6, 0.6, 1.8, "#2B2826", hl=False)
            tip = OL(ox, oy, k=-0.4, emissive=frame % 2 == 1)
            tip.box(ax - 0.9, ay - 0.9, top + 6, 2.4, 2.4, 2.4, "#F2C94C", top=hexc("#FFE08A"), hl=False)
            cv.draw(L)
            cv.draw(tip)
            continue
        elif name == "scarf":
            L.box(hx + 0.6, hy + 0.6, hz - 1.4, w - 1.2, d - 1.2, 1.5, "#2A9D8F", top=hexc("#4FC1B2"), hl=False)
            if facing == "+y":
                L.box(hx + w * 0.62, hy + d - 1, hz - 4.4, 1.8, 0.8, 3.2, "#2A9D8F", hl=False)
            elif facing == "-x":
                L.box(hx - 0.2, hy + d * 0.6, hz - 4.4, 0.8, 1.8, 3.2, "#2A9D8F", hl=False)
        cv.draw(L)
