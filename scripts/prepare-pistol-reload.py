"""Separate the authored pistol battery base and add its concealed cartridge body."""
import json
import struct
from pathlib import Path


def prepare(path):
    data = path.read_bytes()
    size = struct.unpack_from('<I', data, 12)[0]
    model = json.loads(data[20:20 + size])
    if any(n.get('name') == 'PistolEnergyCell' for n in model['nodes']):
        return
    binary = bytearray(data[28 + size:])

    def read(index):
        accessor = model['accessors'][index]
        view = model['bufferViews'][accessor['bufferView']]
        count = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4}[accessor['type']]
        fmt = {5126: 'f', 5125: 'I', 5123: 'H', 5121: 'B'}[accessor['componentType']]
        stride = view.get('byteStride', struct.calcsize(fmt) * count)
        offset = view.get('byteOffset', 0) + accessor.get('byteOffset', 0)
        return [struct.unpack_from('<' + fmt * count, binary, offset + i * stride)
                for i in range(accessor['count'])]

    def append(values, kind, count, component=5126):
        while len(binary) % 4:
            binary.append(0)
        offset = len(binary)
        flat = [v for row in values for v in row]
        binary.extend(struct.pack('<' + ('f' if component == 5126 else 'I') * len(flat), *flat))
        view = len(model['bufferViews'])
        model['bufferViews'].append({'buffer': 0, 'byteOffset': offset, 'byteLength': len(binary)-offset})
        accessor = {'bufferView': view, 'componentType': component, 'count': len(values), 'type': kind}
        if kind == 'VEC3':
            accessor.update(min=[min(row[i] for row in values) for i in range(3)],
                            max=[max(row[i] for row in values) for i in range(3)])
        model['accessors'].append(accessor)
        return len(model['accessors']) - 1

    moving = []
    for primitive in model['meshes'][0]['primitives']:
        xyz = read(primitive['attributes']['POSITION'])
        indices = [v[0] for v in read(primitive['indices'])]
        parents = list(range(len(xyz)))

        def root(i):
            while parents[i] != i:
                parents[i] = parents[parents[i]]
                i = parents[i]
            return i

        def join(a, b):
            parents[root(a)] = root(b)

        points = {}
        for i, position in enumerate(xyz):
            key = tuple(round(v, 5) for v in position)
            if key in points:
                join(i, points[key])
            points[key] = i
        for a, b, c in zip(indices[::3], indices[1::3], indices[2::3]):
            join(a, b)
            join(a, c)
        components = {}
        for i in set(indices):
            components.setdefault(root(i), []).append(xyz[i])
        # The existing lower battery plate and its luminous charge strip.
        battery = {key for key, vertices in components.items()
                   if max(v[1] for v in vertices) < -.15 and min(v[2] for v in vertices) > .10}
        selected = [i for i in indices if root(i) in battery]
        if selected:
            moving.append({**primitive, 'indices': append([(i,) for i in selected], 'SCALAR', 1, 5125)})
            fixed = [i for i in indices if root(i) not in battery]
            primitive['indices'] = append([(i,) for i in fixed], 'SCALAR', 1, 5125)
    assert moving, 'Authored pistol battery plate was not found'
    # Concealed battery body extends from the actual plate into the pistol grip.
    vertices, normals, uv, indices = [], [], [], []
    corners = [(x, y, z) for z in [.123, .171] for y in [-.162, -.057] for x in [-.018, .018]]
    for face, normal in [([0, 1, 3, 2], [0, 0, -1]), ([4, 6, 7, 5], [0, 0, 1]),
                         ([0, 4, 5, 1], [0, -1, 0]), ([2, 3, 7, 6], [0, 1, 0]),
                         ([0, 2, 6, 4], [-1, 0, 0]), ([1, 5, 7, 3], [1, 0, 0])]:
        base = len(vertices)
        vertices.extend(corners[i] for i in face)
        normals.extend([normal] * 4)
        uv.extend([(0, 0), (0, 1), (1, 1), (1, 0)])
        indices.extend([base, base+2, base+1, base, base+3, base+2])
    moving.append({'attributes': {'POSITION': append(vertices, 'VEC3', 3), 'NORMAL': append(normals, 'VEC3', 3),
                                  'TEXCOORD_0': append(uv, 'VEC2', 2)},
                   'indices': append([(i,) for i in indices], 'SCALAR', 1, 5125), 'material': 0})
    mesh = len(model['meshes'])
    model['meshes'].append({'name': 'PistolEnergyCell', 'primitives': moving})
    node = len(model['nodes'])
    model['nodes'].append({'name': 'PistolEnergyCell', 'mesh': mesh})
    next(n for n in model['nodes'] if n.get('name') == 'StarterPistol')['children'].append(node)
    model['buffers'][0]['byteLength'] = len(binary)
    document = json.dumps(model, separators=(',', ':')).encode()
    document += b' ' * (-len(document) % 4)
    binary += b'\0' * (-len(binary) % 4)
    path.write_bytes(struct.pack('<III', 0x46546c67, 2, 28+len(document)+len(binary))
                     + struct.pack('<II', len(document), 0x4e4f534a) + document
                     + struct.pack('<II', len(binary), 0x004e4942) + binary)


if __name__ == '__main__':
    prepare(Path(__file__).resolve().parents[1] / 'assets/weapons/starter_pistol/built/starter_pistol.glb')
