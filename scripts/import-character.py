#!/usr/bin/env python3
"""Register a self-contained GLB/VRM in the server's character catalog."""
import argparse
import copy
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import urllib.request

ROOT = Path(__file__).resolve().parent.parent


def transformed(point, node):
    if 'matrix' in node:
        m = node['matrix']
        return [sum(m[column * 4 + row] * point[column] for column in range(3)) + m[12 + row] for row in range(3)]
    v = [point[i] * node.get('scale', [1, 1, 1])[i] for i in range(3)]
    x, y, z, w = node.get('rotation', [0, 0, 0, 1])
    cross = [y * v[2] - z * v[1], z * v[0] - x * v[2], x * v[1] - y * v[0]]
    twice = [2 * c for c in cross]
    second = [y * twice[2] - z * twice[1], z * twice[0] - x * twice[2], x * twice[1] - y * twice[0]]
    return [v[i] + w * twice[i] + second[i] + node.get('translation', [0, 0, 0])[i] for i in range(3)]


def vertical_bounds(document):
    values = []
    nodes = document['nodes']

    def visit(index, ancestors):
        node = nodes[index]
        chain = [node, *ancestors]
        if 'mesh' in node:
            for primitive in document['meshes'][node['mesh']]['primitives']:
                accessor = document['accessors'][primitive['attributes']['POSITION']]
                if 'min' not in accessor or 'max' not in accessor:
                    raise ValueError('Export POSITION bounds in the GLB to fit the model automatically.')
                for corner in range(8):
                    point = [accessor['max' if corner & (1 << axis) else 'min'][axis] for axis in range(3)]
                    for transform in chain:
                        point = transformed(point, transform)
                    values.append(point[1])
        for child in node.get('children', []):
            visit(child, chain)

    for root in document['scenes'][document.get('scene', 0)]['nodes']:
        visit(root, [])
    if not values or max(values) - min(values) < 0.001:
        raise ValueError('The scene contains no usable model bounds.')
    return min(values), max(values)


def read_glb(data):
    if len(data) < 28 or struct.unpack_from('<III', data) != (0x46546C67, 2, len(data)):
        raise ValueError('Expected a glTF 2 binary container (GLB or VRM).')
    chunks = {}
    cursor = 12
    while cursor < len(data):
        size, kind = struct.unpack_from('<II', data, cursor)
        cursor += 8
        if cursor + size > len(data):
            raise ValueError('Truncated GLB chunk.')
        chunks[kind] = data[cursor:cursor + size]
        cursor += size
    document = json.loads(chunks[0x4E4F534A])
    binary = chunks.get(0x004E4942, b'')
    if len(document.get('buffers', [])) != 1 or 'uri' in document['buffers'][0]:
        raise ValueError('Export a self-contained model with embedded buffers and textures.')
    if any('uri' in image for image in document.get('images', [])):
        raise ValueError('Textures must be embedded in the GLB.')
    return document, binary


def compact(document, binary, strip_morphs):
    document = copy.deepcopy(document)
    # VRM shaders, spring bones and expressions are not glTF animations.
    document.get('extensions', {}).pop('VRM', None)
    for key in ['extensionsUsed', 'extensionsRequired']:
        if key in document:
            document[key] = [extension for extension in document[key] if extension != 'VRM']
    for material in document.get('materials', []):
        normal = material.get('normalTexture')
        color = material.get('pbrMetallicRoughness', {}).get('baseColorTexture')
        if 'KHR_materials_unlit' in material.get('extensions', {}) and normal and color and normal['index'] == color['index']:
            # Some VRM exporters reuse the diffuse image as a fake normal map.
            # That makes glTF loaders treat the shared image as linear rather than sRGB.
            material.pop('normalTexture')
    if strip_morphs:
        for mesh in document.get('meshes', []):
            mesh.pop('weights', None)
            mesh.get('extras', {}).pop('targetNames', None)
            for primitive in mesh['primitives']:
                primitive.pop('targets', None)
        for node in document.get('nodes', []):
            node.pop('weights', None)
        for animation in document.get('animations', []):
            animation['channels'] = [channel for channel in animation['channels'] if channel['target']['path'] != 'weights']

    # Gather actual references, then repack the retained buffer views with 4-byte alignment.
    references = []
    for mesh in document.get('meshes', []):
        for primitive in mesh['primitives']:
            references.extend((primitive['attributes'], key) for key in primitive['attributes'])
            if 'indices' in primitive:
                references.append((primitive, 'indices'))
            for target in primitive.get('targets', []):
                references.extend((target, key) for key in target)
    for skin in document.get('skins', []):
        if 'inverseBindMatrices' in skin:
            references.append((skin, 'inverseBindMatrices'))
    for animation in document.get('animations', []):
        for sampler in animation['samplers']:
            references.extend([(sampler, 'input'), (sampler, 'output')])
    used = sorted({owner[key] for owner, key in references})
    accessor_map = {old: new for new, old in enumerate(used)}
    document['accessors'] = [document['accessors'][index] for index in used]
    for owner, key in references:
        owner[key] = accessor_map[owner[key]]
    views = [(accessor, 'bufferView') for accessor in document['accessors'] if 'bufferView' in accessor]
    for accessor in document['accessors']:
        if 'sparse' in accessor:
            views.extend([(accessor['sparse']['indices'], 'bufferView'), (accessor['sparse']['values'], 'bufferView')])
    views.extend((image, 'bufferView') for image in document.get('images', []))
    used_views = sorted({owner[key] for owner, key in views})
    view_map = {old: new for new, old in enumerate(used_views)}
    payload = bytearray()
    retained = []
    for index in used_views:
        view = document['bufferViews'][index]
        start, size = view.get('byteOffset', 0), view['byteLength']
        if start + size > len(binary):
            raise ValueError('Buffer view is outside the binary chunk.')
        payload.extend(b'\0' * (-len(payload) % 4))
        retained.append({**view, 'buffer': 0, 'byteOffset': len(payload)})
        payload.extend(binary[start:start + size])
    for owner, key in views:
        owner[key] = view_map[owner[key]]
    document['bufferViews'] = retained
    document['buffers'] = [{'byteLength': len(payload)}]
    encoded = json.dumps(document, ensure_ascii=False, separators=(',', ':')).encode()
    encoded += b' ' * (-len(encoded) % 4)
    payload.extend(b'\0' * (-len(payload) % 4))
    return (struct.pack('<III', 0x46546C67, 2, 28 + len(encoded) + len(payload))
            + struct.pack('<II', len(encoded), 0x4E4F534A) + encoded
            + struct.pack('<II', len(payload), 0x004E4942) + payload)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', help='Local GLB/VRM file or direct HTTPS URL')
    parser.add_argument('--id', required=True)
    parser.add_argument('--name', required=True)
    parser.add_argument('--author', required=True)
    parser.add_argument('--source', required=True)
    parser.add_argument('--license', required=True)
    parser.add_argument('--scale', type=float, help='Override automatic fit to the 1.8-unit player height')
    parser.add_argument('--offset-y', type=float, help='Override automatic placement of the feet')
    parser.add_argument('--yaw', type=float, default=180, help='Degrees; game forward is -Z')
    parser.add_argument('--strip-morphs', action='store_true', help='Remove facial expressions, keeping the bind-pose face')
    parser.add_argument('--alpha-material', action='append', default=[], help='Material name whose texture is a transparent overlay')
    parser.add_argument('--base-color-texture', type=Path, help='Embed an external PNG in a model with one material')
    parser.add_argument('--unlit', action='store_true', help='Keep baked/cartoon colors independent of world lighting')
    options = parser.parse_args()
    if not re.fullmatch('[a-z0-9_-]{1,64}', options.id):
        parser.error('ID must be a lowercase slug.')
    if options.input.startswith('https://'):
        with urllib.request.urlopen(options.input, timeout=60) as response:
            data = response.read(128 * 1024 * 1024 + 1)
    else:
        data = Path(options.input).read_bytes()
    if len(data) > 128 * 1024 * 1024:
        parser.error('Model exceeds 128 MiB.')
    original, binary = read_glb(data)
    provenance_meta = copy.deepcopy(original.get('extensions', {}).get('VRM', {}).get('meta'))
    for material in original.get('materials', []):
        if material.get('name') in options.alpha_material:
            material['alphaMode'] = 'BLEND'
        if options.unlit:
            material.setdefault('extensions', {})['KHR_materials_unlit'] = {}
            original.setdefault('extensionsUsed', []).append('KHR_materials_unlit')
    original['extensionsUsed'] = list(dict.fromkeys(original.get('extensionsUsed', [])))
    if options.base_color_texture:
        if len(original.get('materials', [])) != 1:
            parser.error('--base-color-texture requires exactly one material.')
        texture = options.base_color_texture.read_bytes()
        if not texture.startswith(b'\x89PNG\r\n\x1a\n'):
            parser.error('Expected a PNG texture.')
        binary += b'\0' * (-len(binary) % 4)
        view = len(original['bufferViews'])
        original['bufferViews'].append({'buffer': 0, 'byteOffset': len(binary), 'byteLength': len(texture)})
        binary += texture
        image = len(original.setdefault('images', []))
        original['images'].append({'bufferView': view, 'mimeType': 'image/png'})
        index = len(original.setdefault('textures', []))
        original['textures'].append({'source': image})
        material = original['materials'][0].setdefault('pbrMetallicRoughness', {})
        material['baseColorTexture'] = {'index': index}
        material['baseColorFactor'] = [1, 1, 1, 1]
    low, high = vertical_bounds(original)
    scale = options.scale if options.scale is not None else 1.8 / (high - low)
    offset = options.offset_y if options.offset_y is not None else -low * scale
    if not math.isfinite(scale) or not 0.001 <= scale <= 100 or not math.isfinite(offset) or abs(offset) > 100 or not math.isfinite(options.yaw):
        parser.error('Invalid model scale, offset or heading.')
    packed = compact(original, binary, options.strip_morphs)
    destination = ROOT / 'assets' / 'characters' / options.id
    destination.mkdir(parents=True, exist_ok=True)
    (destination / 'model.glb').write_bytes(packed)
    provenance = {'source': options.source, 'author': options.author, 'license': options.license,
                  'original_sha256': hashlib.sha256(data).hexdigest(),
                  'original_vrm_metadata': provenance_meta,
                  'stripped_morphs': options.strip_morphs, 'alpha_materials': options.alpha_material,
                  'texture_sha256': hashlib.sha256(options.base_color_texture.read_bytes()).hexdigest() if options.base_color_texture else None}
    (destination / 'source.json').write_text(json.dumps(provenance, ensure_ascii=False, indent=2) + '\n')
    catalog_path = ROOT / 'assets' / 'characters' / 'catalog.json'
    catalog = json.loads(catalog_path.read_text())
    previous = next((entry for entry in catalog['characters'] if entry['id'] == options.id), {})
    clips = [animation.get('name', '') for animation in original.get('animations', [])]
    animations = dict(previous.get('animations', {}))
    for role, alternatives in {'idle': ['idle'], 'walk': ['walk', 'walking'], 'run': ['run', 'running'],
                               'jump': ['jump', 'fallup'], 'dash': ['dash', 'roll'], 'death': ['death', 'die']}.items():
        if role not in animations:
            match = next((clip for clip in clips if clip.lower().replace('-loop', '').replace('_loop', '') in alternatives), None)
            if match:
                animations[role] = match
    entry = {**previous, 'id': options.id, 'name': options.name, 'model': f'characters/{options.id}/model.glb',
             'author': options.author, 'source': options.source, 'license': options.license,
             'scale': scale, 'offset': [0, offset, 0],
             'yaw': options.yaw * 3.141592653589793 / 180, 'animations': animations}
    humanoid = original.get('extensions', {}).get('VRM', {}).get('humanoid', {}).get('humanBones', [])
    bone_names = {bone['bone']: original['nodes'][bone['node']].get('name', '') for bone in humanoid}
    roles = {'left_arm': 'leftUpperArm', 'right_arm': 'rightUpperArm', 'left_leg': 'leftUpperLeg',
             'right_leg': 'rightUpperLeg', 'left_knee': 'leftLowerLeg', 'right_knee': 'rightLowerLeg', 'chest': 'chest'}
    if all(bone_names.get(bone) for bone in roles.values()) and not animations:
        entry['rig'] = {role: bone_names[bone] for role, bone in roles.items()}
    catalog['characters'] = [entry if item['id'] == options.id else item for item in catalog['characters']]
    if not previous:
        catalog['characters'].append(entry)
    catalog_path.write_text(json.dumps(catalog, ensure_ascii=False, indent=2) + '\n')
    print(f'{options.name}: {len(data):,} → {len(packed):,} bytes; {entry["model"]}')


if __name__ == '__main__':
    main()
