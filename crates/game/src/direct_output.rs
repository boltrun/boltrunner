use bevy::{
    camera::{
        CameraOutputMode, ClearColorConfig, Hdr, MainPassResolutionOverride,
        NormalizedRenderTarget, RenderTarget,
    },
    core_pipeline::{
        core_3d::{
            AlphaMask3d, Opaque3d, Transparent3d, main_opaque_pass_3d, main_transparent_pass_3d,
        },
        schedule::{Core3d, Core3dSystems},
        skybox::{SkyboxBindGroup, SkyboxPipelineId},
        upscaling::upscaling,
    },
    ecs::schedule::ScheduleCleanupPolicy,
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases},
        render_resource::{PipelineCache, RenderPassDescriptor, StoreOp},
        renderer::{RenderAdapterInfo, RenderContext, ViewQuery},
        view::{ExtractedView, ViewDepthTexture, ViewTarget, ViewUniformOffset},
    },
    ui_render::{TransparentUi, UiCameraView, UiViewTarget, ui_pass},
};

// Draw the two window views directly to the sRGB output. Offscreen character
// previews retain Bevy's intermediate attachments, MSAA and final output copy.
pub(crate) fn install(app: &mut App) {
    app.add_systems(PostStartup, configure_cameras);
    app.sub_app_mut(RenderApp)
        .add_systems(RenderStartup, install_passes);
}

fn configure_cameras(
    adapter: Res<RenderAdapterInfo>,
    mut cameras: Query<
        (
            &mut Camera,
            Has<crate::view::PlayerCamera>,
            &Msaa,
            Has<Hdr>,
            &RenderTarget,
            Option<&bevy::core_pipeline::tonemapping::Tonemapping>,
        ),
        With<Camera3d>,
    >,
) {
    assert_eq!(
        adapter.backend.to_str(),
        "gl",
        "Direct output targets the WebGL renderer"
    );
    let mut window_views = 0;
    for (mut camera, world, msaa, hdr, target, tone) in &mut cameras {
        if !matches!(
            target,
            RenderTarget::Window(bevy::window::WindowRef::Primary)
        ) {
            continue;
        }
        window_views += 1;
        assert_eq!(*msaa, Msaa::Off);
        assert!(!hdr && camera.viewport.is_none());
        assert_eq!(camera.order, if world { 0 } else { 1 });
        assert_eq!(matches!(camera.clear_color, ClearColorConfig::None), !world);
        assert!(
            tone.is_none_or(|tone| *tone == bevy::core_pipeline::tonemapping::Tonemapping::None)
        );
        assert!(matches!(
            target,
            RenderTarget::Window(bevy::window::WindowRef::Primary)
        ));
        // Bevy still needs Write to acquire and present the window surface.
        // The final copy is skipped per view by offscreen_view below.
        camera.output_mode = CameraOutputMode::default();
    }
    assert_eq!(window_views, 2);
}

fn install_passes(world: &mut World) {
    world.schedule_scope(Core3d, |world, schedule| {
        let policy = ScheduleCleanupPolicy::RemoveSystemsOnly;
        assert_eq!(
            schedule
                .remove_systems_in_set(main_opaque_pass_3d, world, policy)
                .unwrap(),
            1
        );
        assert_eq!(
            schedule
                .remove_systems_in_set(main_transparent_pass_3d, world, policy)
                .unwrap(),
            1
        );
        assert_eq!(
            schedule
                .remove_systems_in_set(ui_pass, world, policy)
                .unwrap(),
            1
        );
        assert_eq!(
            schedule
                .remove_systems_in_set(upscaling, world, policy)
                .unwrap(),
            1
        );
        schedule.add_systems((
            upscaling
                .run_if(offscreen_view)
                .after(Core3dSystems::PostProcess),
            (direct_opaque, direct_transparent)
                .chain()
                .in_set(Core3dSystems::MainPass),
            direct_ui
                .after(Core3dSystems::PostProcess)
                .before(upscaling),
        ));
    });
}

fn offscreen_view(view: ViewQuery<&ExtractedCamera>) -> bool {
    !matches!(
        view.into_inner().target,
        Some(NormalizedRenderTarget::Window(_))
    )
}

fn direct_opaque(
    world: &World,
    view: ViewQuery<(
        &ExtractedCamera,
        &ExtractedView,
        &ViewTarget,
        &ViewDepthTexture,
        Option<&SkyboxPipelineId>,
        Option<&SkyboxBindGroup>,
        &ViewUniformOffset,
        Option<&MainPassResolutionOverride>,
    )>,
    opaques: Res<ViewBinnedRenderPhases<Opaque3d>>,
    masks: Res<ViewBinnedRenderPhases<AlphaMask3d>>,
    pipeline_cache: Res<PipelineCache>,
    mut context: RenderContext,
) {
    let view_entity = view.entity();
    let (camera, view, target, depth, sky_pipeline, sky_group, uniform, resolution_override) =
        view.into_inner();
    assert!(resolution_override.is_none() && camera.viewport.is_none());
    if matches!(camera.target, Some(NormalizedRenderTarget::Window(_))) {
        assert_eq!(
            Some(target.main_texture_format()),
            target.out_texture_view_format()
        );
    }
    let (Some(opaque), Some(mask)) = (
        opaques.get(&view.retained_view_entity),
        masks.get(&view.retained_view_entity),
    ) else {
        return;
    };
    let clear = match camera.clear_color {
        ClearColorConfig::Default => Some(world.resource::<ClearColor>().0.to_linear()),
        ClearColorConfig::Custom(color) => Some(color.to_linear()),
        ClearColorConfig::None => None,
    };
    // Both views share this output attachment's clear-once state. The world
    // clears it; the weapon, transparent geometry and UI retain its color.
    let colors = [
        if matches!(camera.target, Some(NormalizedRenderTarget::Window(_))) {
            target.out_texture_color_attachment(clear)
        } else {
            Some(target.get_color_attachment())
        },
    ];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("direct_opaque"),
        color_attachments: &colors,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "direct_opaque");
    if !opaque.is_empty() {
        if let Err(error) = opaque.render(&mut pass, world, view_entity) {
            error!("Error rendering opaque geometry: {error:?}");
        }
    }
    if !mask.is_empty() {
        if let Err(error) = mask.render(&mut pass, world, view_entity) {
            error!("Error rendering masked geometry: {error:?}");
        }
    }
    if let (Some(pipeline), Some(bind_group)) = (sky_pipeline, sky_group) {
        if let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline.0) {
            pass.set_render_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group.0.0, &[uniform.offset, bind_group.0.1]);
            pass.draw(0..3, 0..1);
        }
    }
    span.end(&mut pass);
}

fn direct_transparent(
    world: &World,
    view: ViewQuery<(
        &ExtractedCamera,
        &ExtractedView,
        &ViewTarget,
        &ViewDepthTexture,
        Option<&MainPassResolutionOverride>,
    )>,
    phases: Res<ViewSortedRenderPhases<Transparent3d>>,
    mut context: RenderContext,
) {
    let view_entity = view.entity();
    let (camera, view, target, depth, resolution_override) = view.into_inner();
    assert!(camera.viewport.is_none() && resolution_override.is_none());
    let Some(phase) = phases.get(&view.retained_view_entity) else {
        return;
    };
    if phase.items.is_empty() {
        return;
    }
    let colors = [
        if matches!(camera.target, Some(NormalizedRenderTarget::Window(_))) {
            target.out_texture_color_attachment(None)
        } else {
            Some(target.get_color_attachment())
        },
    ];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("direct_transparent"),
        color_attachments: &colors,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "direct_transparent");
    if let Err(error) = phase.render(&mut pass, world, view_entity) {
        error!("Error rendering transparent geometry: {error:?}");
    }
    span.end(&mut pass);
}

fn direct_ui(
    world: &World,
    view: ViewQuery<&UiCameraView>,
    ui_views: Query<(&ExtractedView, &UiViewTarget)>,
    targets: Query<(&ViewTarget, &ExtractedCamera)>,
    phases: Res<ViewSortedRenderPhases<TransparentUi>>,
    mut context: RenderContext,
) {
    let ui_view_entity = view.into_inner().0;
    let Ok((view, ui_target)) = ui_views.get(ui_view_entity) else {
        return;
    };
    let Some(phase) = phases.get(&view.retained_view_entity) else {
        return;
    };
    if phase.items.is_empty() {
        return;
    }
    let Ok((target, camera)) = targets.get(ui_target.0) else {
        return;
    };
    assert!(camera.viewport.is_none());
    if matches!(camera.target, Some(NormalizedRenderTarget::Window(_))) {
        assert_eq!(
            Some(target.main_texture_format()),
            target.out_texture_view_format()
        );
    }
    let colors = [
        if matches!(camera.target, Some(NormalizedRenderTarget::Window(_))) {
            target.out_texture_color_attachment(None)
        } else {
            Some(target.get_unsampled_color_attachment())
        },
    ];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("direct_ui"),
        color_attachments: &colors,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "direct_ui");
    if let Err(error) = phase.render(&mut pass, world, ui_view_entity) {
        error!("Error rendering UI: {error:?}");
    }
    span.end(&mut pass);
}
