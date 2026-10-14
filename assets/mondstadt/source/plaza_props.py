"""Static plaza furniture reconstructed from the original city reference."""
from pathlib import Path
import math
import shutil
import json
import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent


def prepare(world, rotate, sun, textures, first_material):
    materials = []
    meshes = []
    collision = []

    def material(name, source, *, blend=False):
        index = first_material + len(materials)
        filename = 'plaza-' + name + '.png'
        shutil.copyfile(source, textures / filename)
        image = Image.open(source)
        materials.append(dict(id=index, name='mondstadt/plaza/' + name,
                              texture=filename, glow=None, width=image.width,
                              height=image.height, emissive=False,
                              alpha=False, blend=blend))
        meshes.append([])
        return index

    trim = material('carved-stone', next((HERE / 'hd/textures').glob('*-Area_MdBuild_Wall08_Diffuse.png')))
    stone = material('stone', next((HERE / 'hd/textures').glob('*-Area_Mdbuild_Stage01_Diffuse.png')))
    # Water is generated offline so a clean import needs no previous built tree.
    yy, xx = np.mgrid[:128, :128]
    ripple = (np.sin(xx * .22 + np.sin(yy * .07)) * np.cos(yy * .17) + 1) / 2
    pixels = np.array([57, 137, 165]) + ripple[..., None] * [22, 24, 20]
    water_source = textures / 'plaza-water-source.png'
    Image.fromarray(pixels.astype('uint8')).save(water_source)
    water = material('water', water_source, blend=True)
    water_source.unlink()

    def triangle(p, uv, mat, solid=True):
        p = np.asarray(p, float)
        n = np.cross(p[1] - p[0], p[2] - p[0])
        length = np.linalg.norm(n)
        if length < 1e-8:
            return
        n /= length
        color = .195 + .035 * max(n @ sun, 0)
        for point, texcoord in zip(world(p), uv):
            meshes[mat - first_material].append([*point, *(n @ rotate),
                                                *texcoord, 0, 0,
                                                color, color, color, .72 if mat == water else 1.])
        if solid:
            collision.append(p)

    def quad(p, uv, mat, solid=True):
        for ids in ([0, 1, 2], [0, 2, 3]):
            triangle(np.asarray(p)[ids], np.asarray(uv)[ids], mat, solid)

    # The circular paving is source component 282, not the approximate map-image
    # calibration. Keep the basin concentric with that actual city footprint.
    center = np.array(json.loads((HERE / 'plaza.json').read_text())['fountain']['position_raw'])
    segments = 64

    def lathe(profile, mat=stone, *, solid=True, ornamental=False):
        profile = np.asarray(profile, float)
        distances = np.r_[0, np.cumsum(np.linalg.norm(np.diff(profile, axis=0), axis=1))]
        for j, ((r0, y0), (r1, y1)) in enumerate(zip(profile[:-1], profile[1:])):
            for i in range(segments):
                a, b = i * math.tau / segments, (i + 1) * math.tau / segments
                p = [[r0 * math.cos(a), y0, r0 * math.sin(a)],
                     [r1 * math.cos(a), y1, r1 * math.sin(a)],
                     [r1 * math.cos(b), y1, r1 * math.sin(b)],
                     [r0 * math.cos(b), y0, r0 * math.sin(b)]]
                if ornamental:
                    # Only the carved scroll band of the original trim atlas.
                    uv = [[i / 8, .57], [i / 8, .70], [(i + 1) / 8, .70], [(i + 1) / 8, .57]]
                else:
                    uv = [[i / 8, distances[j] / 2], [i / 8, distances[j + 1] / 2],
                          [(i + 1) / 8, distances[j + 1] / 2], [(i + 1) / 8, distances[j] / 2]]
                quad(np.asarray(p) + center, uv, mat, solid)

    # Closed basin/rim with a shallow walkable bottom; no solid water curtain.
    lathe([(0, .03), (4.15, .03), (4.32, .16), (4.32, .62),
           (4.21, .75), (3.92, .75), (3.82, .61), (3.82, .17), (0, .17)])
    lathe([(4.325, .22), (4.325, .56)], trim, ornamental=True)
    # Original fountain silhouette: two cups above the large ground basin.
    lathe([(0, .17), (.62, .17), (.62, .36), (.43, .48),
           (.35, 1.32), (1.85, 1.32), (2.05, 1.54), (2.10, 1.76),
           (1.90, 1.87), (.33, 1.87), (.30, 2.87), (1.04, 2.87),
           (1.23, 3.04), (1.24, 3.27), (1.08, 3.39), (.22, 3.39),
           (.20, 3.72), (0, 3.72)])
    lathe([(2.105, 1.57), (2.105, 1.75)], trim, ornamental=True)
    lathe([(1.245, 3.08), (1.245, 3.25)], trim, ornamental=True)
    # Broad reflective pool and two short translucent cascades are visual only.
    lathe([(3.81, .46), (0, .46)], water, solid=False)
    lathe([(1.91, 1.79), (0, 1.79)], water, solid=False)
    lathe([(1.08, 3.30), (0, 3.30)], water, solid=False)
    for radius, top, bottom in [(1.91, 1.79, .48), (1.08, 3.30, 1.81)]:
        for i in range(16):
            a = (i + .15) * math.tau / 16
            b = (i + .38) * math.tau / 16
            p = [[radius * math.cos(a), top, radius * math.sin(a)],
                 [radius * math.cos(a), bottom, radius * math.sin(a)],
                 [radius * math.cos(b), bottom, radius * math.sin(b)],
                 [radius * math.cos(b), top, radius * math.sin(b)]]
            quad(np.asarray(p) + center, [[0, 0], [0, 2], [.2, 2], [.2, 0]], water, False)

    chunks = [(m['id'], np.asarray(data)) for m, data in zip(materials, meshes)]
    return chunks, materials, collision
