#!/usr/bin/env python3
"""Compile a registered editable Quake 3 map into Boltrunner's current runtime assets.

Normal builds are offline. --fetch imports missing, checksum-verified upstream
sources at the pinned commit. Requires q3map2 (setup-map-tools.sh), numpy and Pillow.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import struct
import urllib.request
from collections import defaultdict
from pathlib import Path

import numpy as np
from PIL import Image

from map_bake import bake
from map_bsp import Bsp, skybox

ROOT = Path(__file__).resolve().parents[1]
MAP = SOURCE = BUILT = MAP_FILE = None
MAP_ID = ""
UPSTREAM = ""
SCALE = 1 / 40  # 40 Quake units per metre; Xonotic's walking speed is comparable.
ROTATE = np.array([[1, 0, 0], [0, 0, 1], [0, -1, 0]], dtype=float)
FETCH = False
TREE = {}
USED = set()


def source(path):
    USED.add(path)
    local = SOURCE / path
    if not local.exists():
        if not FETCH:
            raise RuntimeError(f"Missing source {path}; run this script with --fetch")
        data = urllib.request.urlopen(UPSTREAM + path, timeout=60).read()
        expected = TREE[path]["sha"]
        actual = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        if actual != expected:
            raise RuntimeError(f"Upstream checksum mismatch: {path}")
        local.parent.mkdir(parents=True, exist_ok=True)
        local.write_bytes(data)
        print(f"Fetched {path}", flush=True)
    return local


def vec(text):
    return np.array([float(x) for x in text.split()])


def world(p):
    return (ROTATE @ np.asarray(p)) * SCALE


def normal(v):
    length = np.linalg.norm(v)
    return v / length if length > 1e-10 else np.zeros(3)


class MapParser:
    def __init__(self, text):
        self.tokens = re.findall(r'"[^"\n]*"|[{}()\[\]]|[^\s{}()\[\]]+', re.sub(r'//[^\n]*', '', text))
        self.i = 0

    def take(self, expected=None):
        value = self.tokens[self.i]
        self.i += 1
        if expected is not None and value != expected:
            raise ValueError(f"Expected {expected}, got {value} near token {self.i}")
        return value.strip('"')

    def point(self, count):
        self.take('(')
        result = np.array([float(self.take()) for _ in range(count)])
        self.take(')')
        return result

    def parse(self):
        entities = []
        while self.i < len(self.tokens):
            self.take('{')
            props, brushes, patches = {}, [], []
            while self.tokens[self.i] != '}':
                if self.tokens[self.i] != '{':
                    key = self.take()
                    props[key] = self.take()
                    continue
                self.take('{')
                if self.tokens[self.i] == 'patchDef2':
                    self.take('patchDef2')
                    self.take('{')
                    material = self.take()
                    width, height, *_ = self.point(5).astype(int)
                    self.take('(')
                    points = []
                    for _ in range(width):
                        self.take('(')
                        points.append([self.point(5) for _ in range(height)])
                        self.take(')')
                    self.take(')')
                    self.take('}')
                    patches.append((material, np.array(points)))
                else:
                    faces = []
                    while self.tokens[self.i] != '}':
                        points = [self.point(3) for _ in range(3)]
                        material = self.take()
                        uv_axes = []
                        for _ in range(2):
                            self.take('[')
                            uv_axes.append([float(self.take()) for _ in range(4)])
                            self.take(']')
                        self.take()  # Editor rotation hint; the explicit axes define the UVs.
                        uv = (uv_axes, float(self.take()), float(self.take()))
                        flags = [int(self.take()) for _ in range(3)]
                        n = normal(np.cross(points[2] - points[0], points[1] - points[0]))
                        if np.linalg.norm(n) < .5:
                            raise ValueError("Degenerate brush plane")
                        faces.append((n, np.dot(n, points[0]), material, uv, flags))
                    brushes.append(faces)
                self.take('}')
            self.take('}')
            entities.append((props, brushes, patches))
        return entities


def polygon(face, brush):
    n, d = face[:2]
    tangent = normal(np.cross(n, [0, 0, 1] if abs(n[2]) < .9 else [0, 1, 0]))
    bitangent = np.cross(n, tangent)
    center = n * d
    points = [center + (a * tangent + b * bitangent) * 32768 for a, b in [(-1, -1), (1, -1), (1, 1), (-1, 1)]]
    for clip in brush:
        cn, cd = clip[:2]
        if np.dot(n, cn) > .999999 and abs(d - cd) < .01:
            continue
        result = []
        for a, b in zip(points, points[1:] + points[:1]):
            da, db = np.dot(cn, a) - cd, np.dot(cn, b) - cd
            if da <= .001:
                result.append(a)
            if (da > .001) != (db > .001):
                result.append(a + (b - a) * (da / (da - db)))
        points = result
        if len(points) < 3:
            return []
    return points


def shader_blocks(text):
    text = re.sub(r'//[^\n]*', '', text)
    blocks = {}
    for match in re.finditer(r'(?m)^\s*((?:textures|models)/[^\s{}]+)\s*\{', text):
        depth, end = 1, match.end()
        while depth and end < len(text):
            depth += (text[end] == '{') - (text[end] == '}')
            end += 1
        blocks[match[1]] = text[match.end():end - 1]
    return blocks


class Compiler:
    def __init__(self, entities):
        self.entities = entities
        self.shaders = {}
        for path in sorted((SOURCE / 'scripts').glob('*.shader')):
            self.shaders.update(shader_blocks(source(str(path.relative_to(SOURCE))).read_text()))
        self.materials = {}
        self.collision = []
        self.triggers = []
        self.spawns = []
        self.pickup_spawns = []
        self.warps = []
        self.targets = {p['targetname']: p for p, _, _ in entities if 'targetname' in p}
        self.counts = defaultdict(int)

    def texture(self, name):
        name = re.sub(r'\.(tga|jpg|png)$', '', name, flags=re.I)
        for ext in ['.tga', '.jpg', '.png']:
            candidate = name + ext
            if (SOURCE / candidate).exists() or candidate in TREE:
                return source(candidate)
        raise ValueError(f"Texture source not found: {name}")

    def sky_prefix(self):
        names = {face[2].removeprefix('textures/')
                 for _, brushes, _ in self.entities for brush in brushes for face in brush
                 if face[2].removeprefix('textures/').startswith('skies/')}
        shaders = {}
        for path in sorted((SOURCE / 'scripts').glob('*.shader')):
            shaders.update(shader_blocks(source(str(path.relative_to(SOURCE))).read_text()))
        prefixes = set()
        for name in names:
            match = re.search(r'(?im)^\s*skyparms\s+(\S+)', shaders.get('textures/' + name, ''))
            if not match or match[1] == '-':
                raise ValueError(f'Sky material has no cubemap: {name}')
            prefixes.add(match[1])
        if not prefixes:
            return None
        if len(prefixes) != 1:
            raise ValueError('Map must use exactly one sky cubemap')
        return prefixes.pop()

    def material(self, name):
        if not name.startswith(('models/', 'textures/')):
            name = 'textures/' + name
        if name in self.materials:
            return self.materials[name]
        shader = self.shaders.get(name, '')
        texture = name
        editor = re.search(r'(?im)^\s*qer_editorimage\s+(\S+)', shader)
        diffuse = re.search(r'(?im)^\s*map\s+((?:textures|models)/\S+)', shader)
        if diffuse:
            texture = diffuse[1]
        elif editor:
            texture = editor[1]
        image = Image.open(self.texture(texture))
        filename = name.replace('/', '__') + '.png'
        out = BUILT / 'textures' / filename
        out.parent.mkdir(parents=True, exist_ok=True)
        image.save(out, optimize=True)
        glow = None
        # Preserve authored emission masks where available (light strips and pads).
        base = re.sub(r'\.(tga|jpg|png)$', '', texture)
        glow_name = base + '_glow.tga'
        if (SOURCE / glow_name).exists() or glow_name in TREE:
            glow = filename.replace('.png', '__glow.png')
            Image.open(source(glow_name)).save(BUILT / 'textures' / glow, optimize=True)
        emissive = 'lava' in name or 'warpzone' in name or 'energy_' in name or 'jumpglow' in name
        alpha = 'alphaFunc' in shader or 'alphafunc' in shader or 'grate' in name
        blend = 'warpzone' in name or 'energy_' in name or 'jumpglow' in name or 'misc-glass' in name
        result = dict(id=len(self.materials), name=name, texture=filename, glow=glow,
                      width=image.width, height=image.height, emissive=emissive, alpha=alpha, blend=blend)
        self.materials[name] = result
        return result

    def visible(self, material):
        return not material.startswith(('common/', 'skies/', 'radiant/'))

    def solid(self, materials):
        if any('lava' in m for m in materials):
            return False
        return any(m not in ['common/caulk', 'common/hint', 'common/donotenter', 'common/trigger', 'common/weapclip', 'common/origin']
                   and not m.startswith('skies/') for m in materials) or 'common/clip' in materials or all(m == 'common/caulk' for m in materials)

    def triangle(self, points):
        if np.linalg.norm(np.cross(points[1] - points[0], points[2] - points[0])) >= 1e-9:
            self.collision.append(points)

    def brush(self, brush, props):
        classname = props['classname']
        polys = [polygon(face, brush) for face in brush]
        all_points = [p for poly in polys for p in poly]
        if not all_points:
            raise ValueError(f"Empty brush in {classname}")
        transformed = np.array([world(p) for p in all_points])
        low, high = transformed.min(axis=0), transformed.max(axis=0)
        materials = [face[2] for face in brush]
        trigger = classname.startswith('trigger_') or any('lava' in m for m in materials)
        solid = not trigger and self.solid(materials)
        planes = [[*(ROTATE @ face[0]), face[1] * SCALE] for face in brush]
        center = (low + high) / 2
        if trigger:
            kind = classname.removeprefix('trigger_') if classname.startswith('trigger_') else 'hurt'
            if kind not in ['push', 'teleport', 'warpzone', 'hurt']:
                return
            data = dict(kind=kind, planes=planes)
            if kind == 'push':
                target = world(vec(self.targets[props['target']]['origin']))
                rise = max(target[1] - high[1], .5)
                vy = math.sqrt(2 * 24 * rise)
                velocity = (target - center) / (vy / 24)
                velocity[1] = vy
                data['velocity'] = velocity.tolist()
            elif kind == 'teleport':
                target = self.targets[props['target']]
                data['destination'] = (world(vec(target['origin'])) - np.array([0, 24 * SCALE, 0])).tolist()
                data['rotation'] = math.radians(float(target.get('angle', 0))) - math.pi / 2
            elif kind == 'warpzone':
                faces = [(face, poly) for face, poly in zip(brush, polys) if face[2].endswith('/wavy') and len(poly) >= 3]
                if len(faces) != 1:
                    raise ValueError('Warpzone must have one authored portal surface')
                face, poly = faces[0]
                self.warps.append((data, props, world(np.mean(poly, axis=0)), ROTATE @ face[0]))
            self.triggers.append(data)
        if solid:
            for poly in polys:
                for i in range(1, len(poly) - 1):
                    self.triangle([world(poly[j]) for j in [0, i, i + 1]])
        self.counts['brushes'] += 1

    def patch(self, name, controls):
        # The collision mesh tessellates the same quadratic surfaces as the BSP.
        if self.solid([name]):
            weights = np.array([[(1 - u) ** 2, 2 * u * (1 - u), u * u] for u in np.linspace(0, 1, 5)])
            for x in range(0, len(controls) - 2, 2):
                for y in range(0, len(controls[0]) - 2, 2):
                    grid = np.einsum('ai,bj,ijk->abk', weights, weights, controls[x:x + 3, y:y + 3, :3])
                    grid = grid @ ROTATE.T * SCALE
                    for i in range(4):
                        for j in range(4):
                            self.triangle([grid[i,j], grid[i+1,j+1], grid[i,j+1]])
                            self.triangle([grid[i,j], grid[i+1,j], grid[i+1,j+1]])
        self.counts['patches'] += 1

    def model(self, props):
        path = props['model']
        self.counts['models'] += 1
        if 'crate' not in path:
            return
        data = source(path).read_bytes()
        header = struct.unpack_from('<4si64s9i', data)
        if header[0] != b'IDP3' or header[1] != 15:
            raise ValueError(f'Unsupported MD3: {path}')
        offset = header[10]
        origin = vec(props['origin'])
        scale = vec(props['modelscale_vec']) if 'modelscale_vec' in props else np.ones(3) * float(props.get('modelscale', 1))
        pitch, yaw, roll = np.radians(vec(props.get('angles', f"0 {props.get('angle', '0')} 0")))
        cx, sx, cy, sy, cz, sz = math.cos(roll), math.sin(roll), math.cos(pitch), math.sin(pitch), math.cos(yaw), math.sin(yaw)
        rotation = np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]]) @ np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]]) @ np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]])
        for _ in range(header[6]):
            surface = struct.unpack_from('<4s64s10i', data, offset)
            vertices = []
            for i in range(surface[5]):
                x, y, z = struct.unpack_from('<3h', data, offset + surface[10] + i * 8)
                vertices.append(world(origin + rotation @ (np.array([x, y, z]) / 64 * scale)))
            for i in range(surface[6]):
                a, b, c = struct.unpack_from('<3i', data, offset + surface[7] + i * 12)
                # MD3 faces are clockwise; Bevy culls clockwise faces. Mirrored
                # models reverse the transform's handedness a second time.
                indices = (a, c, b) if np.prod(scale) > 0 else (a, b, c)
                self.triangle([vertices[j] for j in indices])
            offset += surface[11]

    def collect(self):
        for props, brushes, patches in self.entities:
            classname = props['classname']
            # Import only deathmatch geometry when the author specifies mode filters.
            if props.get('gametypefilter', '').startswith('+') and 'dm' not in props['gametypefilter']:
                continue
            if classname == 'info_player_deathmatch':
                self.spawns.append(dict(position=(world(vec(props['origin'])) - np.array([0, 24 * SCALE, 0])).tolist(),
                                        yaw=math.radians(float(props.get('angle', 0))) - math.pi / 2))
            elif classname.startswith('item_health_'):
                self.pickup_spawns.append(world(vec(props['origin'])).tolist())
            elif classname in ['misc_model', 'misc_gamemodel']:
                self.model(props)
            if classname in ['worldspawn', 'func_group', 'func_wall', 'func_bobbing', 'trigger_push', 'trigger_teleport', 'trigger_warpzone', 'trigger_hurt']:
                for brush in brushes:
                    self.brush(brush, props)
                for name, patch in patches:
                    self.patch(name, patch)
        for data, props, entrance, normal in self.warps:
            if props.get('target'):
                matches = [w for w in self.warps if w[1].get('targetname') == props['target']]
            else:
                matches = [w for w in self.warps if props.get('targetname')
                           and w[1].get('target') == props['targetname']]
            if len(matches) != 1 or matches[0][0] is data:
                raise ValueError(f'Warpzone must resolve to one other portal: {props}')
            _, _, target, target_n = matches[0]
            data['kind'] = 'warp'
            data['entrance'] = entrance.tolist()
            data['normal'] = normal.tolist()
            data['destination'] = target.tolist()
            data['exit_normal'] = target_n.tolist()
            data['rotation'] = math.atan2(-target_n[0], -target_n[2]) - math.atan2(normal[0], normal[2])
        if not self.spawns:
            raise ValueError('Map must contain at least one info_player_deathmatch')
        if not self.collision:
            raise ValueError('Map must contain solid collision geometry')

    def compile(self, q3map2):
        self.collect()
        prefix = self.sky_prefix()
        bsp = Bsp(bake(self, q3map2, MAP_FILE, SOURCE, BUILT))
        BUILT.mkdir(parents=True, exist_ok=True)
        vertices, indices, lookup = [], [], {}
        for triangle in self.collision:
            face = []
            for point in triangle:
                key = tuple(round(float(v), 5) for v in point)
                if key not in lookup:
                    lookup[key] = len(vertices)
                    vertices.append(key)
                face.append(lookup[key])
            if len(set(face)) == 3:
                indices.append(face)
        with (BUILT / 'collision.bin').open('wb') as f:
            f.write(struct.pack('<2I', len(vertices), len(indices)))
            f.write(np.asarray(vertices, dtype='<f4').tobytes())
            f.write(np.asarray(indices, dtype='<u4').tobytes())
        lightmaps, render_triangles = bsp.render(self, BUILT, ROTATE, SCALE)
        metadata = dict(id=MAP_ID, name=self.entities[0][0].get('message', MAP_ID), units_per_metre=40, spawns=self.spawns, pickup_spawns=self.pickup_spawns, triggers=self.triggers, pixelated=False, crystals=[],
                        lightmaps=lightmaps, sky=skybox(SOURCE, BUILT, prefix) if prefix else None, materials=list(self.materials.values()), counts=dict(self.counts),
                        collision_triangles=len(indices), render_triangles=render_triangles)
        used_textures = {*lightmaps}
        if metadata['sky']:
            used_textures.add(metadata['sky'])
        for material in self.materials.values():
            used_textures.add(material['texture'])
            if material['glow']:
                used_textures.add(material['glow'])
        for previous in (BUILT / 'textures').glob('*.png'):
            if previous.name not in used_textures:
                previous.unlink()
        (BUILT / 'map.json').write_text(json.dumps(metadata, indent=2) + '\n')
        # Keep original import checksums stable when artists edit source assets.
        if FETCH:
            provenance = json.loads((MAP / 'upstream.json').read_text())
            for path in sorted(USED):
                provenance['files'].setdefault(path, hashlib.sha256((SOURCE / path).read_bytes()).hexdigest())
            (MAP / 'upstream.json').write_text(json.dumps(provenance, indent=2) + '\n')
        print(json.dumps({k: metadata[k] for k in ['counts', 'collision_triangles', 'render_triangles']}, indent=2))
        print(f"Compiled {len(self.spawns)} spawns, {len(self.triggers)} triggers, {len(self.materials)} materials")


def configure(map_id):
    global MAP, SOURCE, BUILT, MAP_FILE, MAP_ID, USED
    registry = json.loads((ROOT / 'assets/maps.json').read_text())
    entry = next((m for m in registry['maps'] if m['id'] == map_id), None)
    if entry is None:
        raise ValueError(f'Unknown map: {map_id}')
    MAP_ID = map_id
    MAP = ROOT / 'assets' / map_id
    SOURCE = ROOT / 'assets' / entry['source']
    BUILT = MAP / 'built'
    MAP_FILE = MAP / f'{map_id}.map'
    USED = set()


def main():
    global FETCH, TREE, UPSTREAM
    registry = json.loads((ROOT / 'assets/maps.json').read_text())
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('map', choices=[m['id'] for m in registry['maps']])
    parser.add_argument('--fetch', action='store_true', help='Fetch this map resource root from its pinned upstream manifest')
    parser.add_argument('--q3map2', default=os.environ.get('Q3MAP2', str(ROOT / '.tools/q3map2/q3map2')))
    args = parser.parse_args()
    configure(args.map)
    FETCH = args.fetch
    if FETCH:
        manifest = MAP / 'upstream.json'
        if not manifest.exists():
            parser.error('This map has no upstream manifest; its sources must already be present')
        provenance = json.loads(manifest.read_text())
        repository = provenance['repository'].removeprefix('https://github.com/')
        commit = provenance['commit']
        UPSTREAM = f'https://raw.githubusercontent.com/{repository}/{commit}/'
        tree = json.loads(urllib.request.urlopen(f'https://api.github.com/repos/{repository}/git/trees/{commit}?recursive=1').read())
        TREE = {entry['path']: entry for entry in tree['tree']}
        for path in provenance['files']:
            source(path)
    entities = MapParser(MAP_FILE.read_text()).parse()
    Compiler(entities).compile(args.q3map2)


if __name__ == '__main__':
    main()
