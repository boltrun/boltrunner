"""Restore the omitted stone townhouse's original silhouette and facade layout."""
from pathlib import Path
import shutil,json
import numpy as np
from PIL import Image
HERE=Path(__file__).resolve().parent

def prepare(textures,first_material):
    source=HERE/'hd/textures'
    stone=Image.open(next(source.glob('*-Area_Mdbuild_Wall06_Diffuse.png'))).convert('RGB')
    roof=next(source.glob('*-Area_MdBuild_House_Roof05_Diffuse.png'))
    trim=Image.open(next(source.glob('*-mond_house_outd-3e4ef29c.png'))).convert('RGB')
    # These rectangles are original, full-resolution Mondstadt window/door trim.
    window=trim.crop((52,1030,280,1523)).transpose(Image.Transpose.ROTATE_180)
    door=trim.crop((590,1030,810,1420))
    regions=[
        ('long',(649,963,760,1015),(1536,768),[(647,976,3,7),(659,976,2,7),(669,975,2,8),(682,977,3,7),(720,976,3,8),(733,975,4,9),(743,975,4,9),(755,976,4,7),(647,995,3,7),(658,994,4,8),(668,994,4,8),(682,995,3,7),(720,995,4,7),(732,994,5,8),(743,994,4,8),(755,995,4,7),(696,974,12,10)],[(696,991,13,16)]),
        ('gable',(803,942,870,1024),(768,1024),[(805,980,4,8),(824,979,5,10),(846,979,5,10),(865,980,3,9),(805,1003,4,8),(865,1002,3,9)],[(831,1002,12,16)]),
        ('dormer',(0,0,1,1),(384,512),[],[]),
        ('tower',(0,0,1,1),(512,1536),[],[]),
        ('wing',(0,0,1,1),(512,1280),[],[]),
    ]
    materials=[];bounds={}
    for name,(x0,y0,x1,y1),size,windows,doors in regions:
        # Keep window counts and positions measured from the retained HLOD UVs;
        # replace their baked 3-pixel patches with original sharp trim materials.
        w,h=size;across,down=(1.2,4.5) if name=='tower' else (1.2,3.8) if name=='wing' else (6,2.5);tw=max(1,int(w/across));th=max(1,int(h/down));tile=stone.resize((tw,th),Image.Resampling.LANCZOS);image=Image.new('RGB',size)
        for x in range(0,w,tw):
            for y in range(0,h,th):image.paste(tile,(x,y))
        for x,y,ww,hh in windows+doors:
            if not (x0<=x<x1 and y0<=y<y1):continue
            px=int((x-x0)*w/(x1-x0));py=int((y-y0)*h/(y1-y0));pw=max(2,int(ww*w/(x1-x0)));ph=max(2,int(hh*h/(y1-y0)))
            ornament=door if (x,y,ww,hh) in doors else window
            image.paste(ornament.resize((pw,ph),Image.Resampling.LANCZOS),(px,py))
        if name in ('tower','wing'):
            for y in ([260,710,1110] if name=='tower' else [250,790]):image.paste(window.resize((128,230),Image.Resampling.LANCZOS),(192,y))
        if name=='dormer':image.paste(window.resize((180,320),Image.Resampling.LANCZOS),(102,105))
        filename='stone-townhouse-'+name+'.png';image.save(textures/filename)
        index=first_material+len(materials);bounds[name]=(index,np.array([x0/1024,y0/1024]),np.array([(x1-x0)/1024,(y1-y0)/1024]))
        materials.append(dict(id=index,name='mondstadt/restored-townhouse/'+name,texture=filename,glow=None,width=w,height=h,emissive=False,alpha=False,blend=False))
    wall_index=first_material+len(materials);stone.save(textures/'stone-townhouse-wall.png')
    materials.append(dict(id=wall_index,name='mondstadt/restored-townhouse/stone',texture='stone-townhouse-wall.png',glow=None,width=stone.width,height=stone.height,emissive=False,alpha=False,blend=False))
    filename='stone-townhouse-roof.png';shutil.copyfile(roof,textures/filename);index=first_material+len(materials)
    materials.append(dict(id=index,name='mondstadt/restored-townhouse/roof',texture=filename,glow=None,width=1024,height=1024,emissive=False,alpha=False,blend=False))
    # Explicit source IDs, so neither adjacent houses nor public paving change.
    wall_ids={5785:'long',5786:'long',5787:'long',5788:'long',**{i:'gable' for i in range(5805,5811)}}
    roof_ids={414,415,591,592,420,421,597,598,422,423,599,600,424,425,601,602}
    restoration=json.loads((HERE/'restoration.json').read_text())
    fortress=set(restoration['fortifications']['source_faces'])
    tower_faces=set(restoration['castle_towers']['source_faces'])
    wing_faces=set(restoration['castle_front_wings']['source_faces'])
    castle_roofs={410,411,412,413,*range(579,591),416,417,418,419,593,594,595,596}
    atlas=np.asarray(Image.open(HERE/'exact/city-atlas.png').convert('RGB'))
    retained={face for house in restoration['retained_neighbour_houses']['assemblies'] for face in house['source_faces']}
    def restore(source_id,data):
        # Townhouse roof/facade IDs were included in the broad fortification
        # restoration mask. Their complete original assembly keeps its own
        # red roofing and timber facade atlas, never a stone wall material.
        if source_id in retained:return None
        if source_id in fortress or source_id in castle_roofs:
            tex=data[:,6:8].mean(0);rgb=atlas[min(1023,int(tex[1]*1024)),min(1023,int(tex[0]*1024))]
            is_roof=source_id in castle_roofs or (tex[1]<.14 and tex[0]<.38 and rgb[2]>rgb[0]*1.05)
            data=data.copy();axis=int(np.argmax(abs(data[:,3:6].mean(0))));axes=[i for i in range(3) if i!=axis];data[:,6:8]=data[:,axes]/(3 if is_roof else 4)
            return index if is_roof else wall_index,data
        if source_id in tower_faces or source_id in wing_faces:
            data=data.copy();uu=data[:,6].copy();data[:,6]=(uu-uu.min())/max(float(np.ptp(uu)),1e-6)
            top,height=(31.6,17.3) if source_id in tower_faces else (28.9,14.7)
            data[:,7]=(top-data[:,1])/height
            return bounds['tower' if source_id in tower_faces else 'wing'][0],data
        if source_id in roof_ids:
            normal=data[:,3:6].mean(0);axes=[i for i in range(3) if i!=int(np.argmax(abs(normal)))];data=data.copy();data[:,6:8]=data[:,axes]/3
            return index,data
        if source_id in range(5811,5823):
            data=data.copy();axis=int(np.argmax(abs(data[:,3:6].mean(0))));axes=[i for i in range(3) if i!=axis];data[:,6:8]=data[:,axes]/4
            return wall_index,data
        if source_id in range(5858,5867):
            group=(source_id-5858)//3;x0,x1=[(.6836,.6899),(.7026,.7090),(.665,.6719)][group]
            data=data.copy();data[:,6:8]=(data[:,6:8]-[x0,1-.0538])/[x1-x0,.0538-.0382]
            return bounds['dormer'][0],data
        if source_id not in wall_ids:return None
        material,origin,span=bounds[wall_ids[source_id]];data=data.copy();data[:,6:8]=(data[:,6:8]-origin)/span
        return material,data
    return materials,restore
