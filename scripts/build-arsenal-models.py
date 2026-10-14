"""Generate the editable arsenal geometry and glTF assets from reusable parts."""
import json
import math
import struct
from pathlib import Path

OUT = Path(__file__).resolve().parents[1] / 'assets/weapons/arsenal/built'


def geometry(points, faces):
    positions, normals, uvs, indices = [], [], [], []
    for face in faces:
        a, b, c = [points[i] for i in face[:3]]
        u, v = [b[i] - a[i] for i in range(3)], [c[i] - a[i] for i in range(3)]
        n = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]
        length = math.sqrt(sum(x*x for x in n))
        n = [x/length for x in n]
        start = len(positions)
        axis = max(range(3), key=lambda i: abs(n[i]))
        axes = [(axis+1)%3, (axis+2)%3]
        low = [min(points[i][a] for i in face) for a in axes]
        span = [max(points[i][a] for i in face)-low[j] for j,a in enumerate(axes)]
        for i in face:
            positions.append(points[i]); normals.append(n)
            uvs.append([(points[i][a]-low[j])/max(span[j],.0001) for j,a in enumerate(axes)])
        for i in range(1, len(face)-1):
            indices.extend([start, start+i, start+i+1])
    return positions, normals, uvs, indices


def box(w, h, depth, bevel=0.012):
    b = min(bevel, w/5, h/5)
    ring = [(-w/2+b,-h/2), (w/2-b,-h/2), (w/2,-h/2+b), (w/2,h/2-b),
            (w/2-b,h/2), (-w/2+b,h/2), (-w/2,h/2-b), (-w/2,-h/2+b)]
    points = [(x,y,z) for z in [-depth/2,depth/2] for x,y in ring]
    faces = [list(reversed(range(8))), list(range(8,16))]
    faces += [[i,(i+1)%8,(i+1)%8+8,i+8] for i in range(8)]
    return geometry(points,faces)


def tube(radius, length, inner=0.0, segments=24):
    points=[]
    for z in [-length/2,length/2]:
        for r in [radius,inner]:
            points += [(r*math.cos(i*2*math.pi/segments),r*math.sin(i*2*math.pi/segments),z) for i in range(segments)]
    faces=[]
    n=segments
    for i in range(n):
        j=(i+1)%n
        faces += [[i,j,j+2*n,i+2*n], [i+n,i+3*n,j+3*n,j+n], [i,i+n,j+n,j], [i+2*n,j+2*n,j+3*n,i+3*n]]
    if inner==0.0:
        # Keep a tiny center ring to avoid degenerate cap triangles.
        return tube(radius,length,0.001,segments)
    return geometry(points,faces)


def pixels(cells, size=.018, depth=.028, occupied=None):
    # Emit only the surface of a single solid silhouette. Neighboring pixels
    # share exact edges: no internal faces, tiny gaps or overlapping shells.
    occupied=set(cells) if occupied is None else occupied
    p,n,uv,idx=[],[],[],[]
    low_x=min(x for x,y in occupied)-.5;low_y=min(y for x,y in occupied)-.5
    span_x=max(x for x,y in occupied)+.5-low_x;span_y=max(y for x,y in occupied)+.5-low_y
    for x,y in sorted(cells):
        points=[(a,b,z) for z in [-depth/2,depth/2]
                for a,b in [(-size/2,-size/2),(size/2,-size/2),(size/2,size/2),(-size/2,size/2)]]
        faces=[[3,2,1,0],[4,5,6,7]]
        for neighbor,face in [((x,y-1),[0,1,5,4]),((x+1,y),[1,2,6,5]),
                              ((x,y+1),[2,3,7,6]),((x-1,y),[3,0,4,7])]:
            if neighbor not in occupied:faces.append(face)
        shape=geometry(points,faces)
        offset=len(p)
        p += [(a+x*size,b+y*size,c) for a,b,c in shape[0]]
        n += shape[1]
        uv += [((a/size+x-low_x)/span_x,(b/size+y-low_y)/span_y)
               if abs(normal[2])>.5 else tex for (a,b,c),normal,tex in zip(shape[0],shape[1],shape[2])]
        idx += [i+offset for i in shape[3]]
    return p,n,uv,idx


class Model:
    def __init__(self,name,accent):
        self.name=name; self.blob=bytearray(); self.views=[]; self.accessors=[]; self.meshes=[]; self.nodes=[]
        def material(name,color,metal,rough,emissive=None):
            result={'name':name,'pbrMetallicRoughness':{'baseColorFactor':[*color,1], 'metallicFactor':metal,'roughnessFactor':rough}}
            if not emissive: result['pbrMetallicRoughness']['baseColorTexture']={'index':0}
            else: result['emissiveFactor']=emissive
            return result
        self.materials=[material('Worn coated steel',[.35,.4,.48],.65,.48),material('Rubber grip',[.12,.13,.15],.05,.9),
                        material('Brushed hardware',[1.7,1.7,1.7],.8,.36),material('Coated accent',accent,.45,.48),
                        material('Illuminated energy',accent,.1,.25,accent),material('Muzzle interior',[.06,.065,.07],.1,.95)]
        # glTF color factors are bounded; brighter metal uses the texture without tint.
        self.materials[2]['pbrMetallicRoughness']['baseColorFactor']=[.72,.75,.8,1]
        skin = {'flamethrower':2, 'shotgun':0, 'machine_gun':2, 'rocket_launcher':2,
                'laser':1, 'sniper':0, 'drone_controller':1, 'ricochet':0, 'drone':1,
                'karambit':0, 'arc_cannon':1, 'plasma_launcher':0, 'electro_plasma':1, 'seeker_plasma':2}[name]
        self.tiles = [skin,4,3,5 if skin!=1 else 1,skin,4]
        self.tile_columns=3;self.tile_rows=2;self.texture_name='weapon-surfaces.png'
        for index in [0,2,3]:
            value={0:.55,2:.32,3:.6}[index]
            self.materials[index]['pbrMetallicRoughness']['baseColorFactor']=[value,value,value,1]
        self.materials[2]['pbrMetallicRoughness'].update(metallicFactor=.55,roughnessFactor=.65)
        self.materials[1]['pbrMetallicRoughness']['baseColorFactor']=[.65,.65,.65,1]
        if skin==1:
            self.materials[0]['pbrMetallicRoughness'].update(metallicFactor=.15,roughnessFactor=.38)

    def accessor(self,values,components,kind,component_type=5126):
        while len(self.blob)%4:self.blob.append(0)
        offset=len(self.blob)
        flat=[x for row in values for x in row] if components>1 else values
        self.blob.extend(struct.pack('<'+('f' if component_type==5126 else 'I')*len(flat),*flat))
        view=len(self.views);self.views.append({'buffer':0,'byteOffset':offset,'byteLength':len(self.blob)-offset})
        index=len(self.accessors);a={'bufferView':view,'componentType':component_type,'count':len(values),'type':kind}
        if kind=='VEC3':a.update(min=[min(v[i] for v in values) for i in range(3)],max=[max(v[i] for v in values) for i in range(3)])
        self.accessors.append(a);return index

    def part(self,name,shape,position=(0,0,0),material=0,rotation=None):
        p,n,uv,idx=shape
        tile=self.tiles[material]
        uv=[[(tile%self.tile_columns+.015+u*.97)/self.tile_columns,
             (tile//self.tile_columns+.015+v*.97)/self.tile_rows] for u,v in uv]
        mesh=len(self.meshes)
        self.meshes.append({'name':name,'primitives':[{'attributes':{'POSITION':self.accessor(p,3,'VEC3'),'NORMAL':self.accessor(n,3,'VEC3'),'TEXCOORD_0':self.accessor(uv,2,'VEC2')},'indices':self.accessor(idx,1,'SCALAR',5125),'material':material}]})
        node={'name':name,'mesh':mesh,'translation':list(position)}
        if rotation:node['rotation']=rotation
        self.nodes.append(node)
        return len(self.nodes)-1

    def group(self,name,start):
        children=list(range(start,len(self.nodes)))
        self.nodes.append({'name':name,'children':children})

    def group_named(self, name, names):
        children=[i for i,n in enumerate(self.nodes) if n.get('name') in names]
        if children:self.nodes.append({'name':name,'children':children})

    def receiver(self,length=.42,width=.19,height=.18):
        self.part('Receiver',box(width,height,length),material=0)
        self.part('Grip',box(.09,.22,.12),(0,-.18,.12),1,[-math.sin(.12),0,0,math.cos(.12)])
        self.part('TriggerGuard',box(.1,.025,.15),(0,-.18,-.03),2)
        self.part('GuardFront',box(.1,.09,.02),(0,-.14,-.1),2)
        self.part('Trigger',box(.015,.05,.015),(0,-.13,-.015),1)
        self.part('TopRail',box(width*.55,.025,length*.85),(0,height/2+.015,0),2)
        for z in [-.13,0,.13]:
            self.part('RailNotch',box(width*.6,.02,.025),(0,height/2+.03,z),1)
        for x in [-width/2,width/2]:
            self.part('InsetPanel',box(.006,height*.6,length*.52),(x,0,0),3)
            for z in [-.065,-.025,.015,.055]:
                self.part('PanelVent',box(.009,.045,.009),(x*1.04,0,z),1)
            for z in [-length*.35,length*.35]:
                self.part('Fastener',tube(.012,.01,.004,12),(x,0,z),2,[0,math.sin(math.pi/4),0,math.cos(math.pi/4)])

    def barrel(self,z=-.35,r=.035,length=.28):
        self.part('Barrel',tube(r,length,r*.65),(0,.02,z),2)
        self.part('Muzzle',tube(r*1.25,.065,r*.68),(0,.02,z-length/2),1)
        self.nodes.append({'name':'Muzzle','translation':[0,.02,z-length/2-.033]})

    def save(self):
        children={child for node in self.nodes for child in node.get('children',[])}
        roots=[i for i in range(len(self.nodes)) if i not in children]
        if self.name!='drone':
            self.nodes.append({'name':'ViewmodelRoot','translation':[0,-.03,-.12],'scale':[.75,.75,.75],'children':roots})
            roots=[len(self.nodes)-1]
        document={'asset':{'version':'2.0','generator':'Boltrunner arsenal geometry'},'scene':0,'scenes':[{'nodes':roots}],
                  'nodes':self.nodes,'meshes':self.meshes,'materials':self.materials,'buffers':[{'uri':self.name+'.bin','byteLength':len(self.blob)}],
                  'bufferViews':self.views,'accessors':self.accessors,'images':[{'uri':self.texture_name}],
                  'textures':[{'source':0,'sampler':0}],'samplers':[{'wrapS':10497,'wrapT':10497,
                    'magFilter':9728 if self.name=='karambit' else 9729,
                    'minFilter':9728 if self.name=='karambit' else 9987}]}
        (OUT/(self.name+'.gltf')).write_text(json.dumps(document,separators=(',',':'))+'\n')
        (OUT/(self.name+'.bin')).write_bytes(self.blob)
        print(self.name,len(self.nodes),'parts',len(self.blob),'geometry bytes')


def build():
    OUT.mkdir(parents=True,exist_ok=True)
    m=Model('flamethrower',[1,.32,.07]);m.receiver(.45,.22,.19);m.barrel(-.37,.065,.3)
    for x in [-.1,.1]:m.part('FuelTank',tube(.065,.28,.003),(x,-.035,.23),3,[math.sin(math.pi/4),0,0,math.cos(math.pi/4)])
    for z in [-.26,-.33,-.4]:m.part('CoolingCollar',tube(.072,.025,.063),(0,.02,z),0)
    m.part('PilotLight',tube(.013,.06,.008),(0,-.045,-.5),4);m.save()

    m=Model('shotgun',[.75,.39,.12]);m.receiver(.43,.21,.19);m.barrel(-.46,.041,.48)
    m.part('MagazineTube',tube(.029,.42,.01),(0,-.045,-.42),0)
    # A visible underside loading port feeds forward into the magazine tube.
    m.part('LoadingPortWell',box(.062,.006,.114),(0,-.101,-.12),5)
    for x in [-.038,.038]:m.part('LoadingPortRim',box(.012,.012,.13),(x,-.105,-.12),2)
    for z in [-.184,-.056]:m.part('LoadingPortRim',box(.088,.012,.012),(0,-.105,z),2)
    m.part('LoadingGate',box(.054,.006,.102),(0,-.109,-.12),1)
    pump_start=len(m.nodes)
    m.part('Pump',box(.15,.11,.18),(0,-.055,-.35),1)
    for z in [-.39,-.35,-.31]:m.part('PumpRib',box(.16,.12,.013),(0,-.055,z),2)
    m.group('MovingPump',pump_start)
    m.part('Stock',box(.12,.18,.24),(0,-.03,.31),3)
    start=len(m.nodes)
    m.part('ShellCasing',tube(.019,.065),material=3)
    m.part('ShellBase',tube(.022,.012),(0,0,.034),2)
    m.part('ShellCrimp',tube(.018,.012),(0,0,-.034),1)
    m.group('ReloadShell',start);m.nodes[-1]['scale']=[0,0,0];m.save()

    m=Model('machine_gun',[.45,.55,.23]);m.receiver(.48,.24,.22);m.barrel(-.47,.045,.48)
    m.part('AmmoDrum',tube(.12,.16,.006),(.16,-.1,.08),0,[0,math.sin(math.pi/4),0,math.cos(math.pi/4)])
    m.part('BarrelShroud',tube(.06,.28,.052),(0,.02,-.38),0)
    for i in range(8):m.part('Vent',box(.025,.014,.07),(.057*math.cos(i*math.pi/4),.02+.057*math.sin(i*math.pi/4),-.38),5)
    m.part('MovingBolt',box(.04,.045,.09),(.13,.055,.04),2)
    m.part('HeatStrip',box(.02,.025,.23),(.13,0,-.11),4)
    for side,name in [(-1,'CoolingVentLeft'),(1,'CoolingVentRight')]:
        m.part(name,box(.028,.08,.2),(side*.07,.04,-.37),3)
    m.save()

    m=Model('rocket_launcher',[.6,.28,.12]);m.receiver(.3,.17,.13)
    m.part('LauncherTube',tube(.095,.82,.075),(0,.085,-.07),0)
    m.part('FrontCollar',tube(.115,.07,.076),(0,.085,-.45),3)
    m.part('RearCollar',tube(.13,.09,.076),(0,.085,.34),1)
    m.part('ShoulderRest',box(.15,.14,.19),(0,-.09,.27),1)
    m.part('Sight',box(.08,.08,.15),(0,.205,-.05),3)
    m.nodes.append({'name':'Muzzle','translation':[0,.085,-.5]})
    start=len(m.nodes)
    m.part('ReloadRocketBody',tube(.048,.22),material=3)
    m.part('ReloadRocketNose',geometry([(0,0,-.2),(-.048,-.048,-.1),(.048,-.048,-.1),(.048,.048,-.1),(-.048,.048,-.1)],[[0,2,1],[0,3,2],[0,4,3],[0,1,4],[1,2,3,4]]),material=2)
    for x in [-.052,.052]:m.part('ReloadRocketFin',box(.025,.07,.07),(x,0,.10),0)
    m.group('ReloadRocket',start);m.nodes[-1]['scale']=[0,0,0];m.save()

    m=Model('laser',[.49,.27,.96]);m.receiver(.44,.21,.2)
    for x in [-.105,.105]:m.part('EmitterRail',box(.047,.08,.45),(x,.03,-.36),2)
    m.part('ChargeCore',tube(.055,.3,.035),(0,.025,-.31),4)
    for z in [-.23,-.31,-.39]:m.part('Inductor',tube(.077,.025,.052),(0,.025,z),3)
    m.part('Lens',tube(.083,.05,.059),(0,.025,-.52),1)
    m.group_named('ReloadElectroCell',{'CellMagazine','ChargeIndicator'})
    m.nodes.append({'name':'Muzzle','translation':[0,.025,-.55]});m.save()

    m=Model('sniper',[.12,.58,.5]);m.receiver(.49,.16,.17);m.barrel(-.56,.024,.72)
    m.part('Scope',tube(.045,.28,.025),(0,.16,-.025),1)
    for z in [-.16,.11]:m.part('ScopeLens',tube(.052,.025,.032),(0,.16,z),3)
    m.part('MovingBolt',box(.07,.05,.075),(.09,.025,.13),2)
    m.part('Stock',box(.12,.15,.24),(0,-.035,.34),3)
    m.part('RifleMagazine',box(.085,.13,.14),(0,-.12,-.01),0)
    m.part('MagazineFloor',box(.095,.018,.15),(0,-.19,-.01),3)
    m.group_named('ReloadRifleMagazine',{'RifleMagazine','MagazineFloor'})
    for x in [-.055,.055]:m.part('FoldedBipod',box(.017,.02,.24),(x,-.07,-.37),2)
    m.save()

    m=Model('drone_controller',[.15,.55,.92])
    m.part('Controller',box(.31,.19,.075),(0,0,0),0)
    m.part('DisplayFrame',box(.17,.105,.015),(0,.015,.046),1)
    m.part('Display',box(.145,.085,.008),(0,.015,.057),4)
    for y in [-.01,.015,.04]:m.part('ScreenReadout',box(.085,.004,.002),(0,y,.063),2)
    for x in [-.12,.12]:
        m.part('Joystick',tube(.024,.026,.003),(x,-.035,.055),2)
        m.part('Antenna',tube(.006,.15,.002),(x,.155,0),1,[math.sin(math.pi/4),0,0,math.cos(math.pi/4)])
    m.nodes.append({'name':'Muzzle','translation':[0,.08,-.07]});m.save()

    m=Model('ricochet',[.16,.75,.28]);m.receiver(.39,.23,.19);m.barrel(-.38,.087,.28)
    for z in [-.25,-.33,-.41]:m.part('AcceleratorCoil',tube(.105,.037,.083),(0,.02,z),3)
    m.part('BallChamber',tube(.09,.17,.06),(0,-.065,.17),4)
    m.part('MovingBolt',box(.055,.05,.07),(.12,.05,0),2);m.save()

    m=Model('karambit',[.15,.9,.8])
    m.texture_name='karambit-pixels.png';m.tile_columns=2;m.tile_rows=2
    m.tiles=[0,2,1,3,3,0]
    for index in range(6):
        m.materials[index]['pbrMetallicRoughness'].update(baseColorTexture={'index':0},
            baseColorFactor=[.94,.94,.94,1],metallicFactor=.25,roughnessFactor=.5)
    m.materials[4]['emissiveFactor']=[.02,.05,.04]
    # Compact hawkbill: a 120-degree claw with a tapering inward hook.
    # Keep its blade in the same horizontal plane as the palm and ring.
    blade=set()
    for x in range(-4,17):
        for y in range(0,14):
            dx,dy=x-8,y-1
            angle=math.atan2(dy,dx)
            radius=math.hypot(dx,dy)
            if math.radians(60)<=angle<=math.radians(188):
                taper=max(.6,min(3.5,(angle-math.radians(60))*3.5))
                if 10.5-taper<=radius<=10.5: blade.add((x,y))
    blade.update((x,y) for x in range(-2,2) for y in range(0,3))
    blade.add((12,10))  # Join the tapered tip to the arc with a shared edge.
    spine={(x,y) for x,y in blade if math.hypot(x-8,y-1)>9.4 and y<10}
    rotation=[-math.sqrt(.5),0,0,math.sqrt(.5)]
    m.part('BladeSteel',pixels(blade-spine,.016,.020,blade),(0,-.12,-.075),2,rotation)
    m.part('BladeSpine',pixels(spine,.016,.020,blade),(0,-.12,-.075),0,rotation)
    m.part('Guard',box(.12,.09,.025,.004),(0,-.12,-.065),3)
    m.part('Grip',box(.07,.065,.17,.003),(0,-.12,.04),1)
    for z in [-.025,.01,.045,.08]:
        m.part('GripPixel',box(.075,.07,.012,.001),(0,-.12,z),0)
    ring={(x,y) for x in range(-3,4) for y in range(-3,4) if 2<=math.hypot(x,y)<=3.5}
    m.part('FingerRing',pixels(ring,.012,.025),(0,-.12,.17),3,rotation)
    m.group('Knife',0)
    m.nodes.append({'name':'Muzzle','translation':[.12,-.12,-.23]});m.save()

    m=Model('electro_plasma',[.10,.7,1.0]);m.receiver(.44,.24,.24)
    m.part('InductorHousing',box(.30,.19,.42),(0,.02,-.24),0)
    m.part('CeramicBarrel',tube(.08,.36,.055),(0,.025,-.30),2)
    for z in [-.16,-.23,-.30,-.37]:
        m.part('LiveCoil',tube(.092,.025,.08),(0,.025,z),4)
        for x in [-.13,.13]:m.part('CoolingVent',box(.03,.12,.025),(x,.035,z),1)
    for x in [-.12,.12]:
        m.part('ArmoredRail',box(.05,.12,.32),(x,-.04,-.28),3)
        m.part('EnergyConduit',box(.008,.025,.28),(x*1.24,.03,-.27),4)
    m.part('TopSight',box(.12,.035,.18),(0,.16,-.08),2)
    m.part('CellMagazine',box(.13,.19,.14),(.13,-.13,.08),0)
    for y in [-.18,-.13,-.08]:m.part('ChargeIndicator',box(.009,.018,.085),(.201,y,.08),4)
    m.part('MuzzleShroud',tube(.1,.055,.068),(0,.025,-.515),0)
    m.group_named('ReloadElectroCell',{'CellMagazine','ChargeIndicator'})
    m.nodes.append({'name':'Muzzle','translation':[0,.025,-.55]});m.save()

    m=Model('seeker_plasma',[.50,1.0,.3]);m.receiver(.34,.22,.26)
    m.part('PressureSleeve',tube(.092,.36,.052),(0,.025,-.26),2)
    for z in [-.16,-.25,-.34]:m.part('ContainmentCollar',tube(.108,.045,.09),(0,.025,z),0)
    for x in [-.105,.105]:
        m.part('CellBank',box(.065,.10,.30),(x,.015,-.25),3)
        for z in [-.17,-.24,-.31]:m.part('PulseWindow',box(.008,.065,.043),(x*1.35,.015,z),4)
    for side in [-1,1]:
        children=[i for i,n in enumerate(m.nodes) if n.get('name') in {'CellBank','PulseWindow'} and n['translation'][0]*side>0]
        m.nodes.append({'name':'ReloadSeekerLeft' if side<0 else 'ReloadSeekerRight','children':children})
    m.part('RangingSight',box(.09,.06,.16),(0,.155,-.11),1)
    m.part('SightLens',box(.05,.035,.006),(0,.155,-.192),4)
    m.part('RearCapacitor',box(.19,.14,.12),(0,.015,.16),0)
    m.part('Muzzle',tube(.12,.065,.07),(0,.025,-.46),2)
    m.nodes.append({'name':'Muzzle','translation':[0,.025,-.5]});m.save()

    m=Model('arc_cannon',[.12,.65,1]);m.receiver(.38,.23,.21)
    for x in [-.12,.12]:
        m.part('EmitterFork',box(.05,.07,.38),(x,.05,-.32),2)
        for z in [-.21,-.29,-.37]:
            m.part('CopperInductor',tube(.044,.028,.033),(x,.05,z),3)
        m.part('LiveTip',tube(.036,.055,.023),(x,.05,-.51),4)
    m.part('ChargeCore',tube(.065,.22,.035),(0,.045,-.23),4)
    m.part('Capacitor',tube(.065,.15,.012),(0,-.095,.16),3)
    m.part('CoilBridge',box(.29,.025,.04),(0,.09,-.17),2)
    m.nodes.append({'name':'Muzzle','translation':[0,.05,-.54]});m.save()

    m=Model('plasma_launcher',[1,.18,.7]);m.receiver(.32,.23,.22);m.barrel(-.34,.075,.32)
    drum_start=len(m.nodes)
    # Mount a contained cartridge bank beside the receiver. Solid end caps
    # hide the exposed ends; braces hold the cells clear of the gun body.
    m.part('DrumSpine',tube(.035,.23,.01),(.235,-.065,.055),2)
    for z in [-.065,.175]:
        m.part('DrumCap',tube(.11,.035,.006),(.235,-.065,z),0)
    for i in range(4):
        angle=i*math.pi/2+math.pi/4
        x=.235+.07*math.cos(angle);y=-.065+.07*math.sin(angle)
        m.part('PlasmaCell',tube(.022,.19,.008),(x,y,.055),4)
        for z in [-.035,.145]:m.part('CellSleeve',tube(.03,.03,.023),(x,y,z),2)
    for z in [-.05,.15]:m.part('DrumBrace',box(.06,.05,.025),(.12,-.065,z),2)
    m.part('ChargeReadout',box(.04,.08,.004),(.235,-.065,.195),4)
    m.group_named('ReloadPlasmaDrum',{'DrumSpine','DrumCap','PlasmaCell','CellSleeve','ChargeReadout'})
    m.part('MovingBolt',box(.055,.045,.09),(.135,.045,.015),2)
    for z in [-.25,-.34,-.43]:
        m.part('Collar',tube(.086,.025,.07),(0,.02,z),3)
    m.save()

    m=Model('drone',[.15,.55,.92]);m.part('Hull',box(.26,.12,.3),material=0)
    m.part('Camera',tube(.05,.055,.035),(0,-.02,-.18),4)
    for x in [-.22,.22]:
        for z in [-.2,.2]:
            m.part('Arm',box(.25,.025,.045),(x/2,0,z),2)
            m.part('Motor',tube(.032,.06,.006),(x,.02,z),3,[math.sin(math.pi/4),0,0,math.cos(math.pi/4)])
            m.part('Rotor',box(.25,.008,.025),(x,.065,z),1)
    m.save()


if __name__=='__main__':build()
