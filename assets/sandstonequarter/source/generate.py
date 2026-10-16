#!/usr/bin/env python3
"""Editable metre-scale Sandstone Quarter brush authoring. Bake with import-map.py.

Plan coordinates: x east 0..72, y north 0..112, z up, metres.
"""
from pathlib import Path
import math
import numpy as np
from PIL import Image, ImageDraw
import json
import random

SOURCE = Path(__file__).resolve().parent
MAP = SOURCE.parent / 'sandstonequarter.map'
BRUSHES, ENTITIES, VISUAL = [], [], []
CUBE = [(0,1,2,3),(4,7,6,5),(0,4,5,1),(1,5,6,2),(2,6,7,3),(3,7,4,0)]
W, L = 72, 112
ROOF = 3.4      # Above a perfect double jump (3.01 m); stairs or jump+dash reach it.
CEIL = 3.1      # Underside of walkable roofs over passages.
PIT = -2.4      # Step-well floor: a double jump (3.01 m) climbs out anywhere.
SKY = 30.0
# Texture world scale (texture pixels per Quake unit). Large rasters use a finer scale.
SCALES = {'sandstone/plaster': .125, 'sandstone/plaster_ochre': .3, 'sandstone/plaster_rose': .3,
          'sandstone/clock': .26, 'sandstone/kashi': .2, 'sandstone/lantern': .1, 'sandstone/sign': .15}
# Visual-only surfaces: no player or projectile collision.
NONSOLID = ('wood_decor', 'window', 'cloth_red', 'cloth_teal', 'cloth_yellow', 'water', 'jali', 'brass', 'lantern', 'eave')


def brush(vertices, faces, material, detail=True):
    visual = material.endswith('_d') or material.split('/')[-1] in NONSOLID
    material = material.removesuffix('_d')
    vertices = np.asarray(vertices, dtype=float) * 40
    center = vertices.mean(axis=0)
    lines = ['{']
    for face in faces:
        a,b,c = (vertices[i] for i in face[:3])
        n = np.cross(c-a,b-a)
        if np.dot(n,a-center) < 0: b,c,n = c,b,-n
        axis = np.argmax(abs(n))
        uv = ('[ 0 1 0 0 ] [ 0 0 -1 0 ]','[ 1 0 0 0 ] [ 0 0 -1 0 ]','[ 1 0 0 0 ] [ 0 -1 0 0 ]')[axis]
        points = ' '.join('( '+' '.join(f'{v:.4f}' for v in p)+' )' for p in (a,b,c))
        scale = SCALES.get(material, .3)
        lines.append(f'{points} {material} {uv} 0 {scale} {scale} {134217728 if detail else 0} 0 0')
    # Visual-only brushes go to a func_illusionary: baked and rendered, never collided.
    (VISUAL if visual else BRUSHES).append('\n'.join([*lines,'}']))


def P(x, y, z=0.):
    """Plan metres -> Quake metres (map centred on the origin)."""
    return (x - W/2, y - L/2, z)


def material(name):
    return name if '/' in name else 'sandstone/' + name


def box(x0, y0, x1, y1, z0, z1, mat, detail=True):
    if x1 - x0 < 1e-3 or y1 - y0 < 1e-3 or z1 - z0 < 1e-3:
        return
    a, b = P(x0, y0, z0), P(x1, y1, z1)
    brush([(a[0],a[1],a[2]),(b[0],a[1],a[2]),(b[0],b[1],a[2]),(a[0],b[1],a[2]),
           (a[0],a[1],b[2]),(b[0],a[1],b[2]),(b[0],b[1],b[2]),(a[0],b[1],b[2])], CUBE, material(mat), detail)


def clip(x0, y0, x1, y1, z0, z1=SKY):
    box(x0, y0, x1, y1, z0, z1, 'common/clip')


def prism(x, y, z, r, h, mat, sides=12, top=None):
    top = r if top is None else top
    cx, cy, _ = P(x, y)
    vertices = [(cx+rr*math.cos(i*math.tau/sides), cy+rr*math.sin(i*math.tau/sides), zz)
                for zz, rr in ((z, r), (z+h, top)) for i in range(sides)]
    faces = [tuple(range(sides)), tuple(range(sides, 2*sides))]
    faces += [(i, (i+1) % sides, (i+1) % sides+sides, i+sides) for i in range(sides)]
    brush(vertices, faces, material(mat))


def stair(bottom, top, width, walls=(-1, 1), mat='stone'):
    """Straight solid flight; every step is filled down to the base (no hanging treads).

    walls: sides (+1 left of the climbing direction, -1 right) that get a low wall.
    Sides against a building facade get none.
    """
    a, b = np.array(P(*bottom), dtype=float), np.array(P(*top), dtype=float)
    delta = b - a; run = np.linalg.norm(delta[:2]); direction = delta[:2] / run
    side = np.array([-direction[1], direction[0]])
    count = math.ceil(abs(delta[2]) / .18)
    tread = run / count
    assert width >= 4 and .24 <= tread <= .5, (bottom, top, width, tread)
    base = min(a[2], b[2]) - .12
    for i in range(count):
        lo = a[:2] + direction*i*tread; hi = lo + direction*tread
        z = a[2] + delta[2]*(i+1)/count
        pts = [lo-side*width/2, lo+side*width/2, hi+side*width/2, hi-side*width/2]
        brush([(x, y, zz) for zz in (base, z) for x, y in pts], CUBE, material(mat))
    # The wall's inner face stays outside the full walking width.
    for sign in walls:
        q0 = a[:2] + side*sign*(width/2+.1); q1 = b[:2] + side*sign*(width/2+.1)
        edges = [q0-side*.1, q0+side*.1, q1+side*.1, q1-side*.1]
        verts = [(x, y, base) for x, y in edges]
        verts += [(x, y, z+.55) for (x, y), z in zip(edges, [a[2], a[2], b[2], b[2]])]
        brush(verts, CUBE, material('brick'))


def entity(classname, pos=None, **props):
    values = dict(classname=classname, **props)
    if pos is not None:
        values['origin'] = ' '.join(f'{v*40:.4f}' for v in pos)
    ENTITIES.append('{\n'+'\n'.join(f'"{k}" "{v}"' for k, v in values.items())+'\n}')


def parapet(x0, y0, x1, y1, z, openings=(), h=.8, mat='brick', inward=1):
    """Roof-edge wall along an axis-aligned segment, minus walk-through openings.

    The 0.3 m wall stands on the roof side of the edge (inward = +1 towards +x/+y),
    flush with the facade, so nothing overhangs a player jumping along the wall.
    """
    t0, t1 = (0, .3) if inward > 0 else (-.3, 0)
    horizontal = abs(y1 - y0) < 1e-6
    start, end = (x0, x1) if horizontal else (y0, y1)
    spans = [(start, end)]
    for lo, hi in openings:
        spans = [p for a, b in spans for p in [(a, min(b, lo)), (max(a, hi), b)] if p[1]-p[0] > .05]
    for a, b in spans:
        if horizontal:
            box(a, y0+t0, b, y0+t1, z, z+h, mat)
        else:
            # Inset so corners never overlap a perpendicular parapet's top face.
            box(x0+t0, a+.3, x0+t1, b-.3, z, z+h, mat)


# ---------------------------------------------------------------------------
# Gameplay layout data. The plan renderer and checks read the same values.
# ---------------------------------------------------------------------------
ZONES = {
    'chowk':  dict(name='Ghanta Ghar Chowk', rect=[3, 3, 69, 30], range='10-30 m'),
    'bazaar': dict(name='Kinari Bazaar', rect=[3, 30, 17, 86], range='4-15 m'),
    'chatta': dict(name='Chatta Hall', rect=[20, 37, 46, 47], range='2-10 m'),
    'roofs':  dict(name='Bazaar Roofs +3.4', rect=[20, 34, 46, 76], range='8-25 m'),
    'baoli':  dict(name='Baoli step-well -2.4', rect=[46, 30, 69, 84], range='8-30 m'),
    'haveli': dict(name='Haveli Courtyard', rect=[18, 90, 54, 105], range='5-18 m'),
}

STAIRS = [
    dict(id='S1', name='chowk -> A roof', bottom=[20, 32, 0], top=[28, 32, ROOF], width=4, walls=[-1]),
    dict(id='S2', name='crossing street -> B roof', bottom=[22, 58, 0], top=[22, 66, ROOF], width=4, walls=[1]),
    dict(id='S3', name='baoli rim -> A roof', bottom=[48, 52, 0], top=[48, 44, ROOF], width=4, walls=[1]),
    dict(id='S4', name='north street -> B roof', bottom=[44, 78, 0], top=[36, 78, ROOF], width=4, walls=[-1]),
    dict(id='S5', name='courtyard -> gatehouse roof', bottom=[42, 92, 0], top=[34, 92, ROOF], width=4, walls=[-1]),
    dict(id='B1', name='baoli south flight', bottom=[62, 40, PIT], top=[62, 34, 0], width=6, walls=[]),
    dict(id='B2', name='baoli north flight', bottom=[62, 72, PIT], top=[62, 78, 0], width=6, walls=[]),
    dict(id='B3', name='east ghat -> baoli floor', bottom=[67, 66, PIT], top=[67, 60, 0], width=4, walls=[-1]),
]
# Solid landings flush with the roof at the top of each roof stair.
LANDINGS = {'S1': (28, 30, 32, 34), 'S2': (20, 66, 24, 70), 'S3': (46, 40, 50, 44),
            'S4': (32, 76, 36, 80), 'S5': (30, 90, 34, 94)}

# (x, y, z, compass angle: 0 east, 90 north, 180 west, 270 south).
# Consecutive indices lie in different districts because respawns step by one.
SPAWNS = [
    (15.2, 4.0, 0, 45),   # chowk south arcade, west end of the west bay
    (12, 96.5, 0, 0),     # haveli west gate passage
    (61.6, 12, 0, 180),   # chowk east, kotwali porch
    (15, 79, 0, 90),      # bazaar north, facing the north street
    (50, 103.2, 0, 270),  # courtyard north verandah, east
    (52, 4.5, 0, 90),     # chowk south arcade, east bay
    (68, 44, PIT, 90),    # baoli floor, south-east corner
    (22, 103.2, 0, 270),  # courtyard north verandah, west
    (24, 44.5, 0, 0),     # chatta hall west passage (bazaar side), facing into the hall
    (66, 91, 0, 270),     # east alley
    (36, 38.5, 0, 90),    # chatta hall
    (40, 52.8, 0, 180),   # crossing street, against the A block
]

PICKUPS = [
    (36, 23, 0), (50.5, 16.6, 1.1), (16, 9, 0),       # chowk: north of tower, chabutra, south-west
    (4.5, 43, 0), (12, 82, 0),                            # bazaar
    (33, 42, 0),                                        # chatta hall
    (42, 38, ROOF), (28, 62, ROOF),                     # roofs
    (60, 47, PIT), (58, 66, PIT),                       # baoli floor
    (67, 57, 0),                                        # east ghat
    (54, 81, 0),                                        # north street
    (42, 88, ROOF),                                     # gatehouse roof
    (24, 96, 0), (48, 99, 0),                           # courtyard
    (6, 92, 0),                                         # west alley
]

ROUTES = {
    'west ground': [[16, 14, 0], [12, 17, 0], [10, 30, 0], [13, 40, 0], [17.5, 48, 0], [12, 56, 0], [8, 62.8, 0], [9.5, 70, 0], [11, 80, 0], [6, 90, 0], [6, 96.5, 0], [14, 96.5, 0], [22, 96.5, 0]],
    'west rim': [[46, 12, 0], [50, 28, 0], [52, 40, 0], [52, 52, 0], [47, 60, 0], [46, 72, 0], [51, 80, 0], [49, 92, 0]],
    'baoli floor': [[62, 28, 0], [62, 34, 0], [62, 40, PIT], [61, 56, PIT], [63.5, 63, PIT], [62, 72, PIT], [62, 78, 0]],
    'bridge + ghat': [[52, 56, 0], [62, 56, 0], [67, 56, 0], [67, 60, 0], [67, 66, PIT]],
    'roof spine': [[14, 32, 0], [20, 32, 0], [28, 32, ROOF], [33, 40, ROOF], [33, 55, ROOF], [34, 64, ROOF], [34, 77, ROOF], [44, 78, 0]],
    'chatta crossing': [[10, 44.5, 0], [24, 44.5, 0], [33, 42, 0], [42, 39.5, 0], [52, 39.5, 0]],
    'cross street': [[12, 55, 0], [30, 55, 0], [50, 55, 0]],
    'haveli gates': [[23, 80, 0], [23, 93, 0], [28, 100, 0], [44, 100, 0], [49.5, 100, 0], [49, 93, 0], [49, 80, 0]],
    'east alley': [[56, 81, 0], [66, 84, 0], [66, 92.5, 0], [58, 92.5, 0], [52, 95, 0]],
}

# Optional movement links, verified with the game's movement code (120 Hz ticks).
TRICKS = [
    dict(id='T1 counter double jump -> A roof', start=[16.2, 38, 1.1], dir=[1, 0], events=[[2, 'jump'], [44, 'jump']]),
    dict(id='T2 baoli wall double jump -> rim', start=[57, 50, PIT], dir=[-1, 0], events=[[10, 'jump'], [52, 'jump']]),
    dict(id='T4 B roof -> gatehouse facade (should fail)', start=[40, 73, ROOF], dir=[0, 1], events=[[22, 'jump'], [64, 'jump'], [100, 'dash']]),
    dict(id='T5 chowk -> A roof wall climb', start=[40, 31, 0], dir=[0, 1], events=[[6, 'jump'], [48, 'jump'], [84, 'dashup']]),
    dict(id='T6 A roof -> B roof over street', start=[24, 49, ROOF], dir=[0, 1], events=[[14, 'jump'], [56, 'jump']]),
    dict(id='T7 ground double jump onto A roof (should fail)', start=[40, 31, 0], dir=[0, 1], events=[[6, 'jump'], [48, 'jump']]),
    dict(id='T8 chabutra hop', start=[44, 13, 0], dir=[1, 0], events=[[20, 'jump']]),
    dict(id='T9 double jump hugging the decorated west frontage', start=[3.42, 16, 0], dir=[0, 1], events=[[10, 'jump'], [52, 'jump']]),
    dict(id='T10 double jump hugging the A block shopfronts', start=[36, 33.58, 0], dir=[1, 0], events=[[10, 'jump'], [52, 'jump']]),
    dict(id='T11 double jump hugging the haveli courtyard wing', start=[18.42, 92, 0], dir=[0, 1], events=[[6, 'jump'], [48, 'jump']]),
]


def mass(x0, y0, x1, y1, h, mat='plaster', z0=0.):
    """Non-playable building volume. Clip above it so no trick jump can stand on it."""
    box(x0, y0, x1, y1, z0, h, mat)
    clip(x0, y0, x1, y1, h)


def ground():
    # A deep base under everything (step-well floor level), then the street fill
    # around the step-well hole. All structural so the BSP stays sealed.
    box(-1, -1, W+1, L+1, PIT-.8, PIT, 'paving', False)
    for x0, y0, x1, y1 in [(0, 0, 55, L), (69, 0, W, L), (55, 0, 69, 34), (55, 78, 69, L),
                           (55, 34, 59, 40), (65, 34, 69, 40), (55, 72, 59, 78), (65, 72, 69, 78),
                           (65, 54, 69, 60)]:
        box(x0, y0, x1, y1, PIT, 0, 'paving', False)


def boundary():
    # Outer frontage; clip continues to the sky so tricks cannot leave the map.
    for x0, y0, x1, y1 in [(0, 0, W, 3), (0, L-3, W, L), (0, 3, 3, L-3), (W-3, 3, W, L-3)]:
        box(x0, y0, x1, y1, 0, 9, 'plaster', False)
        clip(x0, y0, x1, y1, 9)
    for x0, y0, x1, y1 in [(-2, -2, W+2, -1), (-2, L+1, W+2, L+2), (-2, -1, -1, L+1), (W+1, -1, W+2, L+1)]:
        box(x0, y0, x1, y1, PIT-.8, SKY+1, 'skies/sandstone', False)
    box(-2, -2, W+2, L+2, SKY, SKY+1, 'skies/sandstone', False)


def chowk():
    mass(3, 3, 14, 12, 8.2)
    # Kotwali (police post) narrows the east of the square.
    mass(60, 3, 69, 10, 8.4); mass(60, 14, 69, 20, 8.4); mass(63, 10, 69, 14, 8.4)
    # Baoli Darwaza: gate house between the square and the step-well. Two offset
    # covered gates replace the long north-south sightlines along the rim and well.
    mass(44, 26, 48, 30, 7.0); mass(44, 30, 46, 33.8, 7.0); mass(52, 26, 59, 30, 7.0); mass(65, 26, 69, 30, 7.0)
    for x0, x1 in [(48, 52), (59, 65)]:
        box(x0, 26, x1, 30, CEIL+.4, 7.0, 'plaster'); clip(x0, 26, x1, 30, 7.0)
    # South arcade (dalan): overhead cover for the two south spawns.
    box(14, 3, 60, 7, CEIL, ROOF, 'plaster'); clip(14, 3, 60, 7, ROOF)
    for x in np.arange(16.2, 58.5, 4.2):
        box(float(x)-.4, 6.2, float(x)+.4, 7, 0, CEIL, 'stone')
    f = Face('n', 7, 14, 60)
    xs = [float(x) for x in np.arange(16.2, 58.5, 4.2)]
    for xa, xb in zip(xs, xs[1:]):
        arch_band(f, (xa+xb)/2, xb-xa-.8, 2.0, 1.0, .1, -.8, 0, 'stone')
    # Shop partitions split the arcade into bays open only to the square.
    for x in (27, 45):
        box(x-.2, 3, x+.2, 6.2, 0, CEIL, 'plaster')
    # Ghanta Ghar: an 18 m clock tower blocks the long diagonals of the square.
    box(31.5, 11.5, 40.5, 20.5, 0, .3, 'stone')
    box(32.5, 12.5, 39.5, 19.5, .3, 18, 'brick')
    clip(32, 12, 40, 20, 3)
    # Chabutra (pigeon platform): +1.1 island, three 0.37 m steps, dovecote in the middle.
    box(49, 12, 57, 18, 0, 1.1, 'stone')
    for h, a, b in [(.37, 1.0, .5), (.74, .5, 0)]:
        box(49-a, 13.5, 49-b, 16.5, 0, h, 'stone')
        box(57+b, 13.5, 57+a, 16.5, 0, h, 'stone')
    box(52.2, 14.2, 53.8, 15.8, 1.1, 4.2, 'wood'); clip(52.2, 14.2, 53.8, 15.8, 4.2)
    # Carts and a trough: waist-high cover on the open lanes of the square.
    box(14, 17, 18, 19, 0, 1.1, 'wood')
    box(44, 20.5, 46, 24.5, 0, 1.1, 'wood')
    box(22, 10, 25, 12, 0, 1.1, 'brick')


def bazaar():
    # The lane narrows and turns: no straight shot from the chowk to the haveli.
    mass(3, 66, 6, 80, 8.0)
    for y0 in (34, 52):
        box(3, y0, 5.5, y0+5, 0, 1.1, 'wood')
    # Counter in front of the A block: a double jump from it reaches the roof.
    box(15.4, 36, 17, 40, 0, 1.1, 'wood')
    # Sabil (water kiosk) and chai stall break the lane's long sightlines.
    box(3, 46, 15, 50, 0, 2.6, 'blue'); clip(3, 46, 15, 50, 2.6)
    box(10, 61, 19.8, 64.6, 0, 2.4, 'wood'); clip(10, 61, 19.8, 64.6, 2.4)
    box(13, 73.5, 16, 75.5, 0, 1.1, 'wood')


def block_a():
    # Bazaar block with a walkable roof and a covered hall at ground level.
    for x0, y0, x1, y1 in [(20, 47, 46, 52), (20, 34, 46, 37), (20, 37, 28, 42), (38, 42, 46, 47)]:
        box(x0, y0, x1, y1, 0, CEIL, 'plaster_ochre')
    box(20, 34, 46, 52, CEIL, ROOF, 'roof')
    for x, y in [(31, 40), (35, 44), (29.1, 41.9)]:
        box(x-.5, y-.5, x+.5, y+.5, 0, CEIL, 'stone')
    entity('light', P(33, 42, 2.6), light='120', _color='1 .85 .65')
    entity('light', P(24, 44.5, 2.6), light='70', _color='1 .85 .65')
    entity('light', P(42, 39.5, 2.6), light='70', _color='1 .85 .65')
    # Barsati room breaks roof-to-roof sightlines.
    box(38, 45, 44, 50, ROOF, ROOF+2.6, 'plaster')
    clip(38, 45, 44, 50, ROOF+2.6)
    parapet(20, 34, 46, 34, ROOF, openings=[(28, 32)])
    parapet(20, 34, 20, 52, ROOF, openings=[(36, 40)])
    parapet(46, 34, 46, 52, ROOF, openings=[(40, 44)], inward=-1)
    parapet(20, 52, 46, 52, ROOF, openings=[(31, 35)], inward=-1)


def block_b():
    box(24, 58, 44, 76, 0, CEIL, 'plaster_rose'); box(20, 70, 24, 76, 0, CEIL, 'plaster_rose')
    box(24, 58, 44, 76, CEIL, ROOF, 'roof'); box(20, 70, 24, 76, CEIL, ROOF, 'roof')
    box(20.2, 72, 26, 75.8, ROOF, ROOF+2.6, 'plaster')
    clip(20.2, 72, 26, 75.8, ROOF+2.6)
    parapet(24, 58, 44, 58, ROOF, openings=[(31, 35)])
    parapet(20, 66, 20, 76, ROOF)
    parapet(44, 58, 44, 76, ROOF, inward=-1)
    parapet(20, 76, 44, 76, ROOF, openings=[(32, 36)], inward=-1)
    # Bridge across the open crossing street (4 m clear width).
    box(31, 52, 35, 58, ROOF-.3, ROOF, 'wood')
    for x in (31.1, 34.9):
        box(x-.1, 52.15, x+.1, 57.85, ROOF, ROOF+.9, 'wood')


def stairs():
    for s in STAIRS:
        stair(s['bottom'], s['top'], s['width'], walls=s['walls'])
    for x0, y0, x1, y1 in LANDINGS.values():
        box(x0, y0, x1, y1, 0, ROOF, 'stone')


def baoli():
    box(55, 40, 69, 72, PIT, PIT+.03, 'tile')
    # Rim pavilion splits the west rim's long north-south sightline.
    box(48, 66, 55, 70, 0, 2.6, 'stone'); clip(48, 66, 55, 70, 2.6)
    # Bridge to the east ghat, three arches underneath (2.15 m clear headroom).
    box(55, 54, 65, 58, -.25, 0, 'stone')
    for x0 in (59, 62):
        box(x0, 54, x0+1, 58, PIT, -.25, 'brick')
    # Screens on the bridge stop rim-to-rim shots along the well; offset gaps let players drop in.
    for y0, gap, end in [(53.85, (62, 64), 69), (57.85, (58, 60), 65)]:
        for a, b in [(52, gap[0]), (gap[1], end)]:
            box(a, y0, b, y0+.3, 0, 2.2, 'brick')
    # Two basins as cover on the well floor.
    box(57, 45, 59, 48, PIT, PIT+1.0, 'stone')
    box(60, 64, 62, 67, PIT, PIT+1.0, 'stone')


def haveli():
    # Wings around the courtyard; side gates are covered passages through them.
    mass(9, 86, 18, 94, 9.0); mass(9, 99, 18, L-3, 9.0); mass(3, 99, 9, L-3, 9.0)
    mass(54, 86, 63, 90, 9.0); mass(54, 95, 63, L-3, 9.0); mass(63, 95, 69, L-3, 9.0)
    mass(18, 105, 54, L-3, 9.0)
    box(9, 94, 18, 99, CEIL, 9.0, 'plaster'); clip(9, 94, 18, 99, 9.0)
    box(54, 90, 63, 95, CEIL, 9.0, 'plaster'); clip(54, 90, 63, 95, 9.0)
    # Gatehouse with a walkable roof; two gates underneath.
    for x0, x1 in [(18, 20), (26, 46), (52, 54)]:
        box(x0, 86, x1, 90, 0, CEIL, 'plaster')
    box(18, 86, 54, 90, CEIL, ROOF, 'roof')
    # Street facade rises above the gatehouse roof: the roof overlooks the courtyard,
    # not the long streets.
    parapet(26, 86, 46, 86, ROOF, h=2.4, mat='plaster')
    clip(26, 86, 46, 86.3, ROOF+2.4)
    parapet(18, 90, 54, 90, ROOF, openings=[(18, 26), (30, 34), (46, 54)], inward=-1)
    # Corner chhatri bases end the roof short of both long street axes.
    for x0, x1 in [(18, 26), (46, 54)]:
        box(x0, 86, x1, 90, ROOF, ROOF+2.4, 'stone'); clip(x0, 86, x1, 90, ROOF+2.4)
    # North verandah shelters the courtyard spawns from the gatehouse roof.
    box(18, 101, 54, 105, CEIL, ROOF, 'plaster'); clip(18, 101, 54, 105, ROOF)
    for x in np.arange(19.5, 53, 4.0):
        box(float(x)-.4, 101, float(x)+.4, 101.8, 0, CEIL, 'stone')
    box(35.8, 101.8, 36.2, 105, 0, CEIL, 'plaster')
    # Courtyard: pool and a low cart.
    box(31, 95, 41, 99, 0, .5, 'stone')
    box(23, 96.5, 26, 98.5, 0, 1.1, 'wood')
    box(46, 96, 48, 99, 0, 1.1, 'wood')


# ---------------------------------------------------------------------------
# Architecture layer. Facade dressing (frames, sills, balconies, cornices, arches)
# is visual-only (func_illusionary): walls stay flat for collision, so running or
# jumping along a facade never snags on a frame and no ornament is a perch.
# ---------------------------------------------------------------------------
DECOR = ('stone', 'brick', 'wood', 'teal', 'blue', 'shutter', 'sign', 'kashi', 'clock', 'plaster_ochre', 'plaster_rose', 'shade')


def decor(mat):
    return mat + '_d' if mat in DECOR else mat
class Face:
    """Local frame on a vertical face: u along the face, d outward, z up."""
    def __init__(self, side, plane, u0, u1):
        self.side, self.plane, self.u0, self.u1 = side, plane, u0, u1

    def xy(self, u, d):
        p = self.plane
        return {'n': (u, p+d), 's': (u, p-d), 'e': (p+d, u), 'w': (p-d, u)}[self.side]

    def box(self, u0, u1, d0, d1, z0, z1, mat, solid=False):
        (xa, ya), (xb, yb) = self.xy(u0, d0), self.xy(u1, d1)
        box(min(xa, xb), min(ya, yb), max(xa, xb), max(ya, yb), z0, z1, mat if solid else decor(mat))

    def clip(self, d, z0, z1=SKY, u0=None, u1=None):
        (xa, ya), (xb, yb) = self.xy(self.u0 if u0 is None else u0, 0), self.xy(self.u1 if u1 is None else u1, d)
        clip(min(xa, xb), min(ya, yb), max(xa, xb), max(ya, yb), z0, z1)

    def poly(self, pts, d0, d1, mat):
        """Convex (u, z) polygon extruded between d0 and d1."""
        verts = [P(*self.xy(u, d), z) for d in (d0, d1) for u, z in pts]
        n = len(pts)
        brush(verts, [tuple(range(n)), tuple(range(n, 2*n))] + [(i, (i+1) % n, (i+1) % n+n, i+n) for i in range(n)], material(decor(mat)))


def arc(t):
    # Two-centred pointed (Mughal) arch: vertical springing, pointed crown.
    return math.sqrt(max(0., 1-(1-t*.5)**2))/math.sqrt(.75)


def arch_band(f, uc, width, spring, rise, thick, d0, d1, mat, seg=5):
    for sign in (-1, 1):
        for i in range(seg):
            t0, t1 = i/seg, (i+1)/seg
            a, b = uc+sign*width/2*(1-t0), uc+sign*width/2*(1-t1)
            lo, hi = spring+rise*arc(t0), spring+rise*arc(t1)
            f.poly([(a, lo), (b, hi), (b, hi+thick), (a, lo+thick)], d0, d1, mat)


def arch_fill(f, uc, width, spring, rise, d0, d1, mat, seg=6):
    pts = [(uc-width/2, spring)]
    pts += [(uc-width/2*(1-i/seg), spring+rise*arc(i/seg)) for i in range(1, seg+1)]
    pts += [(uc+width/2*(1-i/seg), spring+rise*arc(i/seg)) for i in range(seg-1, 0, -1)]
    pts += [(uc+width/2, spring)]
    f.poly(pts, d0, d1, mat)


def window(f, uc, z0, w=1.0, h=1.5, shutters='teal'):
    rise = w*.45
    f.box(uc-w/2, uc+w/2, 0, .03, z0, z0+h, 'window')
    arch_fill(f, uc, w, z0+h, rise, 0, .03, 'window')
    arch_band(f, uc, w+.1, z0+h, rise+.05, .12, 0, .08, 'stone')
    for s in (-1, 1):
        f.box(uc+s*(w/2+.05)-.05, uc+s*(w/2+.05)+.05, 0, .08, z0, z0+h, 'stone')
    f.box(uc-w/2-.15, uc+w/2+.15, 0, .08, z0-.12, z0, 'stone')
    if shutters:
        for s in (-1, 1):
            f.box(min(uc+s*(w/2+.12), uc+s*(w/2+.55)), max(uc+s*(w/2+.12), uc+s*(w/2+.55)), .02, .06, z0, z0+h, shutters)
    f.box(uc-w/2-.35, uc+w/2+.35, 0, .38, z0+h+rise+.2, z0+h+rise+.3, 'eave')


def door(f, uc, w=1.4, h=2.3, mat='wood', base=0.):
    rise = w*.4
    f.box(uc-w/2, uc+w/2, 0, .04, base, base+h, mat)
    arch_fill(f, uc, w, base+h, rise, 0, .04, 'wood_decor')
    arch_band(f, uc, w+.2, base+h, rise+.08, .16, 0, .08, 'stone')
    for s in (-1, 1):
        f.box(min(uc+s*w/2, uc+s*(w/2+.16)), max(uc+s*w/2, uc+s*(w/2+.16)), 0, .08, base, base+h, 'stone')
    f.box(uc-w/2-.1, uc+w/2+.1, 0, .07, base, base+.1, 'stone')


def shopfront(f, u0, u1, h=2.6, cloth='cloth_red'):
    f.box(u0+.15, u1-.15, 0, .04, .1, h, 'shutter')
    f.box(u0, u0+.15, 0, .08, 0, h, 'stone'); f.box(u1-.15, u1, 0, .08, 0, h, 'stone')
    f.box(u0, u1, 0, .08, h, h+.15, 'stone')
    f.box(u0+.3, u1-.3, 0, .06, h+.22, h+.52, 'sign')
    # Sloped cloth awning (non-solid), high enough to walk and jump under.
    verts = []
    for (u, d, z) in [(u0+.1, .1, h+1.0), (u1-.1, .1, h+1.0), (u1-.1, 1.3, h+.75), (u0+.1, 1.3, h+.75),
                      (u0+.1, .1, h+1.06), (u1-.1, .1, h+1.06), (u1-.1, 1.3, h+.81), (u0+.1, 1.3, h+.81)]:
        verts.append(P(*f.xy(u, d), z))
    brush(verts, CUBE, material(cloth))


def jharokha(f, uc, z0, w=2.0, roof='wood'):
    """Projecting carved balcony (unreachable: front clip)."""
    for s in (-1, 0, 1):
        f.poly([(uc+s*(w/2-.2)-.08, z0), (uc+s*(w/2-.2)+.08, z0), (uc+s*(w/2-.2)+.08, z0-.05), (uc+s*(w/2-.2)-.08, z0-.05)], 0, .7, 'wood')
        f.poly([(uc+s*(w/2-.2)-.07, z0-.05), (uc+s*(w/2-.2)+.07, z0-.05), (uc+s*(w/2-.2)+.07, z0-.6)], 0, .12, 'wood')
    f.box(uc-w/2, uc+w/2, 0, .75, z0, z0+.22, 'wood')
    f.box(uc-w/2+.1, uc+w/2-.1, 0, .03, z0+.22, z0+1.7, 'window')
    for s in (-1, 1):
        f.box(uc+s*w/2-(.12 if s > 0 else 0), uc+s*w/2+(0 if s > 0 else .12), .62, .75, z0+.22, z0+1.85, 'wood')
        f.box(uc+s*w/2-(.1 if s > 0 else 0), uc+s*w/2+(0 if s > 0 else .1), .05, .62, z0+.22, z0+1.6, 'jali')
    f.box(uc-w/2+.12, uc+w/2-.12, .68, .72, z0+.22, z0+1.6, 'jali')
    f.box(uc-w/2-.15, uc+w/2+.15, 0, .95, z0+1.85, z0+2.0, roof)
    f.poly([(uc-w/2-.05, z0+2.0), (uc+w/2+.05, z0+2.0), (uc, z0+2.55)], .1, .8, roof)


def trim_dz(f):
    # Trims of perpendicular faces meet in inner corners; offset them so they never share a plane.
    return -.03 if f.side in 'ew' else 0.


def cornice(f, h, mat='stone', merlons=True, inside=None, lip=.3):
    dz = trim_dz(f)
    f.box(f.u0, f.u1, 0, min(.18, lip*.9), h-.6+dz, h-.42+dz, 'brick')
    f.box(f.u0, f.u1, 0, lip, h-.22+dz, h+dz, mat)
    if merlons and inside is not None:
        # Kanguras stand on the roof edge, inside the mass's own clip column.
        u = f.u0+.3
        while u+.35 < f.u1:
            f.box(u, u+.35, -.25, 0, h, h+.45, 'brick'); u += .8


def plinth(f, u0=None, u1=None, h=.45):
    f.box(f.u0 if u0 is None else u0, f.u1 if u1 is None else u1, 0, .06, 0, h+trim_dz(f), 'stone')


def house(f, h, rng, ground='doors', floors=None, gaps=(), jharokhas=True, base=0., top_clip=True, shop_h=2.6, lip=.3):
    """A generic old-city frontage on a face. gaps: (u0, u1) spans left plain (stairs, openings)."""
    def free(a, b):
        return all(b <= g0 or a >= g1 for g0, g1 in gaps)
    bays = max(1, int((f.u1-f.u0)//3.2))
    width = (f.u1-f.u0)/bays
    spans = [(f.u0, f.u1)]
    for g0, g1 in gaps:
        spans = [q for a, b in spans for q in [(a, min(b, g0)), (max(a, g1), b)] if q[1]-q[0] > .05]
    for a, b in spans:
        plinth(f, a, b)
    floors = floors or ([base+3.7] + ([base+6.7] if h-base > 8.2 else []))
    for i in range(bays):
        a, b = f.u0+i*width, f.u0+(i+1)*width
        uc = (a+b)/2
        if not free(a, b):
            continue
        if ground == 'doors' and base == 0:
            if rng.random() < .55: door(f, uc, 1.3, 2.25, rng.choice(['wood', 'teal', 'blue']))
            else: window(f, uc, 1.0, .9, 1.2, rng.choice(['teal', 'wood', None]))
        elif ground == 'shops' and base == 0:
            shopfront(f, a+.25, b-.25, h=shop_h, cloth=rng.choice(['cloth_red', 'cloth_teal', 'cloth_yellow']))
        for k, z in enumerate(floors):
            if z+2.3 > h-.4:
                continue
            if jharokhas and k == 0 and 0 < i < bays-1 and rng.random() < .3 and width >= 2.8:
                jharokha(f, uc, z-.2, min(2.2, width-.8))
            else:
                window(f, uc, z, .95, 1.45, rng.choice(['teal', 'wood', 'blue', None]))
        if i:
            f.box(a-.08, a+.08, 0, .06, .45, h-.6, 'brick')
    # Floor band between storeys.
    for a, b in spans:
        for z in floors:
            if z+.1 < h-.6:
                f.box(a, b, 0, .07, z-.55+trim_dz(f), z-.4+trim_dz(f), 'brick')
        cornice(Face(f.side, f.plane, a, b), h, lip=lip)


def lamp(x, y, z, light=55):
    """Lantern hanging from a ceiling whose underside is at z."""
    box(x-.03, y-.03, x+.03, y+.03, z-.25, z, 'brass')
    box(x-.13, y-.13, x+.13, y+.13, z-.55, z-.25, 'lantern')
    entity('light', P(x, y, z-.7), light=str(light), _color='1 .82 .55')


def wall_lamp(f, u, z, light=50):
    """Lantern on an iron bracket fixed to a face."""
    f.box(u-.04, u+.04, 0, .5, z, z+.06, 'brass')
    f.box(u-.13, u+.13, .32, .58, z-.32, z, 'lantern')
    x, y = f.xy(u, .45)
    entity('light', P(x, y, z-.5), light=str(light), _color='1 .82 .55')


def decorate():
    rng = random.Random(7031)
    # --- boundary frontages (inner faces) ---
    H = 9.0
    for side, plane, u0, u1, ground in [
        ('e', 3, 12, 34, 'doors'), ('e', 3, 39, 46, 'doors'), ('e', 3, 57, 66, 'shops'), ('e', 3, 80, 86, 'doors'), ('e', 3, 86, 99, 'doors'),
        ('w', W-3, 20, 26, 'doors'), ('w', W-3, 78, 86, 'doors'), ('w', W-3, 86, 95, 'doors')]:
        house(Face(side, plane, u0, u1), H, rng, ground)
    # Pit-side boundary: house above, brick step-well wall with niches below the rim.
    pit = Face('w', W-3, 34, 78)
    house(Face('w', W-3, 34, 78), H, rng, 'none', gaps=[(54, 60)])
    # Corner and district masses.
    house(Face('n', 12, 3, 14), 8.2, rng, 'doors')
    house(Face('e', 14, 7, 12), 8.2, rng, 'doors')
    house(Face('w', 60, 14, 20), 8.4, rng, 'doors'); house(Face('w', 60, 3, 10), 8.4, rng, 'doors')
    house(Face('n', 20, 60, 69), 8.4, rng, 'doors')
    # Kotwali porch: a real (closed) door at the back and a lantern.
    door(Face('w', 63, 10, 14), 12, 1.6, 2.5, 'teal'); lamp(61.5, 12, 4.0, 50)
    box(60, 10, 63, 14, 4.0, 8.4, 'plaster'); clip(60, 10, 63, 14, 4.0)
    house(Face('e', 6, 66, 80), 8.0, rng, 'shops')
    house(Face('s', 66, 3, 6), 8.0, rng, 'none', jharokhas=False)
    house(Face('n', 80, 3, 6), 8.0, rng, 'none', jharokhas=False)
    # Haveli outside and courtyard faces.
    house(Face('s', 86, 9, 18), 9.0, rng, 'doors'); house(Face('w', 9, 86, 94), 9.0, rng, 'doors')
    house(Face('s', 86, 54, 63), 9.0, rng, 'doors'); house(Face('e', 63, 86, 90), 9.0, rng, 'doors')
    house(Face('s', 95, 63, 69), 9.0, rng, 'doors')
    house(Face('s', 99, 3, 9), 9.0, rng, 'doors')
    house(Face('e', 18, 90, 94), 9.0, rng, 'doors'); house(Face('e', 18, 99, 105), 9.0, rng, 'none')
    house(Face('w', 54, 95, 105), 9.0, rng, 'none')
    house(Face('s', 109-4, 18, 54), 9.0, rng, 'none', floors=[3.9, 6.6])
    # Darwaza and kotwali porch.
    for x0, x1 in [(44, 48), (52, 59), (65, 69)]:
        house(Face('s', 26, x0, x1), 7.0, rng, 'none', jharokhas=False)
    for x0, x1 in [(46, 48), (52, 59), (65, 69)]:
        house(Face('n', 30, x0, x1), 7.0, rng, 'none', jharokhas=False)
    for x0, x1 in [(48, 52), (59, 65)]:
        f = Face('s', 26, x0, x1)
        arch_band(f, (x0+x1)/2, x1-x0, CEIL+.4, (x1-x0)*.35, .4, 0, .4, 'stone')
        f.box(x0, x1, 0, .4, CEIL+.4+(x1-x0)*.35+.4, 7.0, 'plaster', solid=True)
        f.clip(.45, CEIL+.4+(x1-x0)*.35+.4)
        f = Face('n', 30, x0, x1)
        arch_band(f, (x0+x1)/2, x1-x0, CEIL+.4, (x1-x0)*.35, .4, 0, .4, 'stone')
        lamp((x0+x1)/2, 28, CEIL+.4, 70)
    # Chhatris on the darwaza crown (inside its clip column).
    for cx in (46, 67):
        for dx in (-1.2, 1.2):
            for dy in (-1.2, 1.2):
                box(cx+dx-.15, 28+dy-.15, cx+dx+.15, 28+dy+.15, 7.0, 9.0, 'stone')
        box(cx-1.6, 26.4, cx+1.6, 29.6, 9.0, 9.25, 'stone')
        prism(cx, 28, 9.25, 1.45, 1.2, 'plaster', 12, .2)
    # --- blocks A and B (walkable roofs: clip only up to the roof edge) ---
    for side, plane, u0, u1, gaps in [('s', 34, 32, 45.7, []), ('w', 20, 34, 52, [(42, 47), (36, 40)]),
                                      ('n', 52, 20, 46, [(31, 35)]), ('e', 46, 34, 52, [(37, 42), (40, 52)]),
                                      ('s', 58, 24, 44, [(31, 35)]), ('e', 44, 58, 76, []), ('n', 76, 20, 32, []), ('w', 20, 70, 76, [])]:
        f = Face(side, plane, u0, u1)
        # Walkable-roof blocks: thin lip, no clip, so jumps along them are unaffected.
        house(f, ROOF, rng, 'shops', floors=[9.0], gaps=gaps, jharokhas=False, top_clip=False, shop_h=2.2, lip=.03)
    # Hall interior: lamps along the passages.
    for x, y in [(24, 44.5), (30, 42), (36, 42), (42, 39.5)]:
        lamp(x, y, CEIL, 60)
    # --- gatehouse street facade above the roof and gate arches ---
    f = Face('s', 86, 26, 46)
    for uc in (30, 36, 42):
        jharokha(f, uc, ROOF+.6, 2.2) if uc == 36 else window(f, uc, ROOF+.5, 1.0, 1.3)
    for x0, x1 in [(20, 26), (46, 52)]:
        for side, plane in [('s', 86), ('n', 90)]:
            arch_band(Face(side, plane, x0, x1), (x0+x1)/2, x1-x0, 2.2, .8, .1, 0, .08, 'stone')
        lamp((x0+x1)/2, 88, CEIL, 60)
    lamp(13.5, 96.5, CEIL, 60); lamp(58.5, 92.5, CEIL, 60)
    # --- courtyard verandah: cusped arches between the pillars ---
    f = Face('s', 101, 18, 54)
    xs = list(np.arange(19.5, 53, 4.0))
    for a, b in zip(xs, xs[1:]):
        arch_band(f, (a+b)/2, b-a-.8, 2.1, .9, .1, -.8, 0, 'stone')
    for x in (22, 30, 42, 50):
        lamp(x, 103, CEIL, 45)
    # Haveli pool: kashi tiles and a fountain.
    box(31.3, 95.3, 40.7, 98.7, .5, .52, 'water')
    prism(36, 97, .5, .35, 1.0, 'kashi', 8)
    # --- clock tower (Ghanta Ghar) ---
    t = 36, 16
    for z0, z1, r in [(.3, 1.0, 3.8), (6.0, 6.4, 3.75), (11.8, 12.2, 3.7)]:
        box(t[0]-r, t[1]-r, t[0]+r, t[1]+r, z0, z1, 'stone')
    for side, plane in [('n', 19.5), ('s', 12.5), ('e', 39.5), ('w', 32.5)]:
        f = Face(side, plane, (32.5 if side in 'ns' else 12.5), (39.5 if side in 'ns' else 19.5))
        arch_band(f, 36 if side in 'ns' else 16, 2.2, 2.0, 1.1, .2, 0, .25, 'stone')
        f.box((36 if side in 'ns' else 16)-1.1, (36 if side in 'ns' else 16)+1.1, 0, .04, .4, 2.0, 'wood')
        window(f, 36 if side in 'ns' else 16, 7.4, 1.2, 2.4, None)
        f.poly([(c+(36 if side in 'ns' else 16), 14.9+s) for c, s in
                [(-1.3, 0), (-.9, -.9), (0, -1.3), (.9, -.9), (1.3, 0), (.9, .9), (0, 1.3), (-.9, .9)]], 0, .12, 'clock')
    box(32.2, 12.2, 39.8, 19.8, 16.6, 17.0, 'stone')
    for dx in (-3.3, 3.3):
        for dy in (-3.3, 3.3):
            box(36+dx-.3, 16+dy-.3, 36+dx+.3, 16+dy+.3, 17.0, 19.4, 'stone')
    box(32.3, 12.3, 39.7, 19.7, 19.4, 19.8, 'stone')
    prism(36, 16, 19.8, 3.2, 2.6, 'plaster', 16, .3)
    prism(36, 16, 22.4, .12, 1.6, 'brass', 6, .02)
    # --- chabutra dovecote roof ---
    prism(53, 15, 4.2, 1.3, 1.1, 'wood', 8, .1); clip(51.6, 13.6, 54.4, 16.4, 4.2)
    # --- baoli: niches in the well walls and lamps ---
    for side, plane, u0, u1 in [('e', 55, 40, 72), ('w', W-3, 40, 54), ('w', W-3, 66, 72)]:
        f = Face(side, plane, u0, u1)
        u = u0+1.0
        while u+2.2 <= u1:
            if not (53 < u < 59 and side == 'e'):
                f.box(u, u+.35, 0, .12, PIT, -.3, 'brick'); f.box(u+1.85, u+2.2, 0, .12, PIT, -.3, 'brick')
                arch_band(f, u+1.1, 1.5, PIT+1.3, .6, .15, 0, .12, 'brick')
                f.box(u+.35, u+1.85, 0, .02, PIT+.05, PIT+1.3, 'shade')
            u += 3.0
        for a, b in ([(u0, 54), (58, u1)] if side == 'e' else [(u0, u1)]):
            f.box(a, b, 0, .15, -.3, 0, 'stone')
    lamp(61, 56, -.25, 50)
    wall_lamp(Face('e', 55, 0, 0), 46, -.5, 45); wall_lamp(Face('e', 55, 0, 0), 66, -.5, 45)
    wall_lamp(Face('w', W-3, 0, 0), 46, -.5, 45)
    box(61.6, 41, 62.4, 71, PIT+.03, PIT+.05, 'water')
    # --- bazaar dressing: stalls, awnings, sabil ---
    for y0 in (34, 52):
        box(3, y0+.2, 4.6, y0+4.8, 1.1, 1.5, 'spice')
        verts = [P(x, y, z) for z0 in (2.6, 2.66) for x, y, z in
                 [(3, y0, z0+.4), (6.2, y0, z0), (6.2, y0+5, z0), (3, y0+5, z0+.4)]]
        brush(verts, CUBE, material('cloth_yellow' if y0 == 34 else 'cloth_teal'))
    f = Face('e', 15, 46, 50)
    arch_band(f, 48, 2.6, 1.0, .8, .15, 0, .1, 'kashi')
    f.box(46.8, 49.2, 0, .03, .1, 1.0, 'kashi')
    box(3, 46, 15.3, 50.3, 2.6, 2.8, 'stone'); clip(3, 46, 15.3, 50.3, 2.6)
    prism(9, 48, 2.8, 1.8, 1.0, 'kashi', 12, .2)
    box(10, 61, 19.8, 64.6, 2.4, 2.5, 'wood')
    for f, u in [(Face('e', 3, 0, 0), 42), (Face('e', 3, 0, 0), 58), (Face('e', 6, 0, 0), 72), (Face('e', 3, 0, 0), 92),
                 (Face('w', W-3, 0, 0), 82), (Face('e', 44, 0, 0), 64), (Face('w', 60, 0, 0), 17), (Face('n', 12, 0, 0), 8)]:
        wall_lamp(f, u, 3.0, 45)
    # --- rim pavilion (chhatri) and ghat ---
    prism(51.5, 68, 2.6, 2.2, 1.4, 'plaster', 12, .3); clip(49.2, 65.7, 53.8, 70.3, 2.6)
    # --- rooftop rooms: doors and a water tank ---
    door(Face('s', 45, 38, 44), 41, 1.0, 2.0, base=ROOF); door(Face('s', 72, 20.2, 26), 23, 1.0, 2.0, base=ROOF)
    prism(30, 48, ROOF, .7, 1.4, 'plaster', 10); clip(29.2, 47.2, 30.8, 48.8, ROOF+1.4)


def materials():
    texture = SOURCE/'textures/sandstone'; texture.mkdir(parents=True, exist_ok=True)
    rng = random.Random(438)
    palette = dict(brick=(134,81,50), stone=(156,136,106), paving=(132,119,102), blue=(87,124,137),
                   teal=(46,98,108), wood=(76,49,31), wood_decor=(76,49,31), window=(36,52,56),
                   cloth_red=(118,48,35), cloth_teal=(42,91,96), water=(67,109,120), tile=(114,147,147))
    for name, c in palette.items():
        image = Image.new('RGB', (256,256), c); draw = ImageDraw.Draw(image)
        if name in ('brick','paving','stone'):
            bw, bh = (64,32) if name == 'brick' else (128,64)
            for row, y in enumerate(range(-bh, 256+bh, bh)):
                for x in range(-bw, 256+bw, bw):
                    xx = x+row % 2*bw/2; tint = rng.randrange(-10,11)
                    cc = tuple(max(0,min(255,v+tint)) for v in c)
                    draw.rectangle((xx+1,y+1,xx+bw-2,y+bh-2), fill=cc)
                    draw.line((xx+2,y+2,xx+bw-2,y+2), fill=tuple(min(255,v+12) for v in cc))
        elif name in ('wood','wood_decor'):
            for x in range(0,256,32):
                draw.line((x,0,x,255), fill=(41,28,20), width=3)
                for j in range(4):
                    xx = x+rng.randint(4,28); draw.line((xx,0,xx+rng.randint(-2,2),255), fill=(59,39,25))
        elif name == 'tile':
            for y in range(0,256,32):
                for x in range(0,256,32):
                    draw.rectangle((x+1,y+1,x+30,y+30), fill=(93,131,140))
                    draw.polygon([(x+16,y+5),(x+27,y+16),(x+16,y+27),(x+5,y+16)], fill=(163,160,127))
        elif name.startswith('cloth'):
            for x in range(0,256,24): draw.line((x,0,x,255), fill=tuple(min(255,v+16) for v in c), width=3)
        else:
            for _ in range(2800):
                xx, yy = rng.randrange(256), rng.randrange(256); v = rng.randrange(-5,6)
                draw.point((xx,yy), fill=tuple(max(0,min(255,k+v)) for k in c))
        image.save(texture/f'{name}.png')
    assert (texture/'plaster.png').exists(), 'Keep the authored plaster raster in source control.'
    # Tinted plaster variants from the authored raster (smaller, for district variety).
    base = Image.open(texture/'plaster.png').convert('RGB').resize((512, 512), Image.LANCZOS)
    for name, tint in [('plaster_ochre', (1.08, .93, .72)), ('plaster_rose', (1.06, .86, .80))]:
        arr = np.asarray(base, dtype=float)*np.array(tint)
        Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8)).save(texture/f'{name}.png')
    def tex(name, bg, paint):
        image = Image.new('RGB', (256, 256), bg); paint(ImageDraw.Draw(image), image); image.save(texture/f'{name}.png')
    def shutter(d, im):
        for y in range(0, 256, 16):
            d.rectangle((0, y, 255, y+13), fill=(62, 104, 98)); d.line((0, y+14, 255, y+14), fill=(34, 58, 55), width=2)
        for x in (0, 254): d.line((x, 0, x, 255), fill=(34, 58, 55), width=6)
    def sign(d, im):
        d.rectangle((0, 0, 255, 255), fill=(150, 44, 36)); d.rectangle((10, 10, 245, 245), outline=(214, 170, 60), width=8)
        for x in range(40, 220, 36): d.ellipse((x, 108, x+22, 130), fill=(232, 205, 120))
    def jali(d, im):
        for y in range(-32, 288, 32):
            for x in range(-32, 288, 32):
                d.polygon([(x+16, y), (x+32, y+16), (x+16, y+32), (x, y+16)], outline=(54, 34, 22), fill=(122, 84, 52))
                d.ellipse((x+11, y+11, x+21, y+21), fill=(40, 26, 18))
    def kashi(d, im):
        for y in range(0, 256, 64):
            for x in range(0, 256, 64):
                d.rectangle((x+1, y+1, x+62, y+62), fill=(236, 232, 214))
                d.ellipse((x+12, y+12, x+52, y+52), outline=(32, 86, 150), width=5)
                d.polygon([(x+32, y+6), (x+40, y+32), (x+32, y+58), (x+24, y+32)], fill=(46, 120, 168))
                d.polygon([(x+6, y+32), (x+32, y+24), (x+58, y+32), (x+32, y+40)], fill=(28, 70, 128))
                for cx, cy in ((x, y), (x+64, y), (x, y+64), (x+64, y+64)): d.ellipse((cx-7, cy-7, cx+7, cy+7), fill=(196, 150, 54))
    def clockface(d, im):
        d.rectangle((0, 0, 255, 255), fill=(238, 232, 210)); d.ellipse((8, 8, 248, 248), outline=(40, 34, 30), width=10)
        for i in range(12):
            a = i*math.tau/12; d.line((128+100*math.sin(a), 128-100*math.cos(a), 128+118*math.sin(a), 128-118*math.cos(a)), fill=(40, 34, 30), width=8)
        d.line((128, 128, 128, 52), fill=(30, 26, 24), width=8); d.line((128, 128, 182, 150), fill=(30, 26, 24), width=10)
    def roof(d, im):
        for row, y in enumerate(range(0, 256, 32)):
            for x in range(-32, 256, 64):
                xx = x+row % 2*32; t = rng.randrange(-8, 9)
                d.rectangle((xx+1, y+1, xx+62, y+30), fill=(196+t, 176+t, 146+t))
    def spice(d, im):
        cols = [(178, 48, 30), (214, 150, 40), (120, 70, 30), (196, 92, 34)]
        for i in range(4):
            d.rectangle((i*64, 0, i*64+63, 255), fill=cols[i])
            for _ in range(900):
                x, y = i*64+rng.randrange(64), rng.randrange(256); v = rng.randrange(-24, 25)
                d.point((x, y), fill=tuple(max(0, min(255, c+v)) for c in cols[i]))
    def shade(d, im):
        for y in range(256): d.line((0, y, 255, y), fill=(52+y//10, 40+y//12, 32+y//14))
    def brass(d, im):
        for y in range(256): d.line((0, y, 255, y), fill=(150+int(40*math.sin(y/9)), 112+int(30*math.sin(y/9)), 48))
    def lantern(d, im):
        d.rectangle((0, 0, 255, 255), fill=(255, 196, 96))
        for x in range(0, 256, 64): d.rectangle((x, 0, x+8, 255), fill=(70, 52, 30))
        d.rectangle((0, 0, 255, 14), fill=(70, 52, 30)); d.rectangle((0, 241, 255, 255), fill=(70, 52, 30))
    for name, bg, fn in [('shutter', (62, 104, 98), shutter), ('sign', (150, 44, 36), sign), ('jali', (122, 84, 52), jali),
                         ('kashi', (236, 232, 214), kashi), ('clock', (238, 232, 210), clockface), ('roof', (196, 176, 146), roof),
                         ('spice', (178, 48, 30), spice), ('shade', (60, 46, 36), shade), ('brass', (170, 128, 48), brass),
                         ('lantern', (255, 196, 96), lantern)]:
        tex(name, bg, fn)
    tex('cloth_yellow', (190, 146, 40), lambda d, im: [d.line((x, 0, x, 255), fill=(214, 172, 66), width=3) for x in range(0, 256, 24)])
    Image.open(texture/'stone.png').save(texture/'eave.png')
    extra = ['eave', 'plaster_ochre', 'plaster_rose', 'shutter', 'sign', 'jali', 'kashi', 'clock', 'roof', 'spice', 'shade', 'brass', 'lantern', 'cloth_yellow']
    env = SOURCE/'env/sandstone'; env.mkdir(parents=True, exist_ok=True)
    gradient = np.zeros((256,256,3), dtype=np.uint8)
    for y in range(256):
        t = y/255; gradient[y,:,:] = np.array([101,150,188])*(1-t)+np.array([216,202,172])*t
    for face in ('rt','lf','ft','bk'): Image.fromarray(gradient).save(env/f'sky_{face}.tga')
    Image.new('RGB', (256,256), (101,150,188)).save(env/'sky_up.tga')
    Image.new('RGB', (256,256), (216,202,172)).save(env/'sky_dn.tga')
    Image.fromarray(gradient).save(texture/'sky.png')
    scripts = SOURCE/'scripts'; scripts.mkdir(parents=True, exist_ok=True)
    shader = ['textures/skies/sandstone','{','qer_editorimage textures/sandstone/sky.png',
              'surfaceparm sky','surfaceparm noimpact','surfaceparm nolightmap',
              'q3map_sun 1 0.93 0.82 45 135 55','q3map_skylight 22 3','skyparms env/sandstone/sky - -','}']
    for name in ('plaster', *palette, *extra):
        shader += [f'textures/sandstone/{name}','{',f'qer_editorimage textures/sandstone/{name}.png']
        if name in NONSOLID: shader += ['surfaceparm nonsolid']
        shader += ['{',f'map textures/sandstone/{name}.png','rgbGen identity','}','}']
    (scripts/'sandstone.shader').write_text('\n'.join(shader)+'\n')
    (scripts/'common.shader').write_text('textures/common/clip\n{\nsurfaceparm nodraw\nsurfaceparm playerclip\n}\n')
    (scripts/'shaderlist.txt').write_text('sandstone\ncommon\n')


def author():
    ground(); boundary(); chowk(); bazaar(); block_a(); block_b(); stairs(); baoli(); haveli(); decorate()
    for x, y, z, yaw in SPAWNS:
        entity('info_player_deathmatch', P(x, y, z+.65), angle=str(yaw))
    for x, y, z in PICKUPS:
        entity('item_health_medium', P(x, y, z+.65))
    props = dict(classname='worldspawn', message='Sandstone Quarter', _blocksize='8192',
                 _description='A walled-city quarter: clock-tower chowk, Kinari bazaar, bazaar roofs, a sunken baoli and a haveli courtyard.')
    MAP.write_text('// Game: Boltrunner\n// Format: Quake3 (Valve)\n{\n'+'\n'.join(f'"{k}" "{v}"' for k, v in props.items())+'\n'
                   + '\n'.join(BRUSHES)+'\n}\n'
                   + '{\n"classname" "func_illusionary"\n"_cs" "1"\n'+'\n'.join(VISUAL)+'\n}\n'
                   + '\n'.join(ENTITIES)+'\n')
    (SOURCE/'layout.json').write_text(json.dumps(dict(
        scale='metres', bounds=[W, L], heights=dict(ground=0, roof=ROOF, passage_ceiling=CEIL, baoli=PIT),
        zones=ZONES, stairs=STAIRS, landings=LANDINGS, spawns=SPAWNS, pickups=PICKUPS, routes=ROUTES, tricks=TRICKS), indent=2)+'\n')
    print(f'Authored {len(BRUSHES)} solid + {len(VISUAL)} visual brushes, {len(ENTITIES)} entities -> {MAP}')


if __name__ == '__main__':
    materials(); author()
