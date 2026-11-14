"""Generate the synthetic template used by the engine tests.

This is NOT an iRacing template. It mimics the layer structure of one (names, groups,
colour keys) on a 512x512 canvas so the engine can be tested without redistributing
iRacing's files. It also writes `expected.json`, computed with NumPy/SciPy using the
same algorithms as the proof-of-concept scripts, as an independent reference.

    pip install psd-tools pillow numpy scipy
    python make_fixture.py
"""
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from psd_tools import PSDImage
from scipy import ndimage

W = H = 512
HERE = Path(__file__).parent

# Paintable islands: (name, shape, box)
ISLANDS = [
    ("side_right", "rect", (20, 20, 300, 130)),
    ("side_left", "rect", (20, 380, 300, 490)),
    ("roof", "ellipse", (330, 150, 490, 290)),
    ("hood", "rect", (40, 170, 250, 330)),
    ("wing", "rect", (330, 30, 490, 100)),
    ("mirror", "ellipse", (360, 360, 440, 440)),
]


def island_mask() -> Image.Image:
    im = Image.new("L", (W, H), 0)
    d = ImageDraw.Draw(im)
    for _, shape, box in ISLANDS:
        (d.rectangle if shape == "rect" else d.ellipse)(box, fill=255)
    return im


def rgba(colour, alpha_l: Image.Image) -> Image.Image:
    im = Image.new("RGBA", (W, H), tuple(colour) + (0,))
    im.putalpha(alpha_l)
    return im


# Copied unchanged from the proof-of-concept (margic/sandbox, render_calibration_livery.py).
def segment_islands(paintable: np.ndarray, wire_rgba: np.ndarray) -> tuple[np.ndarray, int]:
    r, g, b, a = (
        wire_rgba[:, :, 0].astype(int),
        wire_rgba[:, :, 1].astype(int),
        wire_rgba[:, :, 2].astype(int),
        wire_rgba[:, :, 3].astype(int),
    )
    green = (g - np.maximum(r, b) > 15) & (a > 20)
    green_closed = ndimage.binary_closing(green, structure=np.ones((7, 7)))

    labels_w, nw = ndimage.label(~green_closed)
    bg_labels = {labels_w[0, 0], labels_w[0, -1], labels_w[-1, 0], labels_w[-1, -1]}
    areas_w = ndimage.sum(~green_closed, labels_w, range(1, nw + 1))

    seeds = np.zeros_like(labels_w)
    next_id = 1
    for i in range(1, nw + 1):
        if i not in bg_labels and areas_w[i - 1] >= 1200:
            if (paintable & (labels_w == i)).sum() >= 1000:
                seeds[labels_w == i] = next_id
                next_id += 1

    rem = paintable & (~green_closed) & (seeds == 0)
    dist_rem = ndimage.distance_transform_edt(rem)
    labels_r, nr = ndimage.label(dist_rem >= 22)
    areas_r = ndimage.sum(dist_rem >= 22, labels_r, range(1, nr + 1))
    for i in range(1, nr + 1):
        if areas_r[i - 1] >= 400:
            seeds[labels_r == i] = next_id
            next_id += 1

    barrier = paintable & (~green_closed)
    grown = seeds.copy()
    for _ in range(40):
        unlabelled = barrier & (grown == 0)
        if not unlabelled.any():
            break
        dil = ndimage.grey_dilation(grown, size=(3, 3))
        grown[unlabelled & (dil > 0)] = dil[unlabelled & (dil > 0)]

    _, idx = ndimage.distance_transform_edt(grown == 0, return_indices=True)
    raw_islands = np.where(paintable, grown[idx[0], idx[1]], 0)
    n_isl = next_id - 1
    areas = ndimage.sum(paintable, raw_islands, range(1, n_isl + 1))
    order = [i for i in np.argsort(areas)[::-1] if areas[i] >= 1200]

    remap = np.zeros(n_isl + 1, dtype=int)
    for rank, old_idx in enumerate(order, 1):
        remap[old_idx + 1] = rank
    return remap[raw_islands], len(order)


def main() -> None:
    paint = island_mask()
    paint_np = np.array(paint) > 0

    # Mask: grey where NOT paintable, transparent inside the islands.
    mask = rgba((128, 128, 128), Image.fromarray(((~paint_np) * 255).astype(np.uint8)))

    # Wire: green island outlines plus a grey mesh.
    wire = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(wire)
    for x in range(0, W, 32):
        d.line([(x, 0), (x, H)], fill=(90, 90, 90, 120), width=1)
    for _, shape, box in ISLANDS:
        (d.rectangle if shape == "rect" else d.ellipse)(box, outline=(0, 255, 0, 255), width=2)
    # A green panel split inside the hood, so one UV island holds two panels.
    d.line([(145, 170), (145, 330)], fill=(0, 255, 0, 255), width=2)

    # Number blocks, mandatory, sponsor blocks, sample decal.
    nb = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(nb)
    d.rectangle((60, 50, 110, 100), fill=(255, 255, 255, 255))
    d.rectangle((60, 410, 110, 460), fill=(255, 255, 255, 255))
    d.rectangle((70, 220, 110, 270), fill=(255, 255, 255, 255))

    mand = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(mand)
    d.rectangle((250, 40, 290, 70), fill=(200, 30, 30, 255))
    d.rectangle((250, 440, 290, 470), fill=(200, 30, 30, 255))

    spons = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(spons).rectangle((340, 40, 380, 60), fill=(255, 0, 255, 255))

    decal = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(decal).rectangle((380, 200, 440, 240), fill=(240, 240, 240, 255))

    # Patterns: pure R / G / B colour keys. Pattern 000 is one slot; 001 has three bands
    # with an anti-aliased (blended) transition between red and green.
    p0 = Image.new("RGBA", (W, H), (255, 0, 0, 255))
    p1 = np.zeros((H, W, 4), dtype=np.uint8)
    p1[..., 3] = 255
    ys = np.arange(H)[:, None] * np.ones((1, W))
    p1[..., 0] = np.clip((200 - ys) * 255 / 8 + 127, 0, 255)
    p1[..., 1] = np.where(ys < 400, 255 - p1[..., 0], 0)
    p1[..., 2] = np.where(ys >= 400, 255, 0)
    p1[..., 0] = np.where(ys >= 400, 0, p1[..., 0])
    p1_im = Image.fromarray(p1)

    def grey(v, alpha=None):
        a = alpha if alpha is not None else Image.new("L", (W, H), 255)
        return rgba((v, v, v), a)

    parts = Image.new("L", (W, H), 0)
    ImageDraw.Draw(parts).rectangle((440, 380, 500, 500), fill=255)
    ImageDraw.Draw(parts).rectangle((180, 240, 200, 260), fill=255)  # hardware inside the hood

    psd = PSDImage.new("RGBA", (W, H))

    def add(parent, name, im, visible=True):
        bbox = im.getbbox() or (0, 0, 1, 1)
        layer = psd.create_pixel_layer(im.crop(bbox), name=name, top=bbox[1], left=bbox[0])
        layer.visible = visible
        parent.append(layer)
        return layer

    spec = psd.create_group(name="Custom Spec Map")
    for gname, base, part in (
        ("Blue Channel Clearcoat", 0, None),
        ("Green Channel Roughness", 51, 140),
        ("Red Channel Metallic", 0, 200),
    ):
        g = psd.create_group(name=gname)
        spec.append(g)
        add(g, "Base Paint", grey(base))
        if part is not None:
            add(g, "Parts", grey(part, parts))

    paintable = psd.create_group(name="Paintable Area")
    add(paintable, "Main Car Body", Image.new("RGBA", (W, H), (62, 137, 205, 255)))
    pats = psd.create_group(name="Car Patterns")
    paintable.append(pats)
    add(pats, "car_pattern_000.tga", p0, visible=False)
    add(pats, "car_pattern_001.tga", p1_im, visible=False)
    add(paintable, "Car_decal", decal)

    off = psd.create_group(name="Turn Off Before Exporting TGA")
    add(off, "Car_Mandatory", mand)
    add(off, "Number Blocks", nb)
    add(off, "Sponsor Blocks", spons)
    add(off, "Mask", mask)
    add(off, "Wire", wire)

    psd.save(HERE / "synthetic_template.psd")

    # ---- independent reference values (same maths as the proof-of-concept scripts) ----
    back = PSDImage.open(HERE / "synthetic_template.psd")

    def full(name):
        for layer in back.descendants():
            if layer.name == name:
                canvas = Image.new("RGBA", (W, H), (0, 0, 0, 0))
                im = layer.topil()
                if im is not None:
                    canvas.paste(im, (layer.left, layer.top))
                return np.array(canvas)
        raise KeyError(name)

    unpaintable = full("Mask")[:, :, 3] > 0
    p = full("car_pattern_001.tga").astype(float)
    r, g, b = p[:, :, 0] / 255.0, p[:, :, 1] / 255.0, p[:, :, 2] / 255.0
    keys = [(r > 0.78) & (g < 0.25) & (b < 0.25), (g > 0.78) & (r < 0.25) & (b < 0.25), (b > 0.78) & (r < 0.25) & (g < 0.25)]
    areas = [int(k[~unpaintable].sum()) for k in keys]
    s = np.clip(r + g + b, 1e-6, None)
    cols = [(5, 7, 10), (255, 95, 31), (14, 165, 233)]
    order = list(np.argsort(areas)[::-1])
    slot_cols = [None] * 3
    for rank, slot in enumerate(order):
        slot_cols[slot] = cols[rank]
    out = np.zeros((H, W, 3))
    for wgt, c in zip((r / s, g / s, b / s), slot_cols):
        out += wgt[:, :, None] * np.array(c)[None, None, :]
    diffuse = np.clip(out, 0, 255).astype(np.uint8)

    def boxes(mask):
        lab, _ = ndimage.label(mask)
        return [[int(sl[1].start), int(sl[0].start), int(sl[1].stop), int(sl[0].stop)] for sl in ndimage.find_objects(lab)]

    expected = {
        "canvas": [W, H],
        "paintable_px": int((~unpaintable).sum()),
        "pattern_001_slot_areas_px": areas,
        "pattern_001_recolour_sha256": hashlib.sha256(diffuse.tobytes()).hexdigest(),
        "number_block_boxes": boxes(full("Number Blocks")[:, :, 3] > 0),
        "mandatory_boxes": boxes(full("Car_Mandatory")[:, :, 3] > 0),
        "parts_px": int((np.array(parts) > 0).sum()),
    }
    islands, n_islands = segment_islands(~unpaintable, full("Wire"))
    objs = ndimage.find_objects(islands)
    expected["islands"] = [
        {
            "num": i,
            "area_px": int((islands == i).sum()),
            "bbox": [int(objs[i - 1][1].start), int(objs[i - 1][0].start), int(objs[i - 1][1].stop), int(objs[i - 1][0].stop)],
        }
        for i in range(1, n_islands + 1)
    ]
    (HERE / "expected.json").write_text(json.dumps(expected, indent=2) + "\n")
    print(json.dumps(expected, indent=2))


if __name__ == "__main__":
    main()
