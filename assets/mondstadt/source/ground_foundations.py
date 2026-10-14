"""Close genuinely exposed street-terrace edges with measured masonry walls."""
import math
import numpy as np


def prepare(points,normals,uv,stair_volumes,quad,material,to_world,floor_faces):
    mean_uv=uv.mean(1)
    public=np.zeros(len(points),dtype=bool)
    public[floor_faces]=True
    floors=points[public&(normals.mean(1)[:,1]>.65)]
    if not len(floors):return 0
    xz=floors[:,:,[0,2]];A=xz[:,1]-xz[:,0];B=xz[:,2]-xz[:,0]
    den=A[:,0]*B[:,1]-A[:,1]*B[:,0]
    den[np.abs(den)<1e-10]=np.nan
    walls=points[abs(normals.mean(1)[:,1])<.6]
    edge1=walls[:,1]-walls[:,0];edge2=walls[:,2]-walls[:,0]

    def heights(point):
        C=point[[0,2]]-xz[:,0]
        u=(C[:,0]*B[:,1]-C[:,1]*B[:,0])/den
        v=(A[:,0]*C[:,1]-A[:,1]*C[:,0])/den
        h=floors[:,0,1]+u*(floors[:,1,1]-floors[:,0,1])+v*(floors[:,2,1]-floors[:,0,1])
        return h[(u>=-1e-6)&(v>=-1e-6)&(u+v<=1+1e-6)]

    def supported_wall(origin,direction):
        h=np.cross(np.broadcast_to(direction,edge2.shape),edge2)
        det=np.sum(edge1*h,axis=1);ok=abs(det)>1e-8
        inv=np.divide(1.,det,out=np.zeros_like(det),where=ok)
        s=origin-walls[:,0];u=inv*np.sum(s*h,axis=1)
        q=np.cross(s,edge1);v=inv*(q@direction)
        t=inv*np.sum(edge2*q,axis=1)
        return np.any(ok&(u>=-.01)&(v>=-.01)&(u+v<=1.01)&(t>=0)&(t<=.5))

    count=0;seen=set()
    for floor in floors:
        for start,end in zip(floor,np.roll(floor,-1,axis=0)):
            direction=end-start;length=np.linalg.norm(direction[[0,2]])
            if length<.1:continue
            outward=np.cross(direction,[0.,1.,0.]);outward/=np.linalg.norm(outward)
            for index in range(math.ceil(length/3)):
                steps=math.ceil(length/3)
                p=start+direction*(index/steps);q=start+direction*((index+1)/steps)
                middle=(p+q)/2;key=tuple(np.round(middle,3))
                if key in seen:continue
                seen.add(key)
                probe=middle+outward*.18;levels=heights(probe)
                if np.any(abs(levels-middle[1])<.25):continue
                below=levels[levels<middle[1]-.45]
                if not len(below):continue
                # Existing source walls already close almost every edge; do
                # not add coincident faces or recreate them with new UVs.
                if supported_wall(probe+[0.,-.15,0.],-outward):continue
                world_middle=to_world(middle)
                if any(all(float(world_middle@axis-distance)<=.2 for axis,distance in volume) for volume in stair_volumes):continue
                lower=[]
                for vertex in (p,q):
                    candidates=heights(vertex+outward*.18)
                    candidates=candidates[candidates<vertex[1]-.25]
                    lower.append(float(candidates.max())-.025 if len(candidates) else float(below.max())-.025)
                face=np.array([p,[p[0],lower[0],p[2]],[q[0],lower[1],q[2]],q])
                distance=np.linalg.norm(q[[0,2]]-p[[0,2]])
                tex=[[0,p[1]/2],[0,lower[0]/2],[distance/2,lower[1]/2],[distance/2,q[1]/2]]
                quad(face,material,tex);count+=1
    return count


def prepare_buildings(edges,points,floor_faces,collision,quad,material):
    """Support HD facade bottoms where they overhang a lower public street."""
    floors=points[floor_faces]
    xz=floors[:,:,[0,2]];A=xz[:,1]-xz[:,0];B=xz[:,2]-xz[:,0]
    den=A[:,0]*B[:,1]-A[:,1]*B[:,0];den[abs(den)<1e-10]=np.nan
    walls=np.asarray(collision)
    normal=np.cross(walls[:,1]-walls[:,0],walls[:,2]-walls[:,0])
    normal/=np.maximum(np.linalg.norm(normal,axis=1)[:,None],1e-10)
    walls=walls[abs(normal[:,1])<.6]
    e1=walls[:,1]-walls[:,0];e2=walls[:,2]-walls[:,0]
    def height(p):
        C=p[[0,2]]-xz[:,0];u=(C[:,0]*B[:,1]-C[:,1]*B[:,0])/den;v=(A[:,0]*C[:,1]-A[:,1]*C[:,0])/den
        h=floors[:,0,1]+u*(floors[:,1,1]-floors[:,0,1])+v*(floors[:,2,1]-floors[:,0,1])
        ok=(u>=-1e-6)&(v>=-1e-6)&(u+v<=1+1e-6)&(h<p[1]+.25)
        return float(h[ok].max()) if np.any(ok) else None
    def blocked(origin,direction):
        q=np.cross(np.broadcast_to(direction,e2.shape),e2);det=(e1*q).sum(1);valid=abs(det)>1e-8
        inv=np.divide(1,det,out=np.zeros_like(det),where=valid);s=origin-walls[:,0];u=inv*(s*q).sum(1);r=np.cross(s,e1);v=inv*(r@direction);t=inv*(e2*r).sum(1)
        return np.any(valid&(u>=-.01)&(v>=-.01)&(u+v<=1.01)&(t>=0)&(t<=1))
    # Facade meshes contain several parallel trim rows. Generate the lower
    # envelope once per wall plane, rather than overlapping a support strip
    # beneath every tessellation edge on the same visible facade.
    planes={}
    for start,end,normal in edges:
        tangent=end-start;tangent[1]=0;tangent/=np.linalg.norm(tangent)
        outward=np.array([tangent[2],0.,-tangent[0]])
        if outward@normal<0:outward=-outward
        tangent=np.array([-outward[2],0.,outward[0]])
        key=(*np.round(outward[[0,2]],3),round(float(start@outward)*50))
        a,b=float(start@tangent),float(end@tangent)
        if a>b:start,end=end,start;a,b=b,a
        planes.setdefault(key,[]).append((a,b,start,end,outward,tangent))
    boundary=[]
    for wall in planes.values():
        breaks=sorted(set(round(value,6) for edge in wall for value in edge[:2]))
        for lo,hi in zip(breaks,breaks[1:]):
            if hi-lo<.025:continue
            mid=(lo+hi)/2;cover=[edge for edge in wall if edge[0]-.00001<=mid<=edge[1]+.00001]
            if not cover:continue
            edge=min(cover,key=lambda e:e[2][1]+(e[3][1]-e[2][1])*(mid-e[0])/(e[1]-e[0]))
            a,b,start,end,outward,_=edge
            boundary.append((start+(end-start)*((lo-a)/(b-a)),start+(end-start)*((hi-a)/(b-a)),outward))
    seen=set();count=0
    for start,end,outward in boundary:
        length=np.linalg.norm((end-start)[[0,2]]);steps=math.ceil(length/1.5)
        outward=outward.copy();outward[1]=0;outward/=np.linalg.norm(outward)
        for i in range(steps):
            p=start+(end-start)*(i/steps);q=start+(end-start)*((i+1)/steps);mid=(p+q)/2
            key=tuple(np.round(mid,3))
            if key in seen:continue
            seen.add(key);ground=height(mid+outward*.18)
            if ground is None or mid[1]-ground<1.4:continue
            probe=mid.copy();probe[1]=ground+min(.8,(mid[1]-ground)/2)
            if blocked(probe+outward*.5,-outward):continue
            bottom=[]
            for vertex in (p,q):
                h=height(vertex+outward*.18)
                bottom.append(min(vertex[1]-.01,h if h is not None else ground)-.025)
            face=np.array([p,[p[0],bottom[0],p[2]],[q[0],bottom[1],q[2]],q])
            if np.dot(np.cross(face[1]-face[0],face[2]-face[0]),outward)<0:face=face[::-1]
            axis=int(np.argmax(abs(outward)));axes=[j for j in range(3) if j!=axis]
            quad(face,material,(face[:,axes]/2).tolist());count+=1
    return count
