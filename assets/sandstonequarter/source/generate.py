#!/usr/bin/env python3
"""Editable metre-scale Sandstone Quarter brush authoring. Bake with import-map.py."""
from pathlib import Path
import math
import numpy as np
from PIL import Image, ImageDraw
import json
import random

SOURCE = Path(__file__).resolve().parent
MAP = SOURCE.parent / 'sandstonequarter.map'
BRUSHES, ENTITIES = [], []
CUBE = [(0,1,2,3),(4,7,6,5),(0,4,5,1),(1,5,6,2),(2,6,7,3),(3,7,4,0)]

def brush(vertices, faces, material, detail=True):
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
        lines.append(f'{points} {material} {uv} 0 {0.125 if material == 'sandstone/plaster' else 0.3} {0.125 if material == 'sandstone/plaster' else 0.3} {134217728 if detail else 0} 0 0')
    BRUSHES.append('\n'.join([*lines,'}']))

def box(x,y,z,w,d,h,mat,detail=True):
    brush([(x-w/2,y-d/2,z),(x+w/2,y-d/2,z),(x+w/2,y+d/2,z),(x-w/2,y+d/2,z),
           (x-w/2,y-d/2,z+h),(x+w/2,y-d/2,z+h),(x+w/2,y+d/2,z+h),(x-w/2,y+d/2,z+h)], CUBE,mat,detail)

def prism(x,y,z,r,h,mat,sides=12,top=None):
    top = r if top is None else top
    vertices = [(x+rr*math.cos(i*math.tau/sides),y+rr*math.sin(i*math.tau/sides),zz)
                for zz,rr in ((z,r),(z+h,top)) for i in range(sides)]
    faces = [tuple(range(sides)),tuple(range(sides,2*sides))]
    faces += [(i,(i+1)%sides,(i+1)%sides+sides,i+sides) for i in range(sides)]
    brush(vertices,faces,mat)

def extrusion(points,center,depth,mat,axis='xz'):
    cx,cy,cz = center
    verts = [(cx+a,cy+t,cz+b) if axis=='xz' else (cx+t,cy+a,cz+b)
             for t in (-depth/2,depth/2) for a,b in points]
    n = len(points)
    brush(verts,[tuple(range(n)),tuple(range(n,2*n))]+[(i,(i+1)%n,(i+1)%n+n,i+n) for i in range(n)],mat)

def oval(center,rx,rz,depth,mat,axis='xz',sides=20):
    extrusion([(rx*math.cos(i*math.tau/sides),rz*math.sin(i*math.tau/sides)) for i in range(sides)],center,depth,mat,axis)

def beam(a,b,width,mat):
    a,b=np.array(a),np.array(b)
    n=(b-a)/np.linalg.norm(b-a)
    u=np.cross(n,[0,1,0] if abs(n[1])<.9 else [0,0,1])
    u*=width/2/np.linalg.norm(u); v=np.cross(n,u)
    brush([p+su*u+sv*v for p in (a,b) for su,sv in [(-1,-1),(1,-1),(1,1),(-1,1)]],CUBE,mat)

def bridge(a,b,width,top,mat='sandstone/stone'):
    a,b=np.array(a,dtype=float),np.array(b,dtype=float)
    v=b-a; side=np.array([-v[1],v[0]])
    side*=width/2/np.linalg.norm(side)
    points=[a-side,a+side,b+side,b-side]
    brush([(x,y,z) for z in (top-.25,top) for x,y in points],CUBE,mat)

def stair(a,b,width,mat='sandstone/stone'):
    a,b=np.array(a,dtype=float),np.array(b,dtype=float)
    delta=b-a; run=np.linalg.norm(delta[:2]); direction=delta[:2]/run
    side=np.array([-direction[1],direction[0]])
    count=math.ceil(abs(delta[2])/.18); middle=count//2
    landing=1.0 if count>14 else 0
    tread=(run-landing)/count
    assert width>=4 and tread>=.24,(a,b,width,tread)
    for i in range(count):
        off=i*tread+(landing if i>=middle else 0)
        lo=a[:2]+direction*off;hi=lo+direction*tread
        top=a[2]+delta[2]*(i+1)/count
        points=[lo-side*width/2,lo+side*width/2,hi+side*width/2,hi-side*width/2]
        brush([(x,y,z) for z in (a[2]-.12,top) for x,y in points],CUBE,mat)
        if i==middle and landing:
            p0=lo-direction*landing
            points=[p0-side*width/2,p0+side*width/2,lo+side*width/2,lo-side*width/2]
            brush([(x,y,z) for z in (a[2]-.12,a[2]+delta[2]*i/count) for x,y in points],CUBE,mat)
    # Three continuous low side walls follow the two flights and flat landing.
    # Their inner faces stay outside the full four-metre walking width.
    join=middle*tread; height=a[2]+delta[2]*middle/count
    spans=[(0,join,a[2],height),(join,join+landing,height,height),(join+landing,run,height,b[2])]
    for sign in (-1,1):
        for lo,hi,za,zb in spans:
            if hi-lo<.001:continue
            q0=a[:2]+direction*lo+side*sign*(width/2+.10)
            q1=a[:2]+direction*hi+side*sign*(width/2+.10)
            edges=[q0-side*.08,q0+side*.08,q1+side*.08,q1-side*.08]
            verts=[(x,y,a[2]-.10) for x,y in edges]
            verts += [(x,y,z+.48) for (x,y),z in zip(edges,[za,za,zb,zb])]
            brush(verts,CUBE,'sandstone/brick')

def entity(classname,pos=None,**props):
    values=dict(classname=classname,**props)
    if pos is not None: values['origin']=' '.join(f'{x*40:.4f}' for x in pos)
    ENTITIES.append('{\n'+'\n'.join(f'"{k}" "{v}"' for k,v in values.items())+'\n}')

def trigger(classname,low,high,**props):
    index=len(BRUSHES)
    low,high=np.array(low),np.array(high)
    c=(low+high)/2; size=high-low
    box(*c[:2],low[2],*size,'common/trigger')
    geometry=BRUSHES.pop()
    values=dict(classname=classname,**props)
    ENTITIES.append('{\n'+'\n'.join(f'"{k}" "{v}"' for k,v in values.items())+'\n'+geometry+'\n}')
    assert len(BRUSHES)==index

def point(x,y,z=0):
    return (x-36,60-y,z)

def r(x,y,w,d,z,h,mat='plaster',detail=True):
    box(*point(x+w/2,y+d/2,z),w,d,h,('common/clip' if mat=='clip' else 'sandstone/'+mat),detail)

def steps(a,b,width):
    stair(point(*a),point(*b),width)

def arch(x,y,width=3.6,height=4.2,depth=.65,axis='xz',z=0):
    # Pointed masonry arches leave a full-height, capsule-wide opening.
    cx,cy,_=point(x,y)
    spring=height-width*.3
    for sign in (-1,1):
        center=(cx+sign*(width/2+.32),cy,z) if axis=='xz' else (cx,cy+sign*(width/2+.32),z)
        box(*center,.64 if axis=='xz' else depth,depth if axis=='xz' else .64,spring,'sandstone/stone')
        for i in range(5):
            t0=i/5;t1=(i+1)/5
            a=sign*width/2*(1-t0);b=sign*width/2*(1-t1)
            # Two circular ogival halves: vertical spring tangents and a pointed
            # crown. A linear profile reads as a triangular roof, not an arch.
            def arc(t):return math.sqrt(1-(1-t*.5)**2)/math.sqrt(.75)
            lo=spring+(height-spring)*arc(t0);hi=spring+(height-spring)*arc(t1)
            extrusion([(a,lo),(b,hi),(b,hi+.38),(a,lo+.38)],(cx,cy,z),depth,'sandstone/brick',axis)
    for sign in (-1,1):
        center=(cx+sign*(width/2+.36),cy-.02,z+.18) if axis=='xz' else (cx-.02,cy+sign*(width/2+.36),z+.18)
        box(*center,.18 if axis=='xz' else depth+.08,depth+.08 if axis=='xz' else .18,spring-.4,'sandstone/teal')

def facade_block(x,y,w,d,height=6.,mat='plaster'):
    # Closed background houses have readable shutters, never fake walk-in doors.
    r(x,y,w,d,0,height,mat)
    for yy in (y-.07,y+d-.10):
        r(x-.08,yy,w+.16,.18,height-.32,.18,'brick')
        r(x-.12,yy-.03,w+.24,.24,height-.14,.14,'stone')
    for xx in (x-.07,x+w-.10):
        r(xx,y,.18,d,height-.32,.18,'brick')
        r(xx-.03,y-.12,.24,d+.24,height-.14,.14,'stone')
    for xx in np.arange(x+1.2,x+w-.6,2.4):
        for yy in (y-.06,y+d+.03):
            for zz in (1.0,3.65):
                if zz+1.35>height-.4:continue
                r(float(xx)-.45,yy,.9,.06,zz,1.35,'window')
                for off in (-.48,.43):r(float(xx)+off,yy-.03,.10,.12,zz-.08,1.51,'wood_decor')
                r(float(xx)-.48,yy-.03,1.01,.12,zz-.10,.12,'stone')
                r(float(xx)-.48,yy-.03,1.01,.12,zz+1.35,.12,'stone')
                r(float(xx)-.025,yy-.04,.05,.14,zz,1.35,'wood_decor')
    for yy in np.arange(y+1.2,y+d-.6,2.4):
        for xx in (x-.06,x+w+.03):
            for zz in (1.0,3.65):
                if zz+1.35>height-.4:continue
                r(xx,float(yy)-.45,.06,.9,zz,1.35,'window')
                r(xx-.03,float(yy)-.49,.12,.12,zz-.08,1.51,'wood_decor')
                r(xx-.03,float(yy)+.39,.12,.12,zz-.08,1.51,'wood_decor')
                r(xx-.03,float(yy)-.49,.12,1.0,zz-.10,.12,'stone')
                r(xx-.03,float(yy)-.49,.12,1.0,zz+1.35,.12,'stone')
    # Small, clearly decorative rooftop rooms break the skyline.
    if min(w,d)>=4:
        r(x+w*.35,y+d*.35,w*.38,d*.38,height,min(1.4,height*.2),mat)
        r(x+w*.35-.08,y+d*.35-.08,w*.38+.16,d*.38+.16,height+min(1.4,height*.2),.12,'stone')


def parapet(axis,fixed,start,end,openings=(),z=3.):
    spans=[(start,end)]
    for lo,hi in openings:
        spans=[part for a,b in spans for part in [(a,min(b,lo)),(max(a,hi),b)] if part[1]-part[0]>.05]
    for a,b in spans:
        if axis=='x':
            r(fixed-.10,a,.20,b-a,z,.70,'brick')
            r(fixed-.13,a,.26,b-a,z+.70,.10,'stone')
        else:
            r(a,fixed-.10,b-a,.20,z,.70,'brick')
            r(a,fixed-.13,b-a,.26,z+.70,.10,'stone')


def terrace(x,west):
    # A real roof over an open arcade. Two large ground exits remain underneath;
    # every roof has two stairs plus the bridge, rather than a dead-end balcony.
    r(x,16,10,14,2.70,.30,'stone')
    back=x if west else x+9.55
    r(back,16,.45,14,0,2.70,'blue' if west else 'plaster')
    r(x,16,10,.45,0,2.70,'blue' if west else 'plaster')
    for xx,yy in [(x+.35,16.35),(x+9.05,16.35),(x+.35,29.05),(x+9.05,29.05)]:
        r(xx,yy,.60,.60,0,2.70,'stone')
        r(xx-.06,yy-.06,.72,.72,2.43,.25,'brick')
    # Under-roof room is open across an entire frontage and a six-metre side exit.
    front=x+9.75 if west else x-.05
    for yy in (22.7,):
        r(front,yy,.30,.60,0,2.70,'stone')
        r(front-.05,yy-.08,.40,.76,2.43,.25,'brick')
    for yy in (19.,25.):
        cx=x+1.1 if west else x+7.1
        r(cx,yy,1.8,2.,0,1.05,'wood')
        r(cx-.04,yy-.04,1.88,2.08,1.05,.10,'stone')
    # Distinct sheltered floor and facade trim, all flush with the ground.
    r(x+.50,16.50,9.,13.,0,.035,'tile' if west else 'paving')
    parapet('y',16,x,x+10)
    parapet('x',x if west else x+10,16,30,[(19.75,24.25)])
    parapet('x',x+10 if west else x,16,30,[(15.7,20.30)])
    parapet('y',30,x,x+10,[(23.75,28.25)] if west else [(43.75,48.25)])
    # Short roof cover has broad approaches on both sides, away from all joins.
    r(x+4.2,24.8,1.6,2.6,3.,2.05,'blue' if west else 'brick')
    r(x+4.15,24.75,1.7,2.7,5.05,.1,'stone')
    # A broad sheltered spawn nook opens into the arcade rather than the street.
    r(x+5.85 if west else x+3.85,16.45,.30,3.85,0,2.15,'blue' if west else 'plaster')


STAIRS=[
    dict(id='west-street-to-haveli',bottom=[12,22,0],top=[18,22,3],width=4),
    dict(id='south-court-to-haveli',bottom=[26,36,0],top=[26,30,3],width=4),
    dict(id='east-street-to-bazaar',bottom=[60,22,0],top=[54,22,3],width=4),
    dict(id='south-court-to-bazaar',bottom=[46,36,0],top=[46,30,3],width=4),
]
SPAWNS=[(10.5,8.5,0,180),(61.5,8.5,0,180),
        (9.5,43,0,0),(62.5,43,0,0),(22.5,18.5,0,180),(49.5,18.5,0,180),
        (18,109,0,180),(54,109,0,180),(29,99,0,90),(43,84,0,270)]
PICKUPS=[(10,32,0),(22,20,0),(23,18,3),(36,18,3),(49,18,3),
         (50,27,0),(62,31,0),(30,43,0),(42,43,0),(36,52,0),(19,77,0),(53,84,0),(32,95,0),(40,110,0)]
ROUTES={
    'west-main-lane':[[9.5,8,0],[9.5,22,0],[13.5,27,0],[13.5,33,0],[9.5,39,0],[10,47,0],[26,46,0]],
    'east-main-lane':[[62.5,8,0],[62.5,22,0],[58.5,27,0],[58.5,33,0],[62.5,39,0],[62,47,0],[46,46,0]],
    'central-west-loop':[[36,8,0],[36,12,0],[32,15,0],[32,29,0],[31,39,0],[30,46,0],[36,46,0]],
    'central-east-loop':[[36,8,0],[36,12,0],[40,15,0],[40,29,0],[41,39,0],[42,46,0],[36,46,0]],
    'upper-crossing':[[10.5,22,0],[12,22,0],[18.7,22,3],[23,22,3],[24,18,3],[36,18,3],[48,18,3],[49,22,3],[53.3,22,3],[60,22,0],[61.5,22,0]],
    'haveli-roof-to-court':[[23,18,3],[26,23,3],[26,28.5,3],[26,30,3],[26,36,0],[29,39,0],[31,43,0]],
    'bazaar-roof-to-court':[[49,18,3],[46,23,3],[46,28.5,3],[46,30,3],[46,36,0],[43,39,0],[41,43,0]],
    'haveli-underpass':[[30,21,0],[26,21,0],[23,26,0],[21,28,0],[21,32,0],[21,38,0]],
    'bazaar-underpass':[[42,21,0],[46,21,0],[49,26,0],[51,28,0],[51,32,0],[51,38,0]],
}

# A sequence of courts changes the available flanks instead of extending a grid.
ROUTES.update({
    'middle-west-transition':[[10,47,0],[24,46,0],[25,48,0],[25,53,0],[29,60,0],[29,71,0],[21,78,0],[19,88,0],[19,103,0],[18,108,0],[22,110,0],[36,112,0]],
    'middle-east-transition':[[62,47,0],[48,46,0],[47,48,0],[47,53,0],[43,60,0],[43,73,0],[53,79,0],[53,98,0],[53,109,0],[50,111,0],[36,112,0]],
    'caravan-court-loop':[[34,78,0],[34,84,0],[31,88,0],[31,96,0],[38.2,99,0],[38.2,105,0],[41,109,0]],
    'caravan-west-connector':[[19,93,0],[27,93,0],[31,93,0],[31,88,0],[34,84,0]],
    'caravan-east-connector':[[53,89,0],[45,89,0],[42,89,0],[42,96,0],[38.2,99,0]],
})


def caravanserai():
    # Four real, offset gates join a broad open courtyard. No roof stairs here.
    for args in [(25,82,4.8,.7),(38.2,82,8.8,.7),(25,101.3,9.,.7),(42.4,101.3,4.6,.7),
                 (25,82,.7,8.),(25,96,.7,6.),(46.3,82,.7,4.),(46.3,92,.7,10.)]:
        facade_block(*args,height=5.8)
    arch(34,82.35,8.4,4.9,.8)
    arch(38.2,101.65,8.4,4.9,.8)
    arch(25.35,93,6.,4.5,.8,'yz')
    arch(46.65,89,6.,4.5,.8,'yz')
    r(25.7,82.7,20.6,18.6,0,.025,'tile')
    # A carpet stall breaks the one direct shot through both courtyard gates.
    r(34.8,90,3.2,4.,0,1.1,'wood')
    r(35.3,90.6,2.2,2.7,1.1,1.2,'wood')
    r(34.6,89.8,3.6,4.4,3.0,.1,'cloth_red')
    for xx in (34.7,38.1):
        for yy in (90,93.8):r(xx,yy,.13,.13,1.1,1.9,'wood_decor')
    # Long flanks have local cover and different corner positions.
    r(13,94,4.5,3.0,0,2.25,'plaster')
    r(54.5,85,4.5,3.0,0,2.25,'blue')
    # Two broad shelters behind the south front protect spawn approaches.
    r(20,106,4.,.55,0,2.2,'brick')
    r(48,106,4.,.55,0,2.2,'brick')
    cx,cy,_=point(36,110,0)
    prism(cx,cy,0,1.3,.85,'sandstone/stone',16)
    prism(cx,cy,.86,1.05,.04,'sandstone/water',16)


def materials():
    texture=SOURCE/'textures/sandstone'; texture.mkdir(parents=True,exist_ok=True)
    rng=random.Random(438)
    palette=dict(brick=(134,81,50),stone=(156,136,106),paving=(132,119,102),blue=(87,124,137),
                 teal=(46,98,108),wood=(76,49,31),wood_decor=(76,49,31),window=(36,52,56),
                 cloth_red=(118,48,35),cloth_teal=(42,91,96),water=(67,109,120),tile=(114,147,147))
    for name,c in palette.items():
        image=Image.new('RGB',(256,256),c);draw=ImageDraw.Draw(image)
        if name in ('brick','paving','stone'):
            bw,bh=(64,32) if name=='brick' else (128,64)
            for row,y in enumerate(range(-bh,256+bh,bh)):
                for x in range(-bw,256+bw,bw):
                    xx=x+row%2*bw/2; tint=rng.randrange(-10,11)
                    cc=tuple(max(0,min(255,v+tint)) for v in c)
                    draw.rectangle((xx+1,y+1,xx+bw-2,y+bh-2),fill=cc)
                    draw.line((xx+2,y+2,xx+bw-2,y+2),fill=tuple(min(255,v+12) for v in cc))
        elif name in ('wood','wood_decor'):
            for x in range(0,256,32):
                draw.line((x,0,x,255),fill=(41,28,20),width=3)
                for j in range(4):
                    xx=x+rng.randint(4,28);draw.line((xx,0,xx+rng.randint(-2,2),255),fill=(59,39,25))
        elif name=='tile':
            for y in range(0,256,32):
                for x in range(0,256,32):
                    draw.rectangle((x+1,y+1,x+30,y+30),fill=(93,131,140))
                    draw.polygon([(x+16,y+5),(x+27,y+16),(x+16,y+27),(x+5,y+16)],fill=(163,160,127))
        elif name.startswith('cloth'):
            for x in range(0,256,24):draw.line((x,0,x,255),fill=tuple(min(255,v+16) for v in c),width=3)
        else:
            for _ in range(2800):
                xx,yy=rng.randrange(256),rng.randrange(256);v=rng.randrange(-5,6)
                draw.point((xx,yy),fill=tuple(max(0,min(255,k+v)) for k in c))
        image.save(texture/f'{name}.png')
    assert (texture/'plaster.png').exists(), 'Keep the authored plaster raster in source control.'
    env=SOURCE/'env/sandstone';env.mkdir(parents=True,exist_ok=True)
    gradient=np.zeros((256,256,3),dtype=np.uint8)
    for y in range(256):
        t=y/255;gradient[y,:,:]=np.array([101,150,188])*(1-t)+np.array([216,202,172])*t
    for face in ('rt','lf','ft','bk'):Image.fromarray(gradient).save(env/f'sky_{face}.tga')
    Image.new('RGB',(256,256),(101,150,188)).save(env/'sky_up.tga')
    Image.new('RGB',(256,256),(216,202,172)).save(env/'sky_dn.tga')
    Image.fromarray(gradient).save(texture/'sky.png')
    scripts=SOURCE/'scripts';scripts.mkdir(parents=True,exist_ok=True)
    shader=['textures/skies/sandstone','{','qer_editorimage textures/sandstone/sky.png',
            'surfaceparm sky','surfaceparm noimpact','surfaceparm nolightmap',
            'q3map_sun 1 0.93 0.82 45 135 55','q3map_skylight 22 3','skyparms env/sandstone/sky - -','}']
    for name in ('plaster',*palette):
        shader += [f'textures/sandstone/{name}','{',f'qer_editorimage textures/sandstone/{name}.png']
        if name in ('wood_decor','window','cloth_red','cloth_teal','water'):shader += ['surfaceparm nonsolid']
        shader += ['{',f'map textures/sandstone/{name}.png','rgbGen identity','}','}']
    (scripts/'sandstone.shader').write_text('\n'.join(shader)+'\n')
    (scripts/'common.shader').write_text('textures/common/clip\n{\nsurfaceparm nodraw\nsurfaceparm playerclip\n}\n')
    (scripts/'shaderlist.txt').write_text('sandstone\ncommon\n')

def author():
    r(-1,-1,74,122,-.65,.65,'paving',False)
    for x in (-.8,72):
        r(x,-1,.8,122,0,7.,'brick',False)
        box(*point(x+.4,60,7),.8,122,23,'skies/sandstone',False)
        r(x,-1,.8,122,6.9,23.1,'clip',False)
    for y in (-.8,120):
        r(-1,y,74,.8,0,7.,'brick',False)
        box(*point(36,y+.4,7),74,.8,23,'skies/sandstone',False)
        r(-1,y,74,.8,6.9,23.1,'clip',False)
    box(0,0,30,74,122,1,'skies/sandstone',False)
    box(0,0,29.9,74,122,.1,'common/clip',False)
    # Houses sit flush with the map boundary: no cramped back alleys behind them.
    for args in [(0,10,7,16,6.0),(0,35,7,18,6.2),(65,10,7,16,6.2),(65,35,7,18,6.0),
                 (8,0,14,6,5.8),(50,0,14,6,6.2),(0,54,25,16,6.3),(49,60,23,16,5.8),
                 (0,84,13,30,6.),(59,80,13,34,6.3),(0,114,29,6,5.8),(43,114,29,6,6.2)]:
        facade_block(*args)
    # Broad central gate is a visibly closed background landmark, not a fake exit.
    arch(36,.65,8.0,5.8,.65)
    r(32,.48,8.,.20,0,4.3,'teal')
    for xx in np.arange(32.5,40,.65):r(float(xx),.34,.10,.12,.10,4.1,'wood_decor')
    terrace(18,True);terrace(44,False)
    for stairs in STAIRS:steps(stairs['bottom'],stairs['top'],stairs['width'])
    # Four-metre upper crossing; the entire ground floor remains traversable.
    r(28,16,16,4,2.72,.28,'wood')
    for yy in (16.05,19.72):r(27.8,yy,16.4,.23,2.50,.22,'teal')
    for yy in (16.0,19.86):
        r(28,yy,16,.14,3.,.76,'teal')
        r(28,yy-.02,16,.18,3.76,.10,'wood_decor')
    # A single pier supports the bridge and breaks the direct spawn-to-spawn ray.
    r(35.3,16.4,1.4,3.2,0,.20,'stone')
    r(35.4,16.5,1.2,3.0,.20,2.3,'brick')
    r(35.25,16.35,1.5,3.3,2.50,.22,'stone')
    # Flush paving identifies the central loop, rather than steps at every junction.
    r(28.1,20.1,15.8,26.,0,.018,'stone')
    cx,cy,_=point(36,48,0)
    prism(cx,cy,0,1.80,.65,'sandstone/stone',16)
    prism(cx,cy,.66,1.45,.04,'sandstone/water',16)
    prism(cx,cy,.70,.24,1.40,'sandstone/stone',12)
    prism(cx,cy,1.85,.8,.15,'sandstone/stone',16)
    # Shops hug the outer facades; counters never narrow the mandatory stair bypasses.
    for x,y,cloth in [(7.2,38,'cloth_red'),(63.1,40,'cloth_teal')]:
        r(x,y,1.7,4.,0,1.05,'wood')
        r(x-.05,y-.05,1.8,4.1,1.05,.12,'stone')
        r(x-.05,y-.15,1.85,4.3,2.9,.10,cloth)
        for yy in (y,y+3.9):r(x+.8,yy,.12,.12,1.17,1.73,'wood_decor')
    # Broad returns protect spawn areas and make long streets change direction.
    for x in (15.5,56.05):r(x,6,.45,5.5,0,2.2,'blue')
    r(32.5,7.5,1.5,5.,0,2.2,'brick')
    for x in (7,61):r(x,28,4.,2.,0,2.2,'plaster')
    r(26.5,50,5.,.45,0,2.2,'brick')
    r(40.5,50,5.,.45,0,2.2,'brick')
    r(35,40.5,2.,3.,0,2.2,'wood')
    # A few deliberate cover islands, with no thin poles or random boxes in doorways.
    for x,y,w,d,h in [(12.5,40,2.6,1.4,1.20),(56.9,42,2.6,1.4,1.20),
                      (27,47,3.0,1.2,1.15),(42,47,3.,1.2,1.15)]:
        r(x,y,w,d,0,h,'brick');r(x-.04,y-.04,w+.08,d+.08,h,.10,'stone')
    # South court landmark is wide enough to move around on either side.
    r(33.5,66,5,2.,.35,.90,'wood')
    r(34.,66.2,1.6,1.6,1.25,.95,'wood')
    r(36.,66.2,1.8,1.6,1.25,1.25,'wood')
    for xx in (34.,37.5):
        for yy in (65.9,68.1):
            cx,cy,_=point(xx,yy,.50);oval((cx,cy,.50),.47,.47,.16,'sandstone/wood','xz',12)
    caravanserai()
    for x,y,z,yaw in SPAWNS:entity('info_player_deathmatch',point(x,y,z+.65),angle=str(yaw))
    for x,y,z in PICKUPS:entity('item_health_medium',point(x,y,z+.65))
    for x,y,z in [(23,21,2.2),(23,27,2.2),(49,21,2.2),(49,27,2.2)]:entity('light',point(x,y,z),light='45',_color='1 .91 .78')
    props=dict(classname='worldspawn',message='Sandstone Quarter',_blocksize='8192',
               _description='An elongated sequence of bazaar, fountain square and caravanserai courts. Broad offset connections, one connected roof loop and four real stair destinations.')
    MAP.write_text('// Game: Boltrunner\n// Format: Quake3 (Valve)\n{\n'+'\n'.join(f'"{k}" "{v}"' for k,v in props.items())+'\n'+'\n'.join(BRUSHES)+'\n}\n'+'\n'.join(ENTITIES)+'\n')
    (SOURCE/'layout.json').write_text(json.dumps(dict(bounds=[72,120],spawns=SPAWNS,pickups=PICKUPS,
        reference_urls=['https://walledcitylahore.gop.pk/havelis/','https://whc.unesco.org/en/tentativelists/6108/'],
        scale='metres',design_references=['https://book.leveldesignbook.com/process/layout/flow','https://book.leveldesignbook.com/process/combat/balance'],zones={'bazaar':[0,0,72,38],'fountain-square':[0,38,72,78],'caravanserai':[0,78,72,120]},routes=ROUTES,stairs=STAIRS,elevations=[3.0],minimum_main_bypass_width=5.0,
        roof_bounds=[[18,16,28,30,3],[44,16,54,30,3]],bridge=[28,16,44,20,3]),indent=2)+'\n')
    print(f'Authored {len(BRUSHES)} brushes, {len(ENTITIES)} entities → {MAP}')

if __name__=='__main__':
    materials();author()
