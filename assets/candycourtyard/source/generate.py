#!/usr/bin/env python3
"""Editable metre-scale Candy Courtyard brush authoring. Bake with import-map.py."""
from pathlib import Path
import math
import json
import numpy as np
from PIL import Image, ImageDraw

SOURCE = Path(__file__).resolve().parent
MAP = SOURCE.parent / 'candycourtyard.map'
BRUSHES, ENTITIES = [], []
XY_SCALE = 1.4
Z_SCALE = 1.2
CUBE = [(0,1,2,3),(4,7,6,5),(0,4,5,1),(1,5,6,2),(2,6,7,3),(3,7,4,0)]

def brush(vertices, faces, material, detail=True):
    vertices = np.asarray(vertices, dtype=float) * [XY_SCALE, XY_SCALE, Z_SCALE] * 40
    center = vertices.mean(axis=0)
    lines = ['{']
    for face in faces:
        a,b,c = (vertices[i] for i in face[:3])
        n = np.cross(c-a,b-a)
        if np.dot(n,a-center) < 0: b,c,n = c,b,-n
        axis = np.argmax(abs(n))
        uv = ('[ 0 1 0 0 ] [ 0 0 -1 0 ]','[ 1 0 0 0 ] [ 0 0 -1 0 ]','[ 1 0 0 0 ] [ 0 -1 0 0 ]')[axis]
        points = ' '.join('( '+' '.join(f'{v:.4f}' for v in p)+' )' for p in (a,b,c))
        lines.append(f'{points} {material} {uv} 0 0.25 0.25 {134217728 if detail else 0} 0 0')
    BRUSHES.append('\n'.join([*lines,'}']))

def box(x,y,z,w,d,h,mat,detail=True):
    brush([(x-w/2,y-d/2,z),(x+w/2,y-d/2,z),(x+w/2,y+d/2,z),(x-w/2,y+d/2,z),
           (x-w/2,y-d/2,z+h),(x+w/2,y-d/2,z+h),(x+w/2,y+d/2,z+h),(x-w/2,y+d/2,z+h)], CUBE,mat,detail)

def inlaid_floor(x,y,z,w,d,h,mat,inlays):
    """One physical floor, partitioned into adjacent colours, never stacked decals."""
    left,right=x-w/2,x+w/2; bottom,top=y-d/2,y+d/2
    # Keep the ceiling below one uninterrupted structural slab: splitting its
    # full depth creates internal vertical collision faces inside the room.
    skin=min(.012/Z_SCALE,h/4)
    box(x,y,z,w,d,h-skin,mat)
    patches=[]
    for px,py,pw,pd in inlays:
        a,b=max(left,px-pw/2),min(right,px+pw/2)
        c,e=max(bottom,py-pd/2),min(top,py+pd/2)
        if b>a and e>c: patches.append((a,b,c,e))
    xs=sorted({left,right,*(v for p in patches for v in p[:2])})
    ys=sorted({bottom,top,*(v for p in patches for v in p[2:])})
    for a,b in zip(xs,xs[1:]):
        for c,e in zip(ys,ys[1:]):
            cx,cy=(a+b)/2,(c+e)/2
            paint='candy/peach' if any(l<cx<r and low<cy<high for l,r,low,high in patches) else mat
            box(cx,cy,z+h-skin,b-a,e-c,skin,paint)

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

def bridge(a,b,width,top,mat='candy/cream'):
    a,b=np.array(a,dtype=float),np.array(b,dtype=float)
    v=b-a; side=np.array([-v[1],v[0]])
    side*=width/2/np.linalg.norm(side)
    points=[a-side,a+side,b+side,b-side]
    brush([(x,y,z) for z in (top-.25,top) for x,y in points],CUBE,mat)

def stair(a,b,width,mat='candy/cream'):
    a,b=np.array(a,dtype=float),np.array(b,dtype=float)
    delta=b-a; run=np.linalg.norm(delta[:2]); direction=delta[:2]/run
    side=np.array([-direction[1],direction[0]])*width/2
    rise=delta[2]; count=math.ceil(abs(rise)*Z_SCALE/.17)
    landing=(.6 if run*XY_SCALE<7 else .9)/XY_SCALE if count>12 else 0
    step_run=(run-landing)/count
    for i in range(count):
        offset=i*step_run+(landing if i>=count//2 else 0)
        lo=a[:2]+direction*offset
        hi=a[:2]+direction*(offset+step_run)
        if i==count//2 and landing:
            l0=lo-direction*landing
            brush([(x,y,z) for z in (a[2]-.25,a[2]+rise*i/count) for x,y in [l0-side,l0+side,lo+side,lo-side]],CUBE,mat)
        top=a[2]+rise*(i+1)/count
        brush([(x,y,z) for z in (a[2]-.25,top) for x,y in [lo-side,lo+side,hi+side,hi-side]],CUBE,mat)
    # Rounded slim handrail and real posts; geometry stays outside the walk width.
    for sign in (-1,1):
        edge=side*sign
        for i in range(6):
            t=i/5; xy=a[:2]+delta[:2]*t+edge
            zz=a[2]+rise*t
            box(*xy,zz,.07,.07,.85,'candy/rose')
        beam([*(a[:2]+edge),a[2]+.85],[*(b[:2]+edge),b[2]+.85],.065,'candy/rose')

def polygon_deck(points,top,thickness,mat):
    n=len(points)
    brush([(x,y,z) for z in (top-thickness,top) for x,y in points],
          [tuple(range(n)),tuple(range(n,2*n))]+[(i,(i+1)%n,(i+1)%n+n,i+n) for i in range(n)],mat)

def subtract_convex(subject,clip):
    """Disjoint convex pieces outside a CCW polygon, keeping floor faces flush."""
    remaining=[np.array(p,dtype=float) for p in subject]; result=[]
    for a,b in zip(clip,clip[1:]+clip[:1]):
        if not remaining:break
        a,b=np.array(a),np.array(b); edge=b-a
        cross=lambda p:edge[0]*(p[1]-a[1])-edge[1]*(p[0]-a[0])
        def half(inside):
            out=[]
            for p,q in zip(remaining,remaining[1:]+remaining[:1]):
                dp,dq=cross(p),cross(q); pin=dp>=-1e-8 if inside else dp<=1e-8; qin=dq>=-1e-8 if inside else dq<=1e-8
                if pin:out.append(p)
                if pin!=qin:out.append(p+(q-p)*dp/(dp-dq))
            return out
        outside=half(False)
        if len(outside)>=3 and abs(sum(p[0]*q[1]-p[1]*q[0] for p,q in zip(outside,outside[1:]+outside[:1])))>1e-6:result.append(outside)
        remaining=half(True)
    return result

def smooth_ramp(a,b,width,axis,paint):
    """A thin deck level across its width; optional axial cuts meet gallery edges."""
    a,b=np.array(a,dtype=float),np.array(b,dtype=float); delta=b-a
    run=np.linalg.norm(delta[:2]); normal=np.array([-delta[1],delta[0]])/run
    side=normal.copy() if axis is None else np.array([0.,1.]) if axis=='x' else np.array([1.,0.])
    side*=width/(2*abs(np.dot(side,normal)))
    corners=[a[:2]-side,a[:2]+side,b[:2]+side,b[:2]-side]
    thickness=.22/Z_SCALE
    verts=[(*xy,z) for xy,z in [(corners[0],a[2]-thickness),(corners[1],a[2]-thickness),(corners[2],b[2]-thickness),(corners[3],b[2]-thickness),
                              (corners[0],a[2]),(corners[1],a[2]),(corners[2],b[2]),(corners[3],b[2])]]
    brush(verts,CUBE,paint)
    # Edge fascia is below the walking plane: strafes and dashes have no railing to snag.
    for sign in (-1,1):
        beam([*(a[:2]+side*sign),a[2]-.12/Z_SCALE],[*(b[:2]+side*sign),b[2]-.12/Z_SCALE],.07,'candy/rose')
    if a[2]==0:
        # Make low headroom visibly inaccessible with an open cocoa trellis.
        # The higher opening stays >=2.2m tall, so there is no crawlspace trap.
        cutoff=(2.2+.22)/(b[2]*Z_SCALE)
        count=math.ceil(run*XY_SCALE*cutoff/.42)
        for i in range(1,count+1):
            t=cutoff*i/count; p=a+delta*t; h=p[2]-thickness
            if h<=0:continue
            for sign in (-1,1):
                xy=p[:2]+side*sign
                furniture_box(*xy,0,.055,.055,h*Z_SCALE,'cocoa')
        p=a+delta*cutoff
        count=math.ceil(np.linalg.norm(side)*2*XY_SCALE/.42)
        for i in range(count+1):
            xy=p[:2]+side*(2*i/count-1)
            furniture_box(*xy,0,.055,.055,2.2,'cocoa')
    return corners

# Human-scale gingerbread shops have open doorways, framed windows and pitched roofs.
def house(x,y,w,d,paint,base=0,front=-1):
    height=2.8
    box(x,y,base,w,d,.12,'candy/cream')
    for sign in (-1,1):
        box(x+sign*(w/2-.14),y,base,.28,d,height,f'candy/{paint}')
    door=1.65
    yy=y+front*(d/2-.14)
    for sign in (-1,1):
        ww=(w-door)/2
        box(x+sign*(door/2+ww/2),yy,base,ww,.28,height,f'candy/{paint}')
    box(x,yy,base+2.25,door,.28,height-2.25,'candy/white')
    box(x,y-front*(d/2-.14),base,w,.28,height,f'candy/{paint}')
    # Flat eave inside, steep gable outside: this is a building, not a coloured cube.
    box(x,y,base+height,w+.18,d+.18,.16,'candy/white')
    for sign in (-1,1):
        extrusion([(-w/2-.15,0),(w/2+.15,0),(0,1.7)],(x,y+sign*(d/2-.15),base+height+.16),.3,'candy/blush')
    brush([(x-w/2-.2,y-d/2-.25,base+height+.16),(x+w/2+.2,y-d/2-.25,base+height+.16),(x+w/2+.2,y+d/2+.25,base+height+.16),(x-w/2-.2,y+d/2+.25,base+height+.16),
           (x,y-d/2-.25,base+height+1.96),(x,y+d/2+.25,base+height+1.96)],[(0,1,2,3),(0,4,1),(3,2,5),(0,3,5,4),(1,4,5,2)],'candy/rose')
    for sign in (-1,1):
        xx=x+sign*(w*.29)
        box(xx,yy+front*.17,base+.9,.7,.035,1.1,'candy/mint')
        for offset in (-.39,.39):box(xx+offset,yy+front*.20,base+.82,.065,.06,1.26,'candy/white')
        for zz in (base+.82,base+1.43,base+2.02):box(xx,yy+front*.20,zz,.85,.06,.065,'candy/white')
    for side in (-1,1):
        xx=x+side*(w/2+.018)
        for yy in (y-d*.24,y+d*.24):
            box(xx,yy,base+.9,.035,.9,1.05,'candy/mint')
            for off in (-.5,.5):box(xx+side*.018,yy+off,base+.84,.065,.06,1.17,'candy/white')
            for zz in (base+.84,base+1.4,base+2.01):box(xx+side*.018,yy,zz,.065,1.06,.06,'candy/white')
    # Start below the roof eave; the pitched roof always surrounds the chimney base.
    box(x-w*.32,y+front*(d*.28),base+height+.14,.45,.45,2.1,'candy/cream')
    bow((x,yy+front*.22,base+2.7),.20)
    entity('light',(x,y,base+2.1),light='45',_color='1 .96 .98',_extradist='100')

def furniture_box(x,y,z,w,d,h,mat):
    # Furniture dimensions are physical metres, independent of the district scale.
    box(x,y,z,w/XY_SCALE,d/XY_SCALE,h/Z_SCALE,f'candy/{mat}')

def chair(x,y,z,back=1,paint='lavender',axis='y'):
    for dx in (-.19,.19):
        for dy in (-.19,.19):furniture_box(x+dx/XY_SCALE,y+dy/XY_SCALE,z,.055,.055,.44,'cocoa')
    furniture_box(x,y,z+.44/Z_SCALE,.52,.52,.085,paint)
    if axis=='y':furniture_box(x,y+back*.23/XY_SCALE,z+.52/Z_SCALE,.52,.075,.48,paint)
    else:furniture_box(x+back*.23/XY_SCALE,y,z+.52/Z_SCALE,.075,.52,.48,paint)

def table(x,y,z,w=1.2,d=.75,paint='cream'):
    for dx in (-w*.38,w*.38):
        for dy in (-d*.35,d*.35):furniture_box(x+dx/XY_SCALE,y+dy/XY_SCALE,z,.065,.065,.71,'cocoa')
    furniture_box(x,y,z+.71/Z_SCALE,w,d,.075,paint)

def cafe_table(x,y,z,paint='blush',axis='y'):
    prism(x,y,z,.095/XY_SCALE,.70/Z_SCALE,'candy/cocoa',8)
    prism(x,y,z,.32/XY_SCALE,.045/Z_SCALE,'candy/cocoa',12)
    prism(x,y,z+.70/Z_SCALE,.59/XY_SCALE,.075/Z_SCALE,f'candy/{paint}',16)
    for side in (-1,1):
        cx=x+side*.96/XY_SCALE if axis=='x' else x
        cy=y+side*.96/XY_SCALE if axis=='y' else y
        chair(cx,cy,z,back=side,paint='mint' if side<0 else 'lavender',axis=axis)
    # A little ceramic cup is authored geometry, with an inset dark drink surface.
    prism(x+.14/XY_SCALE,y,z+.775/Z_SCALE,.06/XY_SCALE,.12/Z_SCALE,'candy/white',8)
    prism(x+.14/XY_SCALE,y,z+.895/Z_SCALE,.043/XY_SCALE,.009/Z_SCALE,'candy/cocoa',8)

def sofa(x,y,z,w=2.5,paint='blush',facing=-1):
    furniture_box(x,y,z,w,.83,.21,'cocoa')
    furniture_box(x,y,z+.21/Z_SCALE,w-.16,.75,.25,paint)
    furniture_box(x,y-facing*.33/XY_SCALE,z+.42/Z_SCALE,w,.16,.53,paint)
    for side in (-1,1):furniture_box(x+side*(w/2-.09)/XY_SCALE,y,z+.21/Z_SCALE,.18,.83,.39,paint)
    for side in (-1,1):furniture_box(x+side*w*.23/XY_SCALE,y+facing*.055/XY_SCALE,z+.46/Z_SCALE,w*.40,.48,.055,'cream')

def shelf(x,y,z,w=1.25,paint='cream',contents='books',facing=-1):
    depth=.42; height=1.62
    for side in (-1,1):furniture_box(x+side*(w/2-.035)/XY_SCALE,y,z,.07,depth,height,paint)
    furniture_box(x,y-facing*.19/XY_SCALE,z,w,.045,height,'cocoa')
    for level in (0,.52,1.04,1.55):
        furniture_box(x,y,z+level/Z_SCALE,w,depth,.07,paint)
        if level>1.1:continue
        for j in range(4):
            xx=x+(j-1.5)*(w*.19)/XY_SCALE
            if contents=='books':
                furniture_box(xx,y+facing*.045/XY_SCALE,z+(level+.07)/Z_SCALE,w*.13,.21,.28+(.045 if j%2 else 0),('mint','rose','lavender','peach')[j])
            else:
                prism(xx,y,z+(level+.07)/Z_SCALE,.09/XY_SCALE,.14/Z_SCALE,'candy/cream',8)
                prism(xx,y,z+(level+.21)/Z_SCALE,.10/XY_SCALE,.045/Z_SCALE,('candy/rose','candy/mint','candy/peach','candy/lavender')[j],8)

def wardrobe(x,y,z,facing=-1):
    furniture_box(x,y,z,1.18,.58,1.8,'cocoa')
    for side in (-1,1):
        furniture_box(x+side*.29/XY_SCALE,y+facing*.30/XY_SCALE,z+.045/Z_SCALE,.55,.055,1.69,'cream')
        furniture_box(x+side*.055/XY_SCALE,y+facing*.335/XY_SCALE,z+.87/Z_SCALE,.025,.045,.14,'gold')

def bed(x,y,z,facing=-1):
    furniture_box(x,y,z,1.25,2.0,.23,'cocoa')
    furniture_box(x,y,z+.23/Z_SCALE,1.22,1.95,.24,'white')
    furniture_box(x,y+facing*.28/XY_SCALE,z+.47/Z_SCALE,1.22,1.36,.05,'lavender')
    furniture_box(x,y-facing*.64/XY_SCALE,z+.47/Z_SCALE,.82,.43,.10,'cream')
    furniture_box(x,y-facing*.97/XY_SCALE,z,1.28,.09,.92,'blush')

def workbench(x,y,z,monitor=False):
    table(x,y,z,w=1.15,d=.62)
    furniture_box(x+.32/XY_SCALE,y,z,.34,.57,.70,'cocoa')
    if monitor:
        furniture_box(x,y,z+.79/Z_SCALE,.20,.16,.10,'cocoa')
        furniture_box(x,y+.15/XY_SCALE,z+.89/Z_SCALE,.65,.05,.40,'cocoa')
        for height in (.89,1.25):furniture_box(x,y+.1025/XY_SCALE,z+height/Z_SCALE,.65,.045,.04,'cocoa')
        for side in (-1,1):furniture_box(x+side*.3025/XY_SCALE,y+.1025/XY_SCALE,z+.93/Z_SCALE,.045,.045,.32,'cocoa')
        furniture_box(x,y+.09/XY_SCALE,z+.93/Z_SCALE,.56,.02,.32,'mint')
        furniture_box(x,y-.12/XY_SCALE,z+.787/Z_SCALE,.52,.18,.028,'lavender')
    chair(x,y-.87/XY_SCALE,z,back=-1)

def furnish_houses():
    floor=.12
    # Northwest lounge: soft seats, coffee table and a bookcase, clear centre aisle.
    sofa(-23.65,20.3,floor)
    table(-23.4,19.25,floor,w=1.0,d=.62,paint='peach')
    shelf(-20.05,20.35,floor,w=1.15,contents='books')
    # Northeast tearoom: two social table clusters away from the entrance axis.
    cafe_table(20.25,18.4,floor)
    cafe_table(23.75,20.0,floor,paint='cream')
    # Southwest bedroom: a real low bed, storage and a desk.
    bed(-23.65,-19.25,floor,facing=1)
    wardrobe(-20.05,-20.15,floor,facing=1)
    workbench(-20.05,-18.0,floor)
    # Southeast candy storeroom: jars, a pastry worktop and low shipping boxes.
    shelf(20.05,-19.8,floor,contents='sweets',facing=1)
    shelf(23.8,-19.8,floor,paint='lavender',contents='sweets',facing=1)
    table(20.35,-17.6,floor,w=1.35,d=.78,paint='peach')
    furniture_box(23.8,-17.8,floor,.68,.68,.58,'cocoa')
    furniture_box(23.8,-17.8,floor+.58/Z_SCALE,.54,.54,.32,'cream')
    # West roof reading nook and east roof game room have narrower furniture.
    roof=2.5+floor
    sofa(-20.95,6.4,roof,w=1.08,paint='mint')
    table(-20.95,4.55,roof,w=.58,d=.58,paint='peach')
    shelf(-19.15,7.0,roof,w=.86,contents='books')
    workbench(20.95,6.45,roof,monitor=True)
    shelf(19.15,7.0,roof,w=.85,contents='books')
    # Ground bakery retains its two through doorways and pickup clearance.
    shelf(-20.2,8.25,0,w=1.45,contents='sweets')
    shelf(-14.15,-8.25,0,w=1.35,contents='sweets',facing=1)
    table(-20.2,4.8,0,w=1.8,d=.80,paint='peach')
    # Ground cafe: table islands and a rear bench; broad combat paths remain open.
    cafe_table(14.3,8.0,0,axis='x')
    cafe_table(20.25,8.0,0,paint='cream',axis='x')
    sofa(20.15,-7.7,0,w=2.10,paint='lavender',facing=1)


def entity(classname,pos=None,**props):
    values=dict(classname=classname,**props)
    if pos is not None: values['origin']=' '.join(f'{x*k*40:.4f}' for x,k in zip(pos,(XY_SCALE,XY_SCALE,Z_SCALE)))
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

def bow(center,scale=1,axis='xz'):
    cx,cy,cz=center
    for sign in (-1,1):
        points=[(sign*.25*scale,-.32*scale),(sign*2.0*scale,-1.1*scale),
                (sign*2.3*scale,-.7*scale),(sign*2.3*scale,.7*scale),
                (sign*2.0*scale,1.1*scale),(sign*.25*scale,.32*scale)]
        extrusion(points,center,.4*scale,'candy/rose',axis)
        # Rounded pearl highlight on each lobe.
        c=(cx+sign*1.4*scale,cy-.25*scale,cz+.2*scale) if axis=='xz' else (cx-.25*scale,cy+sign*1.4*scale,cz+.2*scale)
        oval(c,.55*scale,.55*scale,.04*scale,'candy/blush',axis,12)
    oval((cx,cy-.3*scale,cz) if axis=='xz' else (cx-.3*scale,cy,cz),.42*scale,.48*scale,.2*scale,'candy/rose',axis,12)

def kitty():
    # Original brush-built face with ears, whiskers and bow: visible from the plaza.
    center=(16.7,0,6.0)
    oval(center,2.4,1.7,.5,'candy/white','yz',24)
    for sign in (-1,1):
        extrusion([(sign*1.0,1.1),(sign*2.0,2.5),(sign*2.3,.9)],center,.5,'candy/white','yz')
        oval((16.4,sign*.85,6.0),.18,.25,.05,'candy/ink','yz',12)
        for angle in (-.22,0,.22):
            y0=sign*1.6; y1=sign*2.6
            beam((16.35,y0,5.55),(16.35,y1,5.55+angle),.075,'candy/ink')
    oval((16.35,0,5.65),.22,.15,.08,'candy/gold','yz',12)
    bow((16.3,1.7,7.5),.45,'yz')
    box(17,0,2.5,.55,.55,2.0,'candy/cream')

def lollipop(x,y,flavor):
    prism(x,y,0,.12,2.7,'candy/white',8)
    oval((x,y,3.25),.92,.92,.32,f'candy/{flavor}','xz',16)
    oval((x,y-.18,3.25),.63,.63,.03,'candy/white','xz',16)
    oval((x,y-.21,3.25),.35,.35,.03,f'candy/{flavor}','xz',16)

def macaron(x,y,width=3.2):
    r=width/2
    prism(x,y,0,r,.53/Z_SCALE,'candy/lavender',12,top=r*.97)
    prism(x,y,.53/Z_SCALE,r*.94,.22/Z_SCALE,'candy/cream',12)
    prism(x,y,.75/Z_SCALE,r*.97,.6/Z_SCALE,'candy/rose',12,top=r*.85)

def materials():
    texture=SOURCE/'textures/candy'; texture.mkdir(parents=True,exist_ok=True)
    palette=dict(cream=(244,223,184),white=(249,242,239),mint=(176,226,206),
                 lavender=(177,157,207),rose=(224,109,159),blush=(249,174,201),
                 ink=(77,51,74),gold=(236,181,73),cocoa=(157,113,106),peach=(238,185,151))
    # Flat satin swatches support geometric confectionery; detailed surfaces use authored raster assets.
    for name,color in palette.items():
        Image.new('RGB',(64,64),color).save(texture/f'{name}.png')
    image=Image.new('RGB',(256,256),palette['cream']); draw=ImageDraw.Draw(image)
    for y in range(0,256,32):
        for x in range(0,256,32):
            draw.rounded_rectangle((x+2,y+2,x+29,y+29),3,fill=(213,164,114),outline=(241,201,155),width=3)
    image.save(texture/'waffle.png')
    # An authored clear pastel gradient cubemap has no projection seam or bitmap logos.
    colors=np.zeros((256,256,3),dtype=np.uint8)
    for y in range(256):
        t=y/255
        colors[y,:,:]=np.array([184,216,241])*(1-t)+np.array([248,215,232])*t
    for face in ('rt','lf','bk','ft'): Image.fromarray(colors).save(SOURCE/f'env/candy/sky_{face}.tga')
    Image.new('RGB',(256,256),(184,216,241)).save(SOURCE/'env/candy/sky_up.tga')
    Image.new('RGB',(256,256),(248,215,232)).save(SOURCE/'env/candy/sky_dn.tga')
    Image.fromarray(colors).save(SOURCE/'textures/candy/sky.png')
    # Current renderer applies Xonotic's 4.672 baked-light compensation without
    # tone mapping: author soft daylight here rather than clipping pastel floors.
    shader=['textures/skies/candy','{','qer_editorimage textures/candy/sky.png','surfaceparm sky','surfaceparm noimpact','surfaceparm nolightmap','q3map_sun 1 0.94 0.97 50 225 65','q3map_skylight 25 3','skyparms env/candy/sky - -','}']
    for name in ['fondant','biscuit',*palette,'waffle']:
        shader += [f'textures/candy/{name}','{',f'qer_editorimage textures/candy/{name}.png','{',f'map textures/candy/{name}.png','rgbGen identity','}','}']
    (SOURCE/'scripts/candy.shader').write_text('\n'.join(shader)+'\n')
    (SOURCE/'scripts/shaderlist.txt').write_text('candy\ncommon\n')
    (SOURCE/'scripts/common.shader').write_text('textures/common/clip\n{\nsurfaceparm nodraw\nsurfaceparm playerclip\n}\ntextures/common/trigger\n{\nqer_editorimage textures/candy/rose.png\nsurfaceparm nodraw\nsurfaceparm nonsolid\nsurfaceparm nolightmap\nsurfaceparm trans\n}\n')

def author():
    # Structural sealed enclosure and visible low walls; sky occupies upper boundary.
    box(0,0,-.64,58,50,.6,'candy/biscuit',False)
    for x in (-28.6,28.6):
        box(x,0,-.1,1.2,50,6.1,'candy/fondant',False)
        box(x,0,6,1.2,50,18,'skies/candy',False)
        box(x,0,5.7,1.35,50,.3,'candy/white')
    for y in (-24.6,24.6):
        box(0,y,-.1,56,1.2,6.1,'candy/fondant',False)
        box(0,y,6,56,1.2,18,'skies/candy',False)
        box(0,y,5.7,56,1.35,.3,'candy/white')
    box(0,0,24,58,50,1,'skies/candy',False)
    # Seal every side up to the sky: double-jumps cannot escape above decorative walls.
    for x in (-28.6,28.6):box(x,0,5.9,1.2,50,18.1,'common/clip',False)
    for y in (-24.6,24.6):box(0,y,5.9,56,1.2,18.1,'common/clip',False)
    box(0,0,23.9,58,50,.1,'common/clip',False)
    # Large mint route and cream centre; flat painted inlays create no raised collision seams.
    box(0,0,-.04,54,46,.04,'candy/mint')
    box(0,0,0,23.5,26,.025,'candy/biscuit')
    box(0,-17.7,0,33,5.8,.025,'candy/waffle')
    # Two roofed shops: open inner doorways and ground-level through routes.
    for sign,paint in [(-1,'fondant'),(1,'lavender')]:
        x=sign*17
        inlaid_floor(x,0,2.25,10,19.2,.25,'candy/cream',
                     [(-19,-6.8,2.6,2.6),(-15,-6.8,2.6,2.6)] if sign<0 else [(17,-6.8,2.6,2.6)])
        box(sign*21.75,0,0,.5,19.2,2.25,f'candy/{paint}')
        for y in (-9.35,9.35):
            box(x,y,0,10,.5,2.25,f'candy/{paint}')
        # Inner storefront walls, two generous 3m doorways and visible lintels.
        xx=sign*12.25
        for lo,hi in [(-9.6,-6.5),(-3.5,5.0),(8.0,9.6)]:
            box(xx,(lo+hi)/2,0,.5,hi-lo,2.25,f'candy/{paint}')
        for y in (-5,6.5):
            box(xx,y,2.0,.5,3,.25,'candy/white')
            for edge in (-1.55,1.55):box(sign*11.96,y+edge,0,.12,.15,2.25,'candy/cream')
        # Framed candy-shop display windows turn the blank slab into a readable façade.
        for y in (-1.9,.8,3.5):
            front=sign*11.96
            box(front,y,.82,.065,1.9,1.05,'candy/mint' if sign<0 else 'candy/blush')
            for edge in (-1.0,1.0):box(front-sign*.045,y+edge,.76,.10,.08,1.18,'candy/white')
            for z in (.76,1.32,1.87):box(front-sign*.045,y,z,.10,2.08,.07,'candy/white')
            box(front-sign*.045,y,.76,.10,.08,1.18,'candy/white')
            box(front-sign*.08,y,1.98,.32,2.2,.12,'candy/rose' if sign<0 else 'candy/lavender')
        for y in (-8.3,8.8):
            box(sign*11.94,y,.9,.10,.7,.9,'candy/cream')
            bow((sign*11.85,y,1.4),.18,'yz')
        # Striped roof edging without an enclosing parapet: jumps can leave every side.
        for yy in np.arange(-9.2,9.3,1.2):
            if abs(float(yy)+6.8)<1.5: continue  # Capsule-wide unobstructed zipline exit.
            box(sign*12.15,float(yy),2.5,.35,.55,.22,'candy/rose' if sign<0 else 'candy/mint')
        # Ground shop cover, open on both sides.
        box(x,-1.8,0,4.7,1.4,1.20/Z_SCALE,'candy/cocoa')
        box(x,-1.8,1.20/Z_SCALE,4.85,1.5,.10/Z_SCALE,'candy/white')
        # Ordinary stair access from outer loop onto each roof.
        stair((sign*24,-13,0),(sign*24,-7.5,2.5),2.3)
        box(sign*22.5,-6.35,2.25,4,2.3,.25,'candy/cream')
    # Four additional village houses articulate the outer circuit and offer real interiors.
    for sign in (-1,1):
        house(sign*22,19.0,5.7,5.0,'blush' if sign<0 else 'lavender',front=-1)
        house(sign*22,-18.6,5.7,4.5,'peach' if sign<0 else 'mint',front=1)
        # Taller, recognisable roof storefronts; leave both cable lanes clear.
        house(sign*20,5.4,3.4,5.0,'cream',base=2.5,front=-1)
    furnish_houses()
    kitty()
    # Elevated north gallery, open underneath and accessible from ordinary stairs.
    inlaid_floor(0,17.5,2.25,33,5.8,.25,'candy/lavender',[(-12.1,15.5,2.6,2.6)])
    for x in (-15.8,15.8):
        box(x,17.5,0,.5,.5,2.25,'candy/cream')
        if x>0:stair((x,10.5,0),(x,14.6,2.5),2.7)
    for x in np.arange(-15,16,2.0):
        box(float(x),20.2,2.5,.8,.4,.55,'candy/white')
    # Open pavilion, accessible from two stairs and launch pads.
    prism(0,0,5.15,6.6,.35,'candy/cream',24)
    prism(0,0,5.5,6.75,.08,'candy/rose',24)
    for x in (-4.4,4.4):
        for y in (-3.8,3.8): prism(x,y,0,.33,5.15,'candy/cream',8)
    stair((8.2,14.6,2.5),(8.2,5.8,5.5),3.0)
    bridge((8.2,5.8),(4.8,3.8),2.25,5.5)
    # Western promenade: two broad calm slopes joined by a real combat floor.
    smooth_ramp((-25.3,13.7,0),(-16.5,13.7,2.5),3.0,'x','candy/cream')
    upper=smooth_ramp((-9.8,13.98,2.5),(-6.0,5.4,5.58),3.0,None,'candy/cream')
    shoulder=upper[0][0]+16.5
    box(-16.5+shoulder/2,13.4,2.25,shoulder,2.4,.25,'candy/lavender')
    polygon_deck([upper[0],upper[1],(upper[1][0],14.6),(upper[0][0],14.6)],2.5,.25,'candy/cream')
    # A full-width level apron meets the round pavilion without overlapping faces.
    side=(upper[2]-upper[3])/2
    apron=[upper[3],upper[2],np.array([-4.5,2.6])+side,np.array([-4.5,2.6])-side]
    circle=[(6.75*math.cos(i*math.tau/24),6.75*math.sin(i*math.tau/24)) for i in range(24)]
    for piece in subtract_convex(apron,circle):polygon_deck(piece,5.58,.22/Z_SCALE,'candy/rose')
    # Giant ribbon breaks high-level sightlines without sealing the pavilion.
    box(0,.9,5.58,.55,.55,2.45,'candy/white')
    bow((0,.9,8.2),1.4)
    # Low and full-height cover alternate; all are convex solids with readable silhouettes.
    for x,y,w,d,h in [(-7.7,3.3,2.1,4.7,2.3),(7.7,-3.6,2.1,4.7,2.3),(-7.7,-4.6,3.4,2.2,1.45),(7.7,6.1,3.4,2.2,1.45)]:
        # Low cover retains human scale after enlarging the district: 1.4m top.
        if h<2.0:
            h=1.30/Z_SCALE; cap=.10/Z_SCALE
        else:cap=.13
        box(x,y,0,w,d,h,'candy/fondant')
        box(x,y,h,w+.1,d+.1,cap,'candy/white')
    for x in (-20.5,20.5):
        for y in (-13.5,12.8):
            macaron(-20.,10.9,2.6) if x<0 and y>0 else macaron(x,y,2.6)
    # Wrapped gifts break spawn-to-spawn firing lanes; both sides stay walkable.
    for x,y in [(-25.5,10),(25.5,10),(-25.5,-10),(25.5,-10)]:
        box(x,y,0,1.4,1.4,2.3,'candy/lavender')
        box(x,y,2.3,1.5,1.5,.13,'candy/cream')
        box(x,y,0,.14,1.43,2.3,'candy/rose')
        bow((x,y,2.76),.3)
    for x,y,flavor in [(-11,20.5,'rose'),(11,20.5,'mint'),(-11,-16,'lavender'),(11,-16,'peach')]: lollipop(x,y,flavor)
    # Small dimensional bows decorate the shops without bitmap signage.
    for sign in (-1,1):
        for y in (-7.5,0,7.5): bow((sign*21.7,y,3.4),.33,'yz')
    # Paired pad shortcuts land at the pavilion's exposed upper platform.
    for sign in (-1,1):
        x=sign*7; y=-6.6
        box(x,y,0,2.6,2.2,.12,'candy/lavender')
        prism(x,y,.12,.6,.04,'candy/gold',12)
        target=f'pad_{"left" if sign<0 else "right"}'
        entity('info_notnull',(sign*3.5,-2.5,10.8),targetname=target)
        trigger('trigger_push',(x-1.15,y-.95,.12),(x+1.15,y+.95,.7),target=target)
    # Eight authored spawn pockets behind local cover; two unobstructed exits each.
    for i,(x,y,yaw) in enumerate([(-26.5,22,315),(26.5,22,225),(-23,11.8,315),(23,11.8,225),(-26,-11,45),(26,-11,135),(-26.5,-22,45),(26.5,-22,135)]):
        entity('info_player_deathmatch',(x,y,.65),angle=str(yaw))
    # Twelve world-authored pickup points. F2 still controls active count / weapon chance.
    for x,y,z in [(0,16.1,2.5),(-14.8,16.3,2.5),(14.8,16.3,2.5),(-17,2.4,0),(17,2.4,0),(-7.4,9.5,3.9),(7.5,9.5,3.9),(0,-3,5.58),(-4.5,-8.2,0),(4.8,-8.2,0),(-10.7,-19.1,0),(10.6,-19.1,0)]:
        entity('item_health_medium',(x,y,z+.65))
    # Cable paths avoid every rising pavilion stair, with full capsule clearance.
    # The route is authored as feet positions; Bevy renders the cable +2.35m higher.
    for name,a,b in [('z1',(-12.1,15.5,2.5),(-19,-6.8,2.5)),('z2',(-15,-6.8,2.5),(17,-6.8,2.5))]:
        for end,pos,other in [('a',a,'b'),('b',b,'a')]:
            endpoint=f'{name}_{end}'
            entity('info_notnull',pos,targetname=endpoint)
            x,y,z=pos
            trigger('trigger_zipline',(x-1,y-1,z+.01),(x+1,y+1,z+2.0),start=endpoint,target=f'{name}_{other}')
            # Metal-orange mounts stand beside the traversed capsule path.
            # The thin cable and its collision are owned by shared client gameplay.
            side=-1 if x<0 else 1
            prism(x+side*1.18,y,z,.095,2.35/Z_SCALE,'candy/gold',8)
            box(x+side*.68,y,z+2.28/Z_SCALE,1.36,.12,.14/Z_SCALE,'candy/gold')
    # Soft fill gives shop interiors clear visibility; the bake supplies real world lighting.
    for x,y,z in [(-17,0,1.9),(17,0,1.9),(0,0,3.7),(0,16.8,1.5)]:
        entity('light',(x,y,z),light='30',_color='1 0.93 0.96')
    for x in (-17,17):
        for y in (-5,5):entity('light',(x,y,1.8),light='20',_color='1 .96 .98',_extradist='90')
    props={'classname':'worldspawn','message':'Candy Courtyard','_blocksize':'8192','_description':'A pastel confectionery courtyard with layered loops and two exposed zipline shortcuts.'}
    MAP.write_text('// Game: Boltrunner\n// Format: Quake3 (Valve)\n{\n'+'\n'.join(f'"{k}" "{v}"' for k,v in props.items())+'\n'+'\n'.join(BRUSHES)+'\n}\n'+'\n'.join(ENTITIES)+'\n')
    (SOURCE/'layout.json').write_text(json.dumps(dict(bounds=[78.4,67.2],xy_scale=XY_SCALE,z_scale=Z_SCALE,routes=['shops and plaza','mint outer circuit','gallery and pavilion'],shops=8,stairs=4,ramps=2,ziplines=2,jump_pads=2),indent=2)+'\n')
    print(f'Authored {len(BRUSHES)} world brushes, {len(ENTITIES)} entities → {MAP}')

if __name__=='__main__':
    materials(); author()
