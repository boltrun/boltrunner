#!/usr/bin/env python3
"""Compile coordinate-authored voxel geometry into the current map format."""
import gzip
import hashlib
import json
import math
import struct
from collections import defaultdict
from pathlib import Path

from PIL import Image
from voxel_bake import VoxelBake

ROOT=Path(__file__).resolve().parents[1]
SOURCE=ROOT/'assets/bottle_shipyard/source'
BUILT=ROOT/'assets/bottle_shipyard/built'
fingerprint=hashlib.sha256()
for path in sorted(SOURCE.rglob('*'))+[Path(__file__),Path(__file__).with_name('voxel_bake.py')]:
    if path.is_file():fingerprint.update(path.name.encode());fingerprint.update(path.read_bytes())
fingerprint=fingerprint.hexdigest()
cache_path=BUILT/'bake.json'
if cache_path.exists():
    cached=json.loads(cache_path.read_text())
    if cached['source']==fingerprint and all((BUILT/name).exists() and hashlib.sha256((BUILT/name).read_bytes()).hexdigest()==digest for name,digest in cached['outputs'].items()):
        print('Voxel geometry and baked lighting are current.')
        raise SystemExit(0)
scene=json.loads(gzip.decompress((SOURCE/'blocks.json.gz').read_bytes()))
blocks={(x,y,z):m for x,y,z,m in scene['blocks']}
palette={p['id']:p for p in scene['palette']}
directions=[
 ('east',(1,0,0),(.5,-.5,-.5),(0,1,0),(0,0,1),.88),
 ('west',(-1,0,0),(-.5,-.5,.5),(0,1,0),(0,0,-1),.69),
 ('up',(0,1,0),(-.5,.5,.5),(1,0,0),(0,0,-1),1.),
 ('down',(0,-1,0),(-.5,-.5,-.5),(1,0,0),(0,0,1),.57),
 ('south',(0,0,1),(-.5,-.5,.5),(1,0,0),(0,1,0),.85),
 ('north',(0,0,-1),(.5,-.5,-.5),(-1,0,0),(0,1,0),.74),
]
def small(m):return 'lantern[' in m or 'chain[' in m
def solid(m):return not small(m) and 'water[' not in m
def glass(m):return 'glass' in m
def texture(m,n):
    name=m.split(':')[1].split('[')[0]
    if 'log' in name:
        axis=0 if 'axis=x' in m else 2 if 'axis=z' in m else 1
        return name+('_top' if n[axis] else '')
    if name=='barrel':return 'barrel_top' if n[1]>0 else 'barrel_bottom' if n[1]<0 else 'barrel_side'
    if name=='sandstone':return 'sandstone_top' if n[1]>0 else 'sandstone_bottom' if n[1]<0 else 'sandstone'
    if name=='bookshelf':return 'oak_planks' if n[1] else 'bookshelf'
    if name=='crafting_table':return 'crafting_table_top' if n[1]>0 else 'oak_planks' if n[1]<0 else 'crafting_table_front' if n[0] else 'crafting_table_side'
    return 'water_still' if name=='water' else palette[m]['texture'].removesuffix('.png')
def rotate(v,axis,angle):
    x,y,z=v;c=math.cos(angle);s=math.sin(angle)
    return (x*c+z*s,y,-x*s+z*c) if axis=='y' else (x,y*c-z*s,y*s+z*c) if axis=='x' else (x*c-y*s,x*s+y*c,z)
surfaces=defaultdict(list);collision_vertices=[];collision_indices=[];vertex_index={}
def add_collision(points):
    ids=[]
    for p in points:
        if p not in vertex_index:vertex_index[p]=len(collision_vertices);collision_vertices.append(p)
        ids.append(vertex_index[p])
    collision_indices.extend([(ids[0],ids[1],ids[2]),(ids[0],ids[2],ids[3])])
for p,m in sorted(blocks.items()):
    shapes=[None]
    if small(m):
        model='chain' if 'chain' in m else 'lantern_hanging' if 'hanging=true' in m else 'lantern'
        shapes=scene['block_models'][model]['elements']
    for shape in shapes:
        for name,n,a,u,v,shade in directions:
            neighbor=blocks.get(tuple(p[i]+n[i] for i in range(3)))
            if shape and name not in shape['faces']:continue
            if shape is None:
                if solid(m) and (not neighbor or not solid(neighbor)):
                    points=[tuple(p[i]+a[i]+su*u[i]+sv*v[i] for i in range(3)) for su,sv in ((0,0),(1,0),(1,1),(0,1))]
                    add_collision(points)
                if neighbor and not small(neighbor) and (not glass(neighbor) or glass(m)):continue
            origin=[shape['from'][i]/16-.5 for i in range(3)] if shape else [-.5]*3
            size=[(shape['to'][i]-shape['from'][i])/16 for i in range(3)] if shape else [1.]*3
            points=[tuple(p[i]+origin[i]+(a[i]+.5+su*u[i]+sv*v[i])*size[i] for i in range(3)) for su,sv in ((0,0),(1,0),(1,1),(0,1))]
            normal=n
            if shape and 'rotation' in shape:
                r=shape['rotation'];angle=math.radians(r['angle']);center=[p[i]+r['origin'][i]/16-.5 for i in range(3)]
                points=[tuple(c[i]+center[i] for i in range(3)) for c in [rotate(tuple(q[i]-center[i] for i in range(3)),r['axis'],angle) for q in points]]
                normal=rotate(n,r['axis'],angle)
            image='chain' if shape and 'chain' in m else 'lantern' if shape else texture(m,n)
            uv=shape['faces'][name].get('uv',[0,0,16,16]) if shape else [0,0,16,16]
            u0,v0,u1,v1=[x/16 for x in uv]
            coords=[(u0,v1),(u1,v1),(u1,v0),(u0,v0)]
            # Side faces whose first axis is vertical need the texture axes swapped.
            if n[0] and not shape:coords=[(u1,v1),(u1,v0),(u0,v0),(u0,v1)]
            tint=(.28,.54,.80) if 'water[' in m else (1.,1.,1.)
            color=tuple(c*shade for c in tint)+(1.,)
            surfaces[(image,glass(m),bool(shape),'water[' in m)].append((points,normal,coords,color))

BUILT.mkdir(parents=True,exist_ok=True);(BUILT/'textures').mkdir(exist_ok=True)
bake=VoxelBake(blocks,BUILT/'textures')
materials=[];render=bytearray(struct.pack('<I',len(surfaces)))
for index,((name,blend,masked,water),quads) in enumerate(sorted(surfaces.items())):
    im=Image.open(SOURCE/'textures'/f'{name}.png').convert('RGBA').crop((0,0,16,16))
    if blend:
        im.putdata([(r,g,b,65 if a else 12) if a else (170,215,225,12) for r,g,b,a in im.getdata()])
    im.save(BUILT/'textures'/f'{name}.png')
    materials.append({'name':f'voxel:{name}','texture':f'{name}.png','glow':None,'emissive':blend or water or masked,'alpha':masked,'blend':blend or water})
    print(f'Baking {name}: {len(quads)} faces',flush=True)
    lighting=[(-1,[(0.,0.)]*4)]*len(quads) if blend or water or masked else bake.batch(quads)
    # Split surfaces only at atlas boundaries; diffuse grouping stays intact.
    groups=defaultdict(list)
    for quad,(atlas,uv) in zip(quads,lighting):groups[atlas].append((quad,uv))
    for atlas,group in groups.items():
        render.extend(struct.pack('<IiII',index,atlas,len(group)*4,len(group)*6))
        for (points,normal,uvs,color),light_uvs in group:
            for position,uv,light_uv in zip(points,uvs,light_uvs):render.extend(struct.pack('<14f',*position,*normal,*uv,*light_uv,*color))
        for i in range(len(group)):
            j=i*4;render.extend(struct.pack('<6I',j,j+1,j+2,j,j+2,j+3))
# There are currently fewer faces than one atlas, but keep the binary count exact.
reader=4;surface_count=0
while reader<len(render):
    _,_,vertices,indices=struct.unpack_from('<IiII',render,reader);reader+=16+vertices*56+indices*4;surface_count+=1
struct.pack_into('<I',render,0,surface_count)
lightmaps=bake.save()
(BUILT/'textures/crystal_explosion.png').write_bytes((SOURCE/'textures/crystal_explosion.png').read_bytes())
(BUILT/'render.bin').write_bytes(render)
collision=bytearray(struct.pack('<II',len(collision_vertices),len(collision_indices)))
for p in collision_vertices:collision.extend(struct.pack('<3f',*p))
for triangle in collision_indices:collision.extend(struct.pack('<3I',*triangle))
(BUILT/'collision.bin').write_bytes(collision)
def planes(lo,hi):return [[1,0,0,hi[0]],[-1,0,0,-lo[0]],[0,1,0,hi[1]],[0,-1,0,-lo[1]],[0,0,1,hi[2]],[0,0,-1,-lo[2]]]
triggers=[]
for pad in scene['jump_pads']:
    x,y,z=pad['from'];triggers.append({'kind':'launch','speed':pad['velocity'][1],'planes':planes([x-1.1,y-.1,z-1.1],[x+1.1,y+.06,z+1.1])})
# A small original pixel sky, sampled from directions for seamless cube edges.
sky=Image.new('RGB',(128,128*6));pixels=sky.load()
for face in range(6):
    for y in range(128):
        for x in range(128):
            u=(x+.5)/64-1;v=(y+.5)/64-1
            d=[(1,-v,-u),(-1,-v,u),(u,1,v),(u,-1,-v),(u,-v,1),(-u,-v,-1)][face]
            length=math.sqrt(sum(c*c for c in d));dx,dy,dz=[c/length for c in d]
            t=max(0.,dy);color=[int(a+(b-a)*t) for a,b in zip((204,226,225),(99,166,224))]
            if dy<-.12:color=[133,171,145]
            if dy>.15:
                cx=math.floor(dx/dy*4);cz=math.floor(dz/dy*4)
                if ((cx*13+cz*29)^(cx//3*41))%17<3:color=[242,246,236]
            pixels[x,y+face*128]=tuple(color)
sky.save(BUILT/'textures/sky.png')
metadata={'id':'bottle_shipyard','name':'Bottle Shipyard','spawns':[{'position':p,'yaw':math.atan2(p[0],p[2])} for p in scene['spawns']],'pickup_spawns':[p['position'] for p in scene['pickup_anchors']],'triggers':triggers,'lightmaps':lightmaps,'sky':'sky.png','materials':materials,'pixelated':True,'crystals':scene['crystals']}
(BUILT/'map.json').write_text(json.dumps(metadata,indent=2)+'\n')
print(json.dumps({'blocks':len(blocks),'quads':sum(len(v) for v in surfaces.values()),'collision_triangles':len(collision_indices),'materials':len(materials),'render_bytes':len(render),'collision_bytes':len(collision),'source_sha256':scene['coordinate_sha256']},indent=2))

outputs={str(p.relative_to(BUILT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(BUILT.rglob('*')) if p.is_file() and p!=cache_path}
cache_path.write_text(json.dumps({'source':fingerprint,'outputs':outputs},indent=2)+'\n')
