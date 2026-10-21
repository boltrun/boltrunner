use crate::baked_material::BakedMaterial;
use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::Skybox,
    image::{
        ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler,
        ImageSamplerDescriptor,
    },
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{TextureFormat, TextureViewDescriptor, TextureViewDimension},
};
use boltrunner_shared::level::{self, BinaryReader};

#[derive(Resource)]
pub struct MapTextures {
    images: Vec<Handle<Image>>,
    mipmapped: Vec<Handle<Image>>,
    sky: Option<Handle<Image>>,
    map: &'static str,
    prepared: usize,
    ready: bool,
}

#[derive(Component)]
pub struct MapGeometry;

pub fn build_map(
    mut session: ResMut<boltrunner_client::Session>,
    previous: Option<Res<MapTextures>>,
    old_geometry: Query<Entity, With<MapGeometry>>,
    mut camera: Single<(Entity, &mut Transform), With<crate::view::PlayerCamera>>,
    mut progress: ResMut<crate::loading::LoadingProgress>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BakedMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(id) = session.map.as_deref() else {
        if session.phase == boltrunner_client::SessionPhase::Connecting && progress.percent == 100 {
            *progress = default();
        }
        return;
    };
    if let Some(previous) = previous
        && previous.map == id
    {
        session.map_ready = previous.ready;
        return;
    }
    for entity in &old_geometry {
        commands.entity(entity).despawn();
    }
    commands.entity(camera.0).remove::<Skybox>();
    *progress = crate::loading::LoadingProgress::default();
    let _ = level::world();
    let spawn = boltrunner_shared::arena::spawn(0, default());
    camera.1.translation = spawn.position
        + Vec3::Y * boltrunner_shared::tuning::SimulationTuning::default().eye_height;
    camera.1.rotation = Quat::from_rotation_y(spawn.yaw);
    let mut textures = Vec::new();
    let mut mipmapped = Vec::new();
    let pixelated = level::data().pixelated;
    let mut texture = |name: &str, repeat: bool, mipmaps: bool| {
        let handle = assets
            .load_builder()
            .with_settings(move |settings: &mut ImageLoaderSettings| {
                let mode = if repeat {
                    ImageAddressMode::Repeat
                } else {
                    ImageAddressMode::ClampToEdge
                };
                settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: mode,
                    address_mode_v: mode,
                    mag_filter: if pixelated && repeat {
                        ImageFilterMode::Nearest
                    } else {
                        ImageFilterMode::Linear
                    },
                    min_filter: if pixelated && repeat {
                        ImageFilterMode::Nearest
                    } else {
                        ImageFilterMode::Linear
                    },
                    anisotropy_clamp: if mipmaps && !pixelated { 4 } else { 1 },
                    ..ImageSamplerDescriptor::linear()
                });
            })
            .load(format!("{}/built/textures/{name}", level::active().id));
        textures.push(handle.clone());
        if mipmaps {
            mipmapped.push(handle.clone());
        }
        handle
    };
    let lightmaps: Vec<_> = level::data()
        .lightmaps
        .iter()
        .map(|name| texture(name, false, false))
        .collect();
    let white = images.add(Image::default());
    let material_handles: Vec<Vec<_>> = level::data()
        .materials
        .iter()
        .map(|material| {
            // Keep cutout coverage and translucent edges unchanged. The opaque
            // world's repeating textures use trilinear mipmaps with 4x anisotropy.
            let mipmaps = !material.alpha && !material.blend;
            let diffuse = texture(&material.texture, true, mipmaps);
            let glow = material
                .glow
                .as_ref()
                .map(|name| texture(name, true, mipmaps));
            // Slot 0 is explicit vertex lighting/fullbright; the remaining slots
            // correspond to the bake's irradiance atlases.
            // DarkPlaces uses 2x overbright and 2.336x sRGB lightmap compensation.
            std::iter::once(&white)
                .chain(lightmaps.iter())
                .map(|lightmap| {
                    materials.add(BakedMaterial {
                        base_lighting: Vec2::new(
                            if material.emissive { 1.0 } else { 4.672 },
                            if glow.is_some() { 1.0 } else { 0.0 },
                        ),
                        settings: Vec4::new(
                            if material.emissive { 1.0 } else { 4.672 },
                            if glow.is_some() { 1.0 } else { 0.0 },
                            if material.alpha { 0.5 } else { 0.0 },
                            if material.blend { 0.0 } else { 1.0 },
                        ),
                        diffuse: diffuse.clone(),
                        lightmap: lightmap.clone(),
                        glow: glow.clone().unwrap_or_else(|| white.clone()),
                        alpha: if material.blend {
                            AlphaMode::Blend
                        } else if material.alpha {
                            AlphaMode::Mask(0.5)
                        } else {
                            AlphaMode::Opaque
                        },
                        double_sided: material.alpha || (material.blend && !pixelated),
                    })
                })
                .collect()
        })
        .collect();
    let mut reader = BinaryReader::new(level::active().render);
    for _ in 0..reader.u32() {
        let material = reader.u32() as usize;
        let lightmap = reader.u32() as i32;
        let count = reader.u32() as usize;
        let index_count = reader.u32() as usize;
        let mut positions = Vec::with_capacity(count);
        let mut normals = Vec::with_capacity(count);
        let mut uvs = Vec::with_capacity(count);
        let mut lightmap_uvs = Vec::with_capacity(count);
        let mut colors = Vec::with_capacity(count);
        for _ in 0..count {
            positions.push([reader.f32(), reader.f32(), reader.f32()]);
            normals.push([reader.f32(), reader.f32(), reader.f32()]);
            uvs.push([reader.f32(), reader.f32()]);
            lightmap_uvs.push([reader.f32(), reader.f32()]);
            colors.push([reader.f32(), reader.f32(), reader.f32(), reader.f32()]);
        }
        let indices = (0..index_count).map(|_| reader.u32()).collect();
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, lightmap_uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices));
        commands.spawn((
            MapGeometry,
            Name::new(level::data().materials[material].name.clone()),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material_handles[material][(lightmap + 1) as usize].clone()),
        ));
    }
    reader.finish();
    let sky = level::data()
        .sky
        .as_ref()
        .map(|name| texture(name, false, false));
    commands.insert_resource(MapTextures {
        images: textures,
        mipmapped,
        sky,
        map: level::active().id,
        prepared: 0,
        ready: false,
    });
    // Static surfaces use baked lighting; dynamic players use ambient light.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
        ..default()
    });
}

pub fn finish_loading(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    textures: Option<ResMut<MapTextures>>,
    weapon: Option<Res<crate::weapon::WeaponAssets>>,
    camera: Single<(Entity, &bevy::camera::Exposure), With<crate::view::PlayerCamera>>,
    mut progress: ResMut<crate::loading::LoadingProgress>,
    mut session: ResMut<boltrunner_client::Session>,
) {
    if progress.percent == 100 || progress.error.is_some() {
        return;
    }
    let (Some(mut textures), Some(weapon)) = (textures, weapon) else {
        return;
    };
    if session.map.as_deref() != Some(textures.map) {
        return;
    }
    let mut ids: Vec<_> = textures
        .images
        .iter()
        .map(|handle| handle.id().untyped())
        .collect();
    ids.push(weapon.scene.id().untyped());
    for id in &ids {
        let failure = match assets.load_state(*id) {
            bevy::asset::LoadState::Failed(error) => Some(error),
            _ => match assets.recursive_dependency_load_state(*id) {
                bevy::asset::RecursiveDependencyLoadState::Failed(error) => Some(error),
                _ => None,
            },
        };
        if let Some(error) = failure {
            progress.error = Some(error.to_string());
            return;
        }
    }
    let loaded = ids
        .iter()
        .filter(|id| assets.is_loaded_with_dependencies(**id))
        .count();
    // Each ready asset, prepared mip chain, and sky setup completes one loading task.
    let total_tasks = ids.len() + textures.mipmapped.len() + 1;
    let value = (100 * loaded / total_tasks) as u8;
    if value > progress.percent {
        progress.advance(value);
    }
    if loaded != ids.len() {
        return;
    }

    // Process a batch, yielding between textures when about one 60 Hz frame is spent.
    let started = bevy::platform::time::Instant::now();
    let budget = std::time::Duration::from_micros(16_667);
    while let Some(handle) = textures.mipmapped.get(textures.prepared) {
        let mut image = images.get_mut(handle).expect("loaded map image");
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
        let width = image.width() as usize;
        let height = image.height() as usize;
        if image.texture_descriptor.mip_level_count == 1 {
            image.texture_descriptor.mip_level_count = crate::mipmaps::append_srgb_mips(
                image.data.as_mut().expect("decoded map pixels"),
                width,
                height,
            );
        }
        textures.prepared += 1;
        progress.advance((100 * (ids.len() + textures.prepared) / total_tasks) as u8);
        if started.elapsed() >= budget {
            return;
        }
    }
    if let Some(sky) = &textures.sky {
        let mut image = images.get_mut(sky).expect("loaded sky image");
        if image.texture_descriptor.size.depth_or_array_layers == 1 {
            image
                .reinterpret_stacked_2d_as_array(6)
                .expect("six square sky faces");
        }
        image.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        });
        commands.entity(camera.0).insert(Skybox {
            image: Some(sky.clone()),
            brightness: camera.1.exposure().recip(),
            ..default()
        });
    }
    textures.ready = true;
    session.map_ready = true;
    progress.advance(100);
}

pub fn tune_lighting(
    settings: Res<boltrunner_client::tuning::TuningClient>,
    textures: Option<Res<MapTextures>>,
    mut previous: Local<Option<(boltrunner_shared::tuning::PresentationTuning, &'static str)>>,
    mut materials: ResMut<Assets<BakedMaterial>>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    let Some(textures) = textures else { return };
    let tuning = settings.values().presentation;
    if !textures.is_added()
        && previous.is_some_and(|(p, map)| {
            map == level::active().id
                && p.lightmap_brightness == tuning.lightmap_brightness
                && p.glow_brightness == tuning.glow_brightness
                && p.ambient_brightness == tuning.ambient_brightness
        })
    {
        return;
    }
    for (_, material) in materials.iter_mut() {
        material.settings.x = material.base_lighting.x * tuning.lightmap_brightness;
        material.settings.y = material.base_lighting.y * tuning.glow_brightness;
    }
    ambient.brightness = tuning.ambient_brightness;
    *previous = Some((tuning, level::active().id));
}
