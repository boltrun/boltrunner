#!/usr/bin/env python3
"""Build the textured, animated Cheburashka GLB from its retained OFF source."""
import json
import math
from pathlib import Path
import struct

from character_motion import leg_angles, rotation, smooth, step

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / 'assets/characters/cheburashka'


def linear(value):
    return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4


def color(rgb):
    return [*(linear(c) for c in rgb), 1.0]


def clamp(value):
    return max(0.0, min(1.0, value))


def mix(a, b, t):
    return [x * (1 - t) + y * t for x, y in zip(a, b)]


def quat(axis, angle):
    result = [0, 0, 0, math.cos(angle / 2)]
    result[axis] = math.sin(angle / 2)
    return result


def build():
    lines = ASSETS.joinpath('source.off').read_text().splitlines()
    count, faces, _ = map(int, lines[1].split())
    source = [list(map(float, line.split())) for line in lines[2:2 + count]]
    low, high = min(p[1] for p in source), max(p[1] for p in source)
    scale = 1.8 / (high - low)
    positions = [[(p[0] - 0.5) * scale, (p[1] - low) * scale, (p[2] - 0.5) * scale] for p in source]
    indices = []
    normals = [[0.0, 0.0, 0.0] for _ in positions]
    for line in lines[2 + count:2 + count + faces]:
        triangle = list(map(int, line.split()))[1:]
        if len(triangle) != 3:
            raise ValueError('The retained source must be triangulated.')
        indices.extend(triangle)
        a, b, c = [positions[index] for index in triangle]
        u, v = [b[i] - a[i] for i in range(3)], [c[i] - a[i] for i in range(3)]
        cross = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
        for index in triangle:
            normals[index] = [normals[index][i] + cross[i] for i in range(3)]
    normals = [[c / max(math.sqrt(sum(n * n for n in normal)), 1e-8) for c in normal] for normal in normals]

    brown, cream = color([0.36, 0.19, 0.09]), color([0.85, 0.66, 0.42])
    colors, joints, weights, uvs = [], [], [], []
    for (x, y, z), normal in zip(positions, normals):
        face = (x / 0.29) ** 2 + ((y - 1.42) / 0.29) ** 2
        belly = (x / 0.24) ** 2 + ((y - 0.70) / 0.32) ** 2
        ear = ((abs(x) - 0.68) / 0.24) ** 2 + ((y - 1.51) / 0.25) ** 2
        front = clamp((normal[2] + 0.1) * 3)
        patch = max(clamp((1.1 - face) * 5), clamp((1.0 - belly) * 4),
                    clamp((1.0 - ear) * 4)) * front
        shade = 0.62 + 0.48 * max(0.0, sum(a * b for a, b in zip(normal, [-0.38, 0.58, 0.72])))
        tinted = mix(brown, cream, patch)
        colors.append([*(c * shade for c in tinted[:3]), 1.0])
        uvs.append([math.atan2(x, z) / (2 * math.pi) * 4 + 2, y / 1.8 * 6])
        joint, secondary, weight = (2 if y > 1.06 else 1 if y > 0.66 else 0), 0, 1.0
        if y > 1.18 and abs(x) > 0.42:
            joint, secondary, weight = (9 if x < 0 else 10), 2, clamp((abs(x) - 0.42) / 0.2) * 0.85
        elif 0.30 < y < 1.08 and abs(x) > 0.24:
            side = 0 if x < 0 else 1
            joint, secondary, weight = 3+side, 1, clamp((abs(x)-.24)/.11)
            if y < .77:
                joint, secondary, weight = 13+side, 3+side, 1-smooth((y-.63)/.14)
            if y < .59:
                joint, secondary, weight = 15+side, 13+side, 1-smooth((y-.49)/.10)
        if y < 0.60 and (y < 0.30 or abs(x) < 0.29):
            # Blend the hip, thigh, shin and foot across their actual joint heights.
            leg = 1 - smooth((y - .44) / .16)
            knee = 1 - smooth((y - .20) / .11)
            foot = 1 - smooth((y - .10) / .10)
            side = 0 if x < 0 else 1
            joints.append([0, 5 + side, 7 + side, 11 + side])
            weights.append([1-leg, leg*(1-knee), leg*knee*(1-foot), leg*knee*foot])
        else:
            joints.append([joint, secondary, 0, 0])
            weights.append([weight, 1 - weight, 0, 0])
    primitive_indices = [indices, [], [], []]

    def ellipsoid(center, radii, material):
        start = len(positions)
        segments, rings = 24, 12
        for ring in range(rings + 1):
            theta = math.pi * ring / rings
            for segment in range(segments + 1):
                phi = 2 * math.pi * segment / segments
                unit = [math.sin(theta) * math.cos(phi), math.cos(theta), math.sin(theta) * math.sin(phi)]
                positions.append([center[i] + radii[i] * unit[i] for i in range(3)])
                normal = [unit[i] / radii[i] for i in range(3)]
                length = math.sqrt(sum(n * n for n in normal))
                normals.append([n / length for n in normal])
                colors.append([1, 1, 1, 1]); uvs.append([segment / segments, ring / rings])
                joints.append([2, 0, 0, 0]); weights.append([1, 0, 0, 0])
        for ring in range(rings):
            for segment in range(segments):
                a = start + ring * (segments + 1) + segment
                b = a + segments + 1
                primitive_indices[material].extend([a, a + 1, b, a + 1, b + 1, b])

    for x in [-0.15, 0.15]:
        ellipsoid([x, 1.49, 0.326], [0.083, 0.10, 0.063], 1)
        ellipsoid([x + 0.012, 1.485, 0.386], [0.049, 0.071, 0.023], 2)
        ellipsoid([x - 0.004, 1.515, 0.408], [0.014, 0.017, 0.009], 1)
    ellipsoid([0, 1.355, 0.362], [0.056, 0.041, 0.034], 3)
    for i in range(17):
        x = -0.065 + i * 0.13 / 16
        ellipsoid([x, 1.275 + 3 * x * x, 0.347 - 1.3 * x * x], [0.006, 0.006, 0.006], 3)

    document = {'asset': {'version': '2.0', 'generator': 'Boltrunner Cheburashka builder'},
                'scene': 0, 'scenes': [{'nodes': [17]}], 'nodes': [], 'meshes': [],
                'skins': [], 'animations': [], 'accessors': [], 'bufferViews': [],
                'materials': [], 'images': [], 'textures': [], 'samplers': [{'magFilter': 9729, 'minFilter': 9729, 'wrapS': 10497, 'wrapT': 10497}],
                'extensionsUsed': ['KHR_materials_unlit']}
    payload = bytearray()

    def view(data, target=None):
        payload.extend(b'\0' * (-len(payload) % 4))
        index = len(document['bufferViews'])
        entry = {'buffer': 0, 'byteOffset': len(payload), 'byteLength': len(data)}
        if target:
            entry['target'] = target
        document['bufferViews'].append(entry); payload.extend(data)
        return index

    def accessor(values, kind, component=5126, bounds=False, target=None):
        size = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4, 'MAT4': 16}[kind]
        rows = [[value] for value in values] if size == 1 else values
        flat = [c for row in rows for c in row]
        data = struct.pack('<' + {5126: 'f', 5123: 'H', 5125: 'I'}[component] * len(flat), *flat)
        entry = {'bufferView': view(data, target), 'componentType': component, 'count': len(values), 'type': kind}
        if bounds:
            entry.update(min=[min(row[i] for row in rows) for i in range(size)], max=[max(row[i] for row in rows) for i in range(size)])
        document['accessors'].append(entry)
        return len(document['accessors']) - 1

    document['images'].append({'bufferView': view(ASSETS.joinpath('fur.png').read_bytes()), 'mimeType': 'image/png'})
    document['textures'].append({'source': 0, 'sampler': 0})
    for name, rgb in [('Fur', [1, 1, 1]), ('Eyes white', [0.93, 0.91, 0.83]), ('Pupils', [0.025, 0.018, 0.012]), ('Nose', [0.13, 0.06, 0.035])]:
        material = {'name': name, 'pbrMetallicRoughness': {'baseColorFactor': color(rgb), 'metallicFactor': 0, 'roughnessFactor': 0.9}, 'extensions': {'KHR_materials_unlit': {}}}
        if name == 'Fur':
            material['pbrMetallicRoughness']['baseColorTexture'] = {'index': 0}
        document['materials'].append(material)
    attributes = {name: accessor(values, kind, component, name == 'POSITION', 34962) for name, values, kind, component in [
        ('POSITION', positions, 'VEC3', 5126), ('NORMAL', normals, 'VEC3', 5126), ('TEXCOORD_0', uvs, 'VEC2', 5126),
        ('COLOR_0', colors, 'VEC4', 5126), ('JOINTS_0', joints, 'VEC4', 5123), ('WEIGHTS_0', weights, 'VEC4', 5126)]}
    document['meshes'].append({'name': 'Cheburashka', 'primitives': [
        {'attributes': attributes, 'indices': accessor(group, 'SCALAR', 5125, target=34963), 'material': i}
        for i, group in enumerate(primitive_indices)]})
    pivots = [(0, .56, 0), (0, .87, 0), (0, 1.38, 0), (-.29, .86, -.10), (.29, .86, -.10),
              (-.15, .50, 0), (.15, .50, 0), (-.15, .25, 0), (.15, .25, 0), (-.68, 1.49, 0), (.68, 1.49, 0),
              (-.15, .10, .04), (.15, .10, .04),
              (-.38, .69, -.12), (.38, .69, -.12), (-.44, .53, -.12), (.44, .53, -.12)]
    parents = [None, 0, 1, 1, 1, 0, 0, 5, 6, 2, 2, 7, 8, 3, 4, 13, 14]
    names = ['Hips', 'Chest', 'Head', 'Left arm', 'Right arm', 'Left leg', 'Right leg', 'Left knee', 'Right knee', 'Left ear', 'Right ear', 'Left foot', 'Right foot', 'Left elbow', 'Right elbow', 'Left hand', 'Right hand']
    for i, (point, parent, name) in enumerate(zip(pivots, parents, names)):
        translation = list(point) if parent is None else [point[a] - pivots[parent][a] for a in range(3)]
        children = [child for child, owner in enumerate(parents) if owner == i]
        document['nodes'].append({'name': name, 'translation': translation, 'children': children})
    document['nodes'].extend([{'name': 'Cheburashka rig', 'children': [0, 18]}, {'name': 'Cheburashka body', 'mesh': 0, 'skin': 0}])
    matrices = [[1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -x, -y, -z, 1] for x, y, z in pivots]
    document['skins'].append({'joints': list(range(17)), 'skeleton': 0, 'inverseBindMatrices': accessor(matrices, 'MAT4')})

    def pose(name, t):
        phase = 2 * math.pi * t
        moving = name in ['Walk', 'Run']
        running = name == 'Run'
        sway = math.sin(phase)
        bob = (-.055 if running else -.035) + (.010 if running else .007) * math.cos(2*phase) if moving else 0
        translations = {0: [(.010 * sway if moving else 0), .56 + bob, 0]}
        rotations = {joint: rotation() for joint in range(17)}
        rotations[0] = rotation(y=.04*sway if moving else 0, z=.026*sway if moving else 0)
        rotations[1] = rotation(x=.09 if running else .02 if moving else .008*math.sin(phase),
                                y=-.055*sway if moving else 0)
        rotations[2] = rotation(x=-.055 if running else -.006*math.sin(phase),
                                y=.025*sway if moving else .012*math.sin(phase),
                                z=-.012*sway if moving else .010*math.sin(phase))
        for side, offset, sign in [(0, 0.0, -1), (1, .5, 1)]:
            leg_phase = phase + offset*2*math.pi
            forward, lift, flight = step(t + offset, .12 if running else .085, .075 if running else .04)
            # This model faces +Z. Keep each sole level during its planted phase.
            hip, knee = leg_angles(forward + .04, .40 + bob - lift, .25, math.hypot(.15, .04)) if moving else (0, 0)
            # The shin's rest vector already inclines forwards by atan2(.04,.15).
            shin_rest = math.atan2(.04, .15)
            thigh = -hip
            shin = -knee + shin_rest if moving else 0
            rotations[5+side] = rotation(x=thigh)
            rotations[7+side] = rotation(x=shin)
            rotations[11+side] = rotation(x=-thigh-shin + flight*.12)
            rotations[3+side] = rotation(x=math.cos(leg_phase)*(.35 if running else .20) if moving else .018*math.sin(phase),
                                         z=-sign*(.06 if running else .018))
            rotations[9+side] = rotation(z=sign*(.055 if running else .035 if moving else .018)*math.sin(phase-.7),
                                         x=.02*math.sin(2*phase-.8) if moving else 0)
            if name in ['Jump', 'Dash']:
                rotations[5+side] = rotation(x=-.20 if name == 'Jump' else -.45 if side == 0 else .15)
                rotations[7+side] = rotation(x=.50 if name == 'Jump' else .65)
                rotations[11+side] = rotation(x=-.3)
                rotations[3+side] = rotation(x=.28 if name == 'Jump' else .5, z=-sign*.08)
                rotations[1] = rotation(x=.08 if name == 'Jump' else .24)
                rotations[2] = rotation(x=-.07)
            if name == 'Death':
                fall = smooth(t)
                rotations[0] = rotation(x=1.4*fall, z=.12*fall)
                translations[0] = [0, .56-.32*fall, .25*fall]
                rotations[3+side] = rotation(x=-.25*fall)
                rotations[7+side] = rotation(x=.3*fall)
        return rotations, translations

    for name, duration in [('Idle', 3.4), ('Walk', .92), ('Run', .58), ('Jump', 1.0), ('Dash', .6), ('Death', 1.2)]:
        frames = 48
        times = [duration * frame / frames for frame in range(frames + 1)]
        inputs = accessor(times, 'SCALAR', bounds=True)
        samples = [pose(name, frame/frames) for frame in range(frames + 1)]
        animation = {'name': name, 'samplers': [], 'channels': []}
        for path, group, kind in [('rotation', 0, 'VEC4'), ('translation', 1, 'VEC3')]:
            for joint in samples[0][group]:
                values = [sample[group][joint] for sample in samples]
                animation['channels'].append({'sampler': len(animation['samplers']), 'target': {'node': joint, 'path': path}})
                animation['samplers'].append({'input': inputs, 'output': accessor(values, kind), 'interpolation': 'LINEAR'})
        document['animations'].append(animation)
    document['buffers'] = [{'byteLength': len(payload)}]
    encoded = json.dumps(document, separators=(',', ':')).encode()
    encoded += b' ' * (-len(encoded) % 4); payload.extend(b'\0' * (-len(payload) % 4))
    data = (struct.pack('<III', 0x46546C67, 2, 28 + len(encoded) + len(payload))
            + struct.pack('<II', len(encoded), 0x4E4F534A) + encoded
            + struct.pack('<II', len(payload), 0x004E4942) + payload)
    ASSETS.joinpath('model.glb').write_bytes(data)
    print(f'Cheburashka: {len(positions)} vertices, {sum(len(group) for group in primitive_indices) // 3} triangles, {len(data):,} bytes')


if __name__ == '__main__':
    build()
