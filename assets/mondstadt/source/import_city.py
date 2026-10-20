#!/usr/bin/env python3
"""Offline, reproducible port of Mondstadt's static city geometry and atlas."""
from pathlib import Path
import hashlib,json,math,struct,sys
import numpy as np
from PIL import Image
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'scripts'))
from map_bsp import index_vertices
HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE))
BUILT=HERE.parent/'built'
ROTATE=np.diag([-1.,1.,-1.])
GROUND=53.1

def obj(path):
    v=[];n=[];uv=[];faces=[]
    for line in path.read_text().splitlines():
        a=line.split()
        if not a:continue
        if a[0]=='v':v.append(list(map(float,a[1:4])))
        elif a[0]=='vn':n.append(list(map(float,a[1:4])))
        elif a[0]=='vt':uv.append(list(map(float,a[1:3])))
        elif a[0]=='f':
            polygon=[[int(i)-1 for i in corner.split('/')] for corner in a[1:]]
            for j in range(1,len(polygon)-1):faces.append([polygon[0],polygon[j],polygon[j+1]])
    return [np.asarray(a) for a in (v,n,uv,faces)]

def world(p):
    p=np.asarray(p,dtype=float).copy();p[...,1]+=GROUND
    return p@ROTATE

def hull(points):
    points=sorted(set(map(tuple,np.round(points,4))))
    def cross(o,a,b):return (a[0]-o[0])*(b[1]-o[1])-(a[1]-o[1])*(b[0]-o[0])
    chains=[]
    for ps in (points,points[::-1]):
        chain=[]
        for p in ps:
            while len(chain)>1 and cross(chain[-2],chain[-1],p)<=0:chain.pop()
            chain.append(p)
        chains.append(chain[:-1])
    return np.array(chains[0]+chains[1])

def main():
    BUILT.mkdir(parents=True,exist_ok=True);textures=BUILT/'textures';textures.mkdir(exist_ok=True)
    v,n,uv,f=obj(HERE/'exact/city.obj');a=v[f[:,:,0]];normal=n[f[:,:,2]]
    geometric=np.cross(a[:,1]-a[:,0],a[:,2]-a[:,0]);valid=np.linalg.norm(geometric,axis=1)>1e-7
    assert np.min(np.sum(geometric[valid]*normal[valid].mean(1),axis=1))>0,'OBJ outward winding'
    a=a[valid];normal=normal[valid];f=f[valid]
    # The distant city mesh contains decorative paving/garden overlays up to
    # two metres above its continuous public floor. They must remain decals,
    # not floating platforms with invisible, impassable edges.
    plaza=json.loads((HERE/'plaza.json').read_text())
    for correction in plaza['ground_corrections']:
        ids=correction['source_faces']
        a[ids,:,1]=correction['height_raw']
        normal[ids]=[0.,1.,0.]
    for correction in plaza['support_top_corrections']:
        ids=correction['source_faces']
        a[ids,:,1]=np.minimum(a[ids,:,1],correction['ceiling_raw'])
    # The HLOD contains many independent decorative sheets above the actual
    # street. Fit every sheet to its measured supporting faces, not to a
    # constant city-wide elevation; this preserves district slopes/terraces.
    layers=json.loads((HERE/'ground_layers.json').read_text())
    corners={}
    for correction in layers['corrections']:
        face=correction['source_face']
        for vertex,height in zip(a[face],correction['heights_raw']):
            corners.setdefault(tuple(np.round(vertex,4)),[]).append(height)
    corners={key:float(np.median(heights)) for key,heights in corners.items()}
    for face in range(len(a)):
        changed=False
        for corner,vertex in enumerate(a[face]):
            height=corners.get(tuple(np.round(vertex,4)))
            if height is not None:
                a[face,corner,1]=height;changed=True
        if changed:
            axis=np.cross(a[face,1]-a[face,0],a[face,2]-a[face,0])
            length=np.linalg.norm(axis)
            if length>1e-8:normal[face]=axis/length
    floor=a[(geometric[valid,1]/np.maximum(np.linalg.norm(geometric[valid],axis=1),1e-9))>.65]
    def floor_at(x,z,ceiling=1e9):
        q=floor[:,:,[0,2]];A=q[:,1]-q[:,0];B=q[:,2]-q[:,0];C=np.array([x,z])-q[:,0];den=A[:,0]*B[:,1]-A[:,1]*B[:,0]
        u=(C[:,0]*B[:,1]-C[:,1]*B[:,0])/den;w=(A[:,0]*C[:,1]-A[:,1]*C[:,0])/den
        h=floor[:,0,1]+u*(floor[:,1,1]-floor[:,0,1])+w*(floor[:,2,1]-floor[:,0,1]);ok=(u>=-1e-6)&(w>=-1e-6)&(u+w<=1+1e-6)&(h<ceiling)
        if not np.any(ok):raise ValueError(f'No walkable floor at {x,z}')
        return float(h[ok].max())
    stairs=json.loads((HERE/'navigation.json').read_text())['stairs']
    meshes=[[],[],[]]
    from restore_stone import prepare as prepare_stone
    restored_materials,restore_stone=prepare_stone(textures,len(meshes))
    meshes.extend([] for _ in restored_materials)
    sun=np.array([-.35,.85,-.35]);sun/=np.linalg.norm(sun)
    light=.195+.035*np.maximum(normal@sun,0)
    vertices=np.concatenate((world(a),normal@ROTATE,uv[f[:,:,1]]*np.array([1,-1])+np.array([0,1]),np.zeros((*a.shape[:2],2)),np.repeat(light[...,None],3,axis=2),np.ones((*a.shape[:2],1))),axis=2)
    def clip(poly,axis,distance,inside):
        output=[]
        for previous,current in zip(poly[-1:]+poly[:-1],poly):
            p=float(previous[:3]@axis-distance);c=float(current[:3]@axis-distance)
            pin=p<=1e-7 if inside else p>=-1e-7
            cin=c<=1e-7 if inside else c>=-1e-7
            if pin!=cin:output.append(previous+(current-previous)*(p/(p-c)))
            if cin:output.append(current)
        return output
    volumes=[]
    for stair in stairs:
        bottom=np.array(stair['bottom']);top=np.array(stair['top']);center=(bottom+top)/2
        direction=top-bottom;direction[1]=0;length=np.linalg.norm(direction);direction/=length
        right=np.array([direction[2],0,-direction[0]])
        volumes.append([(right,float(center@right)+stair['width']/2+.015),(-right,float(-center@right)+stair['width']/2+.015),(direction,float(top@direction)+.015),(-direction,float(-bottom@direction)+.015),(np.array([0.,1.,0.]),top[1]+2.6),(np.array([0.,-1.,0.]),-bottom[1]+1.1)])
    deleted_faces={f for p in json.loads((HERE/'placements.json').read_text())['buildings'] for f in p.get('replace_faces',[])}
    cut=[];render_flags=[];source_ids=[];public_flags=[]
    for source_id,data in enumerate(vertices):
        uvmean=data[:,6:8].mean(0)
        # Only source paving/grass/retaining walls. Historical building facades,
        # roofs and architectural details are never removed to make a route.
        generic=uvmean[0]<.5 and .125<uvmean[1]<.625
        polygons=[list(data)]
        if generic:
            for planes in volumes:
                pieces=[]
                for polygon in polygons:
                    remaining=polygon
                    for axis,distance in planes:
                        if not remaining:break
                        outside=clip(remaining,axis,distance,False)
                        if len(outside)>=3:pieces.append(outside)
                        remaining=clip(remaining,axis,distance,True)
                polygons=pieces
        for polygon in polygons:
            for k in range(1,len(polygon)-1):
                triangle=np.array([polygon[0],polygon[k],polygon[k+1]])
                if np.linalg.norm(np.cross(triangle[1,:3]-triangle[0,:3],triangle[2,:3]-triangle[0,:3]))>1e-7:
                    cut.append(triangle);render_flags.append(source_id not in deleted_faces);source_ids.append(source_id);public_flags.append(generic)
    vertices=np.array(cut)
    raw=vertices[:,:,:3]@ROTATE;raw[:,:,1]-=GROUND
    collision=list(raw)

    atlas=Image.open(HERE/'exact/city-atlas.png')
    city_uv=vertices[:,:,6:8]
    tile=np.floor(city_uv.mean(1)*8).astype(int)
    same_tile=np.all(np.floor(city_uv*8).astype(int)==tile[:,None,:],axis=(1,2))
    tile_materials={}
    repeating_tiles={(0,1),(1,1),(2,1),(2,2),(3,2),(0,3),(1,3),(2,3),(3,3),(0,4),(1,4),(2,4),(3,4)}
    for i,data in enumerate(vertices):
        if not render_flags[i]:continue
        restored=restore_stone(source_ids[i],data)
        if restored is not None:
            material,data=restored;meshes[material].extend(data);continue
        x,y=tile[i]
        if same_tile[i] and (x,y) in repeating_tiles:
            key=int(x),int(y)
            if key not in tile_materials:
                tile_materials[key]=len(meshes);meshes.append([])
            # Preserve the source's stone/grass/plaza types while giving large
            # HLOD street faces human-scale repeats instead of one blurry texel.
            axis=int(np.argmax(np.abs(data[:,3:6].mean(0))))
            axes=[j for j in range(3) if j!=axis]
            period=6.0 if (x,y)==(3,2) else 2.0 if y==4 or (x,y)==(3,3) else 4.0
            data=data.copy();data[:,6:8]=data[:,axes]/period
            meshes[tile_materials[key]].extend(data)
        else:meshes[0].extend(data)
    # Only the original HLOD prefix may be replaced by detailed buildings.
    # Authored stairs, bridge and shoreline foundations must remain closed.
    source_vertex_counts=[len(mesh) for mesh in meshes]
    bridge_start=np.array([50.7,-52.9,-161.365]);forward=np.array([4.,0,-7.3]);forward/=np.linalg.norm(forward);right=np.array([forward[2],0,-forward[0]])
    def tri(points,material=0,tex=None,solid=True,color=.21,shore=False):
        if shore:
            polygon=[np.concatenate((p,t)) for p,t in zip(points,tex)]
            pieces=[];remaining=polygon
            for axis,distance in [(right,float(bridge_start@right)+7),(-right,float(-bridge_start@right)+7),(forward,float((bridge_start+forward*87)@forward)),(-forward,float(-bridge_start@forward)+25)]:
                if not remaining:break
                outside=clip(remaining,axis,distance,False)
                if len(outside)>=3:pieces.append(outside)
                remaining=clip(remaining,axis,distance,True)
            for polygon in pieces:
                for k in range(1,len(polygon)-1):
                    vertices=np.array([polygon[0],polygon[k],polygon[k+1]])
                    tri(vertices[:,:3],material,vertices[:,3:5],solid,color)
            return
        points=np.array(points,float);normal=np.cross(points[1]-points[0],points[2]-points[0]);length=np.linalg.norm(normal)
        if length<1e-7:return
        normal/=length
        if solid:collision.append(points)
        if material is None:return
        if tex is None:tex=np.array([[.01,.54],[.11,.54],[.11,.64]])
        for p,t in zip(world(points),tex):meshes[material].append([*p,*(normal@ROTATE),*t,0,0,color,color,color,1])
    def quad(points,material=0,tex=None,solid=True,color=.21,shore=False):
        if tex is None:tex=np.array([[.005,.51],[.245,.51],[.245,.635],[.005,.635]])
        tri(np.array(points)[[0,1,2]],material,np.array(tex)[[0,1,2]],solid,color,shore);tri(np.array(points)[[0,2,3]],material,np.array(tex)[[0,2,3]],solid,color,shore)
    def box(center,size,angle=0,material=0):
        pts=np.array([[x,y,z] for x in (-.5,.5) for y in (-.5,.5) for z in (-.5,.5)])*size
        c,s=math.cos(angle),math.sin(angle);r=np.array([[c,0,s],[0,1,0],[-s,0,c]])
        pts=pts@r.T+center
        for ids in ([0,1,3,2],[4,6,7,5],[0,4,5,1],[2,3,7,6],[0,2,6,4],[1,5,7,3]):
            face=pts[ids];tex=None
            if material is not None and material>=3:
                normal=np.cross(face[1]-face[0],face[2]-face[0]);axes=[i for i in range(3) if i!=int(np.argmax(np.abs(normal)))];tex=face[:,axes]/2
            quad(face,material,tex)
    masonry=tile_materials[(2,2)]
    from ground_foundations import prepare as prepare_foundations
    foundation_count=prepare_foundations(a,normal,uv[f[:,:,1]]*[1,-1]+[0,1],volumes,quad,masonry,world,layers['public_floor_faces'])
    # The source HLOD ends at the island's walls; the lake and approaches are
    # authored here, retaining the city at its real metre scale.
    outline=hull(v[:,[0,2]]);centroid=outline.mean(0);d=outline-centroid;d/=np.linalg.norm(d,axis=1)[:,None]
    inner=outline+d*1.5;outer=outline+d*15
    floorcenters=floor.mean(1)
    heights=[]
    for point in inner:
        nearest=np.argsort(np.linalg.norm(floorcenters[:,[0,2]]-point,axis=1))[:16]
        heights.append(float(floorcenters[nearest,1].min())-2)
    for i in range(len(inner)):
        j=(i+1)%len(inner)
        q=[[inner[i,0],heights[i],inner[i,1]],[outer[i,0],-57.8,outer[i,1]],[outer[j,0],-57.8,outer[j,1]],[inner[j,0],heights[j],inner[j,1]]]
        quad(q,1,[[0,0],[0,3],[3,3],[3,0]],color=.215,shore=True)
        quad([[outer[i,0],-57.8,outer[i,1]],[outer[i,0],-75,outer[i,1]],[outer[j,0],-75,outer[j,1]],[outer[j,0],-57.8,outer[j,1]]],masonry,[[0,0],[0,5],[5,5],[5,0]])
    quad([[-260,-59.1,-350],[-260,-59.1,300],[260,-59.1,300],[260,-59.1,-350]],2,[[0,0],[0,12],[12,12],[12,0]],False,color=.22)
    # Main entrance/bridge matches the direction of the source gate's two towers.
    bridge_start=np.array([50.7,-52.9,-161.365]);forward=np.array([4.,0,-7.3]);forward/=np.linalg.norm(forward);right=np.array([forward[2],0,-forward[0]])
    bridge_length=85.;angle=math.atan2(forward[0],forward[2]);bridge_center=bridge_start+forward*(bridge_length/2)
    box(bridge_center+[0,-1.,0],[10.,2.,bridge_length],angle,masonry)
    for side in (-1,1):
        box(bridge_center+right*side*5+[0,.48,0],[.55,1.05,bridge_length],angle,masonry)
        for length in np.arange(4,bridge_length,8):box(bridge_start+forward*length+right*side*5+[0,.6,0],[.9,1.4,.9],angle,masonry)
    for length in (18,43,68):
        # Paired piers leave open water arches below the deck.
        for side in (-1,1):box(bridge_start+forward*length+right*side*3.6+[0,-4,0],[1.7,6,4],angle,masonry)
    end=bridge_start+forward*bridge_length
    box(end+forward*12+[0,-1.5,0],[24,3,24],angle,1)
    stairs=json.loads((HERE/'navigation.json').read_text())['stairs']
    for stair in stairs:
        bottom=np.array(stair['bottom'],float);top=np.array(stair['top'],float)
        # These locations replace the tall ledges of the distant HLOD with
        # faithful, walkable stone steps along existing public street approaches.
        rise=top[1]-bottom[1];steps=math.ceil(rise/.22)
        direction=top-bottom;direction[1]=0;run=np.linalg.norm(direction);direction/=run
        rbottom=bottom@ROTATE;rbottom[1]-=GROUND
        rtop=top@ROTATE;rtop[1]-=GROUND
        rdir=direction@ROTATE;angle=math.atan2(rdir[0],rdir[2])
        for i in range(steps):
            tread=run/steps;upper=rbottom[1]+rise*(i+1)/steps
            center=rbottom+rdir*tread*(i+.5);center[1]=(rbottom[1]-1+upper)/2
            box(center,[stair['width'],upper-rbottom[1]+1,tread+.006],angle,tile_materials.get((0,4),0))
    # Short, solid entrance treads connect the actual prefab door thresholds
    # to the measured street, rather than leaving a knee-high floating sill.
    from hd_city import rotation as building_rotation
    entrance_staircases=0
    for placement in json.loads((HERE/'placements.json').read_text())['buildings']:
        entry=placement.get('entrance_steps')
        if entry is None:continue
        R=building_rotation(placement['yaw'])
        top=np.array(entry['threshold_local'])@R.T*placement['scale']+placement['position']
        top[1]+=.012
        outward=np.array(entry['outward_local'])@R.T
        bottom=top+outward*entry['run'];bottom[1]=entry['street_world_height']-GROUND
        rise=top[1]-bottom[1];steps=math.ceil(rise/entry['max_rise'])
        direction=-outward;angle=math.atan2(direction[0],direction[2]);tread=entry['run']/steps
        for i in range(steps):
            upper=bottom[1]+rise*(i+1)/steps
            center=bottom+direction*tread*(i+.5);center[1]=(bottom[1]-.12+upper)/2
            box(center,[entry['width'],upper-bottom[1]+.12,tread+.012],angle,masonry)
        entrance_staircases+=1
    # Solid perimeter and lid are exclusively collision. Water has a sealed bed.
    for center,size in [([0,-80,0],[560,2,760]),([-275,40,0],[2,240,760]),([275,40,0],[2,240,760]),([0,40,-375],[560,240,2]),([0,40,375],[560,240,2]),([0,160,0],[560,2,760])]:box(np.array(center,float),size,material=None)
    # Carefully selected streets/plazas; their precise height comes from source.
    coordinates=[(40,-140,-49),(20,-120,-44),(0,-100,-44),(-40,-100,-49),(-60,-60,-37),(20,-60,-43),(58,-22,-31),(-20,-20,-31),(0,0,-25),(-40,20,-11),(-80,60,-11),(-40,60,-11),(-100,100,-2),(-100,85,-2),(80,-100,-50),(60,-120,-49)]
    spawns=[]
    for x,z,ceiling in coordinates:
        y=floor_at(x,z,ceiling);spawns.append({'position':world([x,y+.12,z]).tolist(),'yaw':0.})
    spawns.append({'position':world(bridge_start+forward*65+[0,.12,0]).tolist(),'yaw':math.atan2(-forward[0],-forward[2])})
    pickups=[s['position'] for s in spawns]
    from hd_city import prepare
    hd_chunks,hd_materials,windmills,windmill_mesh,replaces,hd_collision,hd_foundation_edges=prepare(world,ROTATE,sun,textures,len(meshes),a,raw[np.asarray(public_flags)].mean(1))
    from plaza_props import prepare as prepare_plaza
    plaza_chunks,plaza_materials,plaza_collision=prepare_plaza(world,ROTATE,sun,textures,len(meshes)+len(hd_materials))
    hd_chunks.extend(plaza_chunks);hd_materials.extend(plaza_materials);hd_collision.extend(plaza_collision)
    collision=[t for i,t in enumerate(raw) if render_flags[i] and not replaces(vertices[i],physics=True)]+collision[len(raw):]+hd_collision
    from ground_foundations import prepare_buildings
    building_foundations=prepare_buildings(hd_foundation_edges,a,layers['public_floor_faces'],collision,quad,masonry)
    for i,data in enumerate(meshes):
        source_count=source_vertex_counts[i]
        triangles=np.asarray(data[:source_count]).reshape(-1,3,14)
        # These materials were assigned from the original generic atlas tiles,
        # before their UVs became world-space repeats. They identify public
        # paving and retaining walls, which are not parts of the replaced house.
        retained=data[:source_count] if i in tile_materials.values() else np.asarray([t for t in triangles if not replaces(t)]).reshape(-1,14).tolist()
        meshes[i]=retained+data[source_count:]
    # Exact OBJ data retains real walls/roofs/facades/streets/statue/church topology.
    with (BUILT/'render.bin').open('wb') as out:
        out.write(struct.pack('<I',len(meshes)+len(hd_chunks)))
        for material,data in enumerate(meshes):
            unique,indices=index_vertices(data);out.write(struct.pack('<IiII',material,-1,len(unique),len(indices)));out.write(unique.tobytes());out.write(indices.tobytes())
        for material,data in hd_chunks:
            unique,indices=index_vertices(data);out.write(struct.pack('<IiII',material,-1,len(unique),len(indices)));out.write(unique.tobytes());out.write(indices.tobytes())
    col=np.asarray(collision);unique,inverse=np.unique(np.round(world(col).reshape(-1,3),5),axis=0,return_inverse=True);indices=inverse.reshape(-1,3)
    with (BUILT/'collision.bin').open('wb') as out:out.write(struct.pack('<II',len(unique),len(indices)));out.write(unique.astype('<f4').tobytes());out.write(indices.astype('<u4').tobytes())
    Image.open(HERE/'exact/city-atlas.png').save(textures/'city-atlas.png')
    Image.open(HERE/'exact/sky.png').save(textures/'sky.png')
    for name,colors in [('shore',[(69,116,53),(84,133,63)]),('lake',[(56,124,151),(70,148,169)])]:
        yy,xx=np.mgrid[:128,:128];blend=(np.sin(xx*.22+np.sin(yy*.07))*np.cos(yy*.17)+1)/2;pixels=np.array(colors[0])+blend[...,None]*(np.array(colors[1])-colors[0]);Image.fromarray(pixels.astype('uint8')).save(textures/f'{name}.png')
    materials=[dict(id=i,name=name,texture=filename,glow=None,width=size,height=size,emissive=False,alpha=False,blend=False) for i,(name,filename,size) in enumerate([('mondstadt/city','city-atlas.png',1024),('mondstadt/shore','shore.png',128),('mondstadt/lake','lake.png',128)])]
    hd_textures=HERE/'hd/textures'
    ground_texture=next(hd_textures.glob('*-Area_Mdcity_Ground15_Diffuse.png'))
    wall_texture=next(hd_textures.glob('*-Area_Mdbuild_Wall06_Diffuse.png'))
    stage_texture=next(hd_textures.glob('*-Area_Mdbuild_Stage01_Diffuse.png'))
    warm_texture=next(hd_textures.glob('*-Area_MdBuild_Wall09_Diffuse.png'))
    for (x,y),material in tile_materials.items():
        filename=f'city-detail-{x}-{y}.png'
        source=HERE/'exact/grass-meadow-v2.png' if (x,y)==(3,2) else ground_texture if y==4 or (x,y)==(3,3) else stage_texture if (x,y)==(0,3) else warm_texture if (x,y)==(1,1) else wall_texture
        image=Image.open(source).convert('RGB')
        if (x,y) in ((1,4),(3,4)):
            # The original plaza's warm stone region retains its separate color.
            pixels=np.asarray(image,dtype=float)*[1.12,1.025,.86]
            image=Image.fromarray(np.clip(pixels,0,255).astype('uint8'))
        image.save(textures/filename)
        materials.append(dict(id=material,name=f'mondstadt/detail/{x}/{y}',texture=filename,glow=None,width=image.width,height=image.height,emissive=False,alpha=False,blend=False))
    materials.extend(restored_materials)
    materials.extend(hd_materials)
    materials.sort(key=lambda material:material['id'])
    keep={'city-atlas.png','shore.png','lake.png','sky.png',*(m['texture'] for m in materials)}
    for path in textures.glob('*.png'):
        if path.name not in keep:path.unlink()
    meta=dict(id='mondstadt',name='Mondstadt',units_per_metre=1,spawns=spawns,pickup_spawns=pickups,windmills=windmills,windmill_mesh=windmill_mesh,triggers=[dict(kind='hurt',planes=[[0,1,0,-5.8],[0,-1,0,79],[1,0,0,274],[-1,0,0,274],[0,0,1,374],[0,0,-1,374]])],lightmaps=[],sky='sky.png',materials=materials,counts={'source_city_triangles':len(a),'public_staircases':len(stairs),'building_foundations':building_foundations,'street_edge_foundations':foundation_count,'entrance_staircases':entrance_staircases},collision_triangles=len(indices),render_triangles=sum(len(m)//3 for m in meshes)+sum(len(data)//3 for _,data in hd_chunks))
    (BUILT/'map.json').write_text(json.dumps(meta,indent=2)+'\n');print(json.dumps({k:meta[k] for k in ('counts','collision_triangles','render_triangles')},indent=2))

if __name__=='__main__':main()
