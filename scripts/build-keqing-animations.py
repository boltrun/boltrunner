#!/usr/bin/env python3
"""Bake a complete humanoid gait into the retained Keqing GLB."""
import json
import math
from pathlib import Path
import struct

from character_motion import leg_angles, multiply, quat, rotation, smooth, step

ROOT = Path(__file__).resolve().parent.parent
MODEL = ROOT / 'assets/characters/keqing-opulent/model.glb'


def build():
    data = MODEL.read_bytes()
    length = struct.unpack_from('<I', data, 12)[0]
    document = json.loads(data[20:20 + length])
    offset = 20 + length
    size = struct.unpack_from('<I', data, offset)[0]
    payload = bytearray(data[offset + 8:offset + 8 + size])
    # Re-running the exporter replaces its clips instead of growing the model.
    extras = document.setdefault('extras', {})
    previous = extras.get('boltrunner_motion_base')
    if previous:
        del document['accessors'][previous['accessors']:]
        del document['bufferViews'][previous['views']:]
        del payload[previous['bytes']:]
    else:
        previous = {'accessors': len(document['accessors']), 'views': len(document['bufferViews']),
                    'bytes': document['buffers'][0]['byteLength']}
        del payload[previous['bytes']:]
        extras['boltrunner_motion_base'] = previous
    nodes = document['nodes']
    ids = {node.get('name'): i for i, node in enumerate(nodes)}
    hips = nodes[ids['Hips']]['translation']
    lengths = [abs(nodes[ids[name]]['translation'][1]) for name in ['Left knee', 'Left ankle']]
    rest_height = sum(lengths)

    def accessor(values, kind):
        components = {'SCALAR': 1, 'VEC3': 3, 'VEC4': 4}[kind]
        flat = values if components == 1 else [component for value in values for component in value]
        packed = struct.pack('<' + 'f' * len(flat), *flat)
        payload.extend(b'\0' * (-len(payload) % 4))
        view = len(document['bufferViews'])
        document['bufferViews'].append({'buffer': 0, 'byteOffset': len(payload), 'byteLength': len(packed)})
        payload.extend(packed)
        entry = {'bufferView': view, 'componentType': 5126, 'count': len(values), 'type': kind}
        if kind == 'SCALAR':
            entry.update(min=[min(values)], max=[max(values)])
        document['accessors'].append(entry)
        return len(document['accessors']) - 1

    def pose(name, t):
        phase = 2 * math.pi * t
        moving = name in ['Walk', 'Run']
        running = name == 'Run'
        sway = math.sin(phase)
        bob = (-0.042 if running else -0.022) + (0.012 if running else 0.005) * math.cos(2*phase) if moving else 0.0
        translations = {'Hips': [hips[0] + (0.010 * sway if moving else 0), hips[1] + bob, hips[2]]}
        rotations = {'Hips': rotation(y=0.045*sway if moving else 0, z=0.018*sway if moving else 0),
                     'Spine': rotation(x=-0.025 if moving else math.sin(phase)*0.007),
                     'Chest': rotation(x=-0.09 if running else -0.02 if moving else 0,
                                       y=-0.065*sway if moving else 0),
                     'Upper Chest': rotation(), 'Neck': rotation(x=0.025 if running else 0),
                     'Head': rotation(x=0.04 if running else 0.006*math.sin(phase),
                                      y=0.018*sway if moving else 0.012*math.sin(phase))}
        for side, offset, sign in [('Left', 0.0, -1), ('Right', 0.5, 1)]:
            forward, lift, flight = step(t + offset, .24 if running else .16, .13 if running else .055)
            hip, knee = leg_angles(forward, rest_height + bob - lift, *lengths) if moving else (0.0, 0.0)
            ankle = -hip - knee - flight * 0.14
            arm_swing = -math.cos(phase + offset*2*math.pi) * (.65 if running else .30) if moving else .025*math.sin(phase)
            elbow = .95 if running else .24 if moving else .12
            spread = 1.40 if running else 1.46
            rotations[f'{side} shoulder'] = rotation(y=-sign*0.015*sway if moving else 0)
            # The source is a T-pose. Lowering the arm must happen before world-X swing.
            rotations[f'{side} arm'] = multiply(quat(0, arm_swing), quat(2, -sign*spread))
            rotations[f'{side} elbow'] = quat(1, sign*elbow)
            rotations[f'{side} wrist'] = quat(1, -sign*.05)
            rotations[f'{side} leg'] = quat(0, hip)
            rotations[f'{side} knee'] = quat(0, knee)
            rotations[f'{side} ankle'] = quat(0, ankle)
            rotations[f'{side} toe'] = quat(0, 0)
            suffix = 'L' if side == 'Left' else 'R'
            for finger in ['IndexFinger', 'MiddleFinger', 'RingFinger', 'LittleFinger']:
                for section in range(1, 4):
                    rotations[f'{finger}{section}_{suffix}'] = quat(2, -sign*((.42 if running else .25) if section == 1 else .24 if running else .16))
            if name in ['Jump', 'Dash']:
                rotations[f'{side} leg'] = quat(0, .35 if name == 'Jump' else .52 if side == 'Left' else -.25)
                rotations[f'{side} knee'] = quat(0, -.75 if name == 'Jump' else -.9)
                rotations[f'{side} ankle'] = quat(0, .25)
                rotations[f'{side} arm'] = multiply(quat(0, -.45 if name == 'Jump' else -.7), quat(2, -sign*1.35))
                rotations['Chest'] = quat(0, -.15 if name == 'Jump' else -.30)
                rotations['Head'] = quat(0, .12)
            if name == 'Death':
                fall = smooth(t)
                rotations['Hips'] = rotation(x=-1.35*fall, z=.20*fall)
                translations['Hips'] = [hips[0], hips[1] - .60*fall, hips[2] - .25*fall]
                rotations[f'{side} knee'] = quat(0, -.45*fall)
                rotations[f'{side} arm'] = multiply(quat(0, .12*fall), quat(2, -sign*(1.46 - .2*fall)))
        # Small delayed follow-through for the long hair and costume attachments.
        for node in nodes:
            bone = node.get('name', '')
            if bone.startswith(('AHair_0_', 'Sweet_0_')):
                rotations[bone] = rotation(x=(.035 if moving else .012)*math.sin(phase-.65),
                                           z=.014*math.sin(phase-.4))
        return rotations, translations

    document['animations'] = []
    for name, duration in [('Idle', 3.4), ('Walk', .88), ('Run', .58), ('Jump', 1.0), ('Dash', .6), ('Death', 1.25)]:
        frames = 48
        samples = [pose(name, frame/frames) for frame in range(frames + 1)]
        animation = {'name': name, 'samplers': [], 'channels': []}
        time = accessor([duration*frame/frames for frame in range(frames + 1)], 'SCALAR')
        for path, group, kind in [('rotation', 0, 'VEC4'), ('translation', 1, 'VEC3')]:
            for bone in samples[0][group]:
                if bone not in ids:
                    continue
                values = [sample[group][bone] for sample in samples]
                animation['channels'].append({'sampler': len(animation['samplers']), 'target': {'node': ids[bone], 'path': path}})
                animation['samplers'].append({'input': time, 'output': accessor(values, kind), 'interpolation': 'LINEAR'})
        document['animations'].append(animation)
    document['buffers'][0]['byteLength'] = len(payload)
    encoded = json.dumps(document, separators=(',', ':')).encode()
    encoded += b' ' * (-len(encoded) % 4)
    payload.extend(b'\0' * (-len(payload) % 4))
    MODEL.write_bytes(struct.pack('<III', 0x46546c67, 2, 28 + len(encoded) + len(payload))
                      + struct.pack('<II', len(encoded), 0x4e4f534a) + encoded
                      + struct.pack('<II', len(payload), 0x004e4942) + payload)
    print(f'Keqing: six authored clips, {len(encoded) + len(payload) + 28:,} bytes')


if __name__ == '__main__':
    build()
