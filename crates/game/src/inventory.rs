use bevy::world_serialization::WorldAssetRoot;
use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use boltrunner_shared::{
    PlayerState,
    inventory::{GroundWeapon, reachable},
    weapon::ShockField,
};
use lightyear::prelude::Predicted;

pub struct InventoryVisualsPlugin;
impl Plugin for InventoryVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (
                attach_weapons,
                attach_fields,
                animate_fields,
                prompt,
                weapon_status,
            )
                .chain()
                .after(boltrunner_client::update_session),
        );
    }
}
#[derive(Component)]
struct WeaponStatus;
#[derive(Component)]
struct GroundVisual;
#[derive(Component)]
struct FieldVisual;
#[derive(Component)]
struct Prompt;
#[derive(Component)]
struct PromptText;
#[derive(Resource)]
struct FieldAssets {
    arcs: Handle<Mesh>,
    blue: Handle<StandardMaterial>,
    white: Handle<StandardMaterial>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        WeaponStatus,
        Text::default(),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: px(24),
            top: px(100),
            display: Display::None,
            ..default()
        },
    ));
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for arm in 0..8 {
        let angle = std::f32::consts::TAU * arm as f32 / 8.0;
        let point = |step: usize| {
            let radius = 0.08 + step as f32 * 0.15;
            let theta = angle + ((arm * 7 + step * 11) as f32).sin() * 0.13;
            Vec3::new(theta.cos() * radius, 0.0, theta.sin() * radius)
        };
        for step in 0..6 {
            let a = point(step);
            let b = point(step + 1);
            let side = (b - a).cross(Vec3::Y).normalize_or_zero() * 0.009;
            let base = vertices.len() as u32;
            for p in [a - side, a + side, b + side, b - side] {
                vertices.push(p.to_array());
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; vertices.len()];
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(bevy::mesh::Indices::U32(indices));
    let make = |color: Color| StandardMaterial {
        base_color: color,
        unlit: true,
        cull_mode: None,
        alpha_mode: AlphaMode::Blend,
        ..default()
    };
    commands.insert_resource(FieldAssets {
        arcs: meshes.add(mesh),
        blue: materials.add(make(Color::srgba(0.12, 0.65, 1.0, 0.9))),
        white: materials.add(make(Color::srgba(0.75, 0.92, 1.0, 0.8))),
    });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(28),
                width: percent(100),
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            Prompt,
        ))
        .with_children(|p| {
            p.spawn((
                Text::default(),
                Node {
                    padding: UiRect::axes(px(12), px(6)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.03, 0.04, 0.75)),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                TextLayout::justify(Justify::Center),
                PromptText,
            ));
        });
}

fn attach_weapons(
    mut commands: Commands,
    assets: Res<crate::weapon::WeaponAssets>,
    items: Query<(Entity, &GroundWeapon), Without<GroundVisual>>,
    mut roots: Query<(&GroundWeapon, &mut Transform), With<GroundVisual>>,
) {
    for (item, mut transform) in &mut roots {
        transform.translation = item.position + Vec3::Y * if item.map_spawn { -0.65 } else { -0.2 };
    }
    for (entity, item) in &items {
        commands
            .entity(entity)
            .insert((
                GroundVisual,
                Transform::from_translation(
                    item.position + Vec3::Y * if item.map_spawn { -0.65 } else { -0.2 },
                ),
                Visibility::Inherited,
            ))
            .with_children(|p| {
                let model = crate::weapon_visual::geometry(item.weapon.kind);
                p.spawn((
                    WorldAssetRoot(assets.scenes[item.weapon.kind.index()].clone()),
                    Transform::from_translation(Vec3::Y * (-model.min_x * model.scale.x + 0.035))
                        .with_rotation(Quat::from_euler(
                            EulerRot::YXZ,
                            0.45,
                            0.0,
                            std::f32::consts::FRAC_PI_2,
                        ))
                        .with_scale(model.scale),
                    NotShadowCaster,
                ));
            });
    }
}
fn attach_fields(
    mut commands: Commands,
    assets: Res<FieldAssets>,
    fields: Query<(Entity, &ShockField), Without<FieldVisual>>,
) {
    for (entity, field) in &fields {
        commands
            .entity(entity)
            .insert((
                FieldVisual,
                Transform::from_translation(field.position),
                Visibility::Inherited,
            ))
            .with_children(|p| {
                p.spawn((
                    Mesh3d(assets.arcs.clone()),
                    MeshMaterial3d(assets.blue.clone()),
                    Transform::default(),
                    NotShadowCaster,
                ));
                p.spawn((
                    Mesh3d(assets.arcs.clone()),
                    MeshMaterial3d(assets.white.clone()),
                    Transform::from_rotation(Quat::from_rotation_y(0.3))
                        .with_scale(Vec3::splat(0.6)),
                    NotShadowCaster,
                ));
            });
    }
}
fn animate_fields(
    time: Res<Time>,
    mut fields: Query<(&ShockField, &mut Transform), With<FieldVisual>>,
) {
    for (field, mut transform) in &mut fields {
        let age =
            (field.total_ticks - field.remaining_ticks) as f32 / boltrunner_shared::TICK_HZ as f32;
        let fade = (field.remaining_ticks as f32 / 30.0).min(1.0);
        let pulse = 0.92 + 0.08 * (age * 37.0 + time.elapsed_secs() * 0.1).sin();
        transform.translation = field.position;
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, field.normal)
            * Quat::from_rotation_y((age * 24.0).sin() * 0.12);
        transform.scale = Vec3::splat(field.radius * pulse * fade);
    }
}
fn prompt(
    session: Res<boltrunner_client::Session>,
    settings: Res<boltrunner_client::tuning::TuningClient>,
    players: Query<&PlayerState, With<Predicted>>,
    items: Query<(Entity, &GroundWeapon)>,
    mut root: Single<&mut Node, With<Prompt>>,
    mut text: Single<&mut Text, With<PromptText>>,
) {
    let item = if session.is_playing() {
        players.iter().next().and_then(|state| {
            items
                .iter()
                .filter(|(_, item)| {
                    !boltrunner_shared::inventory::owns(&state.weapon, item.weapon.kind)
                        && reachable(state, item.position, settings.values().rules.pickup_radius)
                })
                .min_by(|(a, wa), (b, wb)| {
                    wa.position
                        .distance_squared(state.position)
                        .total_cmp(&wb.position.distance_squared(state.position))
                        .then(a.to_bits().cmp(&b.to_bits()))
                })
                .map(|(_, item)| (state, item))
        })
    } else {
        None
    };
    if let Some((state, item)) = item {
        root.display = Display::Flex;
        let slot = |number: u8| {
            state
                .weapon
                .held(number)
                .map(|w| w.kind.name())
                .unwrap_or("Empty")
        };
        text.0 = format!(
            "{} · {}\n2 → {}    3 → {}",
            item.weapon.kind.name(),
            item.weapon.ammo_label(state.tuning),
            slot(1),
            slot(2)
        );
    } else {
        root.display = Display::None;
    }
}

fn weapon_status(
    session: Res<boltrunner_client::Session>,
    players: Query<&PlayerState, With<Predicted>>,
    time: Res<Time<Real>>,
    mut elapsed: Local<f32>,
    mut panel: Single<(&mut Text, &mut Node), With<WeaponStatus>>,
) {
    let Some(state) = players
        .iter()
        .next()
        .filter(|s| session.is_playing() && s.death.is_none())
    else {
        panel.1.display = Display::None;
        return;
    };
    panel.1.display = Display::Flex;
    *elapsed += time.delta_secs();
    if *elapsed < 0.1 {
        return;
    }
    *elapsed = 0.0;
    use std::fmt::Write;
    let mut text = String::new();
    for slot in 0..3 {
        let contents = state
            .weapon
            .held(slot)
            .map(|held| format!("{} · {}", held.kind.name(), held.ammo_label(state.tuning)))
            .unwrap_or_else(|| "—".into());
        let _ = writeln!(
            text,
            "{}{}  {}",
            if slot == state.weapon.active_slot {
                "› "
            } else {
                "  "
            },
            slot + 1,
            contents
        );
    }
    if state.weapon.reload_ticks > 0 {
        let _ = writeln!(
            text,
            "{} · {:.1}s",
            if state.weapon.selected.loads_individually() {
                "Loading round · fire to interrupt"
            } else {
                "Reloading"
            },
            state.weapon.reload_ticks as f64 / boltrunner_shared::TICK_HZ
        );
    } else if state.weapon.selected.ammo_limit(state.tuning).is_some() {
        text.push_str("R · reload · ∞ reserve\n");
    }
    use boltrunner_shared::arsenal::WeaponKind;
    if state.weapon.selected == WeaponKind::MachineGun {
        let _ = writeln!(
            text,
            "Heat: {:.0}%{}",
            state.weapon.heat * 100.0,
            if state.weapon.overheated {
                " / cooling"
            } else {
                ""
            }
        );
    }
    if state.weapon.selected == WeaponKind::Laser && state.weapon.charge_ticks > 0 {
        let _ = writeln!(
            text,
            "Charge: {:.0}%",
            (state.weapon.charge_ticks as f32
                / (state.tuning.laser_charge * boltrunner_shared::TICK_HZ as f32).max(1.0)
                * 100.0)
                .min(100.0)
        );
    }
    if let Some(drone) = state.drone {
        let _ = writeln!(
            text,
            "Drone: {} HP · {:.1}s · {}",
            drone.health,
            drone.remaining_ticks as f64 / boltrunner_shared::TICK_HZ,
            if drone.controlling {
                "piloting"
            } else {
                "hovering"
            }
        );
    } else if state.weapon.selected == WeaponKind::Drone {
        let cooldown = state.weapon.cooldowns[WeaponKind::Drone.index()];
        if cooldown > 0 {
            let _ = writeln!(
                text,
                "Drone launch ready in {:.1}s",
                cooldown as f64 / boltrunner_shared::TICK_HZ
            );
        } else {
            text.push_str("LMB · launch · unlimited controller");
        }
    }
    panel.0.0 = text;
}
