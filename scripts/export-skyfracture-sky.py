#!/usr/bin/env python3
"""Project Skyfracture's authored panorama into the Quake cubemap convention."""
from pathlib import Path
import json
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
REGISTRY = json.loads((ROOT / 'assets/maps.json').read_text())
SOURCE = ROOT / 'assets' / next(m['source'] for m in REGISTRY['maps'] if m['id'] == 'skyfracture')
SIZE = 512


def sample_panorama(pixels, directions):
    directions = directions / np.linalg.norm(directions, axis=-1, keepdims=True)
    height, width = pixels.shape[:2]
    # Quake Z is up. Azimuth 0 points along +X, counterclockwise toward +Y.
    x = (np.arctan2(directions[..., 1], directions[..., 0]) / (2 * np.pi) + .5) * width - .5
    y = np.clip((.5 - np.arcsin(directions[..., 2]) / np.pi) * height - .5, 0, height - 1)
    x0, y0 = np.floor(x).astype(int), np.floor(y).astype(int)
    tx, ty = (x - x0)[..., None], (y - y0)[..., None]
    a = pixels[y0, x0 % width] * (1-tx) + pixels[y0, (x0+1) % width] * tx
    b = pixels[np.minimum(y0+1, height-1), x0 % width] * (1-tx) + pixels[np.minimum(y0+1, height-1), (x0+1) % width] * tx
    return np.clip(a * (1-ty) + b * ty, 0, 255).astype(np.uint8)


def main():
    image = Image.open(ROOT / 'assets/skyfracture/source/sky.png').convert('RGB')
    assert image.width == image.height * 2, 'Expected a 2:1 equirectangular panorama'
    pixels = np.asarray(image, dtype=float)
    corners = np.array([
        [[1,-1,1],[1,-1,-1],[1,1,-1]], [[-1,1,1],[-1,1,-1],[-1,-1,-1]],
        [[1,1,1],[1,1,-1],[-1,1,-1]], [[-1,-1,1],[-1,-1,-1],[1,-1,-1]],
        [[-1,-1,1],[1,-1,1],[1,1,1]], [[1,-1,-1],[-1,-1,-1],[-1,1,-1]],
    ])
    coords = np.array([[[0,1],[1,1],[1,0]], [[1,0],[0,0],[0,1]],
                       [[1,1],[1,0],[0,0]], [[0,0],[0,1],[1,1]],
                       [[0,1],[1,1],[1,0]], [[0,1],[1,1],[1,0]]])
    u,v = np.meshgrid((np.arange(SIZE)+.5)/SIZE, (np.arange(SIZE)+.5)/SIZE)
    uv = np.stack([u,v],axis=-1)
    destination = SOURCE / 'env/skyfracture'
    destination.mkdir(parents=True,exist_ok=True)
    transforms = [('rt',False,False,True), ('lf',True,True,True), ('bk',False,True,False),
                  ('ft',True,False,False), ('up',False,False,True), ('dn',False,False,True)]
    for i,(name,flipx,flipy,transpose) in enumerate(transforms):
        a,b,c = corners[i]
        coefficients = (uv-coords[i,0]) @ np.linalg.inv(coords[i,1:]-coords[i,0])
        directions = a + coefficients @ np.stack([b-a,c-a])
        result = sample_panorama(pixels,directions)
        # Invert the DarkPlaces transforms consumed by map_bsp.skybox.
        if transpose: result = result.transpose(1,0,2)
        if flipy: result = result[::-1]
        if flipx: result = result[:,::-1]
        Image.fromarray(result).save(destination / f'sky_{name}.tga')
    preview = SOURCE / 'textures/skies/skyfracture.jpg'
    image.resize((512,256),Image.Resampling.LANCZOS).save(preview,quality=92)
    print(f'Exported six {SIZE} x {SIZE} sky faces and editor preview')


if __name__ == '__main__':
    main()
