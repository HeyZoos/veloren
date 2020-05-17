use crate::{
    anim::{
        character::{CharacterSkeleton, IdleAnimation, SkeletonAttr},
        fixture::FixtureSkeleton,
        Animation, Skeleton,
    },
    mesh::Meshable,
    render::{
        create_pp_mesh, create_skybox_mesh, Consts, FigurePipeline, Globals, Light, Mesh, Model,
        PostProcessLocals, PostProcessPipeline, Renderer, Shadow, SkyboxLocals, SkyboxPipeline,
    },
    scene::{
        camera::{self, Camera, CameraMode},
        figure::{load_mesh, FigureModelCache, FigureState},
        LodData,
    },
    window::{Event, PressState},
};
use common::{
    comp::{humanoid, Body, Loadout},
    figure::Segment,
    terrain::BlockKind,
    vol::{BaseVol, ReadVol, Vox},
};
use log::error;
use vek::*;

#[derive(PartialEq, Eq, Copy, Clone)]
struct VoidVox;
impl Vox for VoidVox {
    fn empty() -> Self { VoidVox }

    fn is_empty(&self) -> bool { true }

    fn or(self, _other: Self) -> Self { VoidVox }
}
struct VoidVol;
impl BaseVol for VoidVol {
    type Error = ();
    type Vox = VoidVox;
}
impl ReadVol for VoidVol {
    fn get<'a>(&'a self, _pos: Vec3<i32>) -> Result<&'a Self::Vox, Self::Error> { Ok(&VoidVox) }
}

fn generate_mesh(segment: &Segment, offset: Vec3<f32>) -> Mesh<FigurePipeline> {
    Meshable::<FigurePipeline, FigurePipeline>::generate_mesh(segment, (offset, Vec3::one())).0
}

struct Skybox {
    model: Model<SkyboxPipeline>,
    locals: Consts<SkyboxLocals>,
}

struct PostProcess {
    model: Model<PostProcessPipeline>,
    locals: Consts<PostProcessLocals>,
}

pub struct Scene {
    globals: Consts<Globals>,
    lights: Consts<Light>,
    shadows: Consts<Shadow>,
    camera: Camera,

    skybox: Skybox,
    postprocess: PostProcess,
    lod: LodData,
    map_bounds: Vec2<f32>,
    backdrop: Option<(Model<FigurePipeline>, FigureState<FixtureSkeleton>)>,

    figure_model_cache: FigureModelCache,
    figure_state: FigureState<CharacterSkeleton>,

    turning: bool,
    char_ori: f32,
}

pub struct SceneData {
    pub time: f64,
    pub delta_time: f32,
    pub tick: u64,
    pub body: Option<humanoid::Body>,
    pub gamma: f32,
    pub figure_lod_render_distance: f32,
    pub mouse_smoothing: bool,
}

impl Scene {
    pub fn new(renderer: &mut Renderer, backdrop: Option<&str>) -> Self {
        let start_angle = 90.0f32.to_radians();
        let resolution = renderer.get_resolution().map(|e| e as f32);

        let map_bounds = Vec2::new(-65536.0, 131071.0);
        let map_border = [0.0, 0.0, 0.0, 0.0];
        let map_image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([0, 0, 0, 0]),
        ));
        let horizon_image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([0, 1, 0, 1]),
        ));

        let mut camera = Camera::new(resolution.x / resolution.y, CameraMode::ThirdPerson);
        camera.set_focus_pos(Vec3::unit_z() * 1.5);
        camera.set_distance(3.4);
        camera.set_orientation(Vec3::new(start_angle, 0.0, 0.0));

        Self {
            globals: renderer.create_consts(&[Globals::default()]).unwrap(),
            lights: renderer.create_consts(&[Light::default(); 32]).unwrap(),
            shadows: renderer.create_consts(&[Shadow::default(); 32]).unwrap(),
            camera,

            skybox: Skybox {
                model: renderer.create_model(&create_skybox_mesh()).unwrap(),
                locals: renderer.create_consts(&[SkyboxLocals::default()]).unwrap(),
            },
            postprocess: PostProcess {
                model: renderer.create_model(&create_pp_mesh()).unwrap(),
                locals: renderer
                    .create_consts(&[PostProcessLocals::default()])
                    .unwrap(),
            },
            lod: LodData::new(renderer, &map_image, &horizon_image, 1, map_border.into()),// Lod::new(renderer, client, settings),
            map_bounds,//: client.world_map.2,

            figure_model_cache: FigureModelCache::new(),
            figure_state: FigureState::new(renderer, CharacterSkeleton::new()),

            backdrop: backdrop.map(|specifier| {
                let mut state = FigureState::new(renderer, FixtureSkeleton::new());
                state.update(
                    renderer,
                    Vec3::zero(),
                    Vec3::new(start_angle.sin(), -start_angle.cos(), 0.0),
                    1.0,
                    Rgba::broadcast(1.0),
                    15.0, // Want to get there immediately.
                    1.0,
                    0,
                    true,
                    false,
                );
                (
                    renderer
                        .create_model(&load_mesh(
                            specifier,
                            Vec3::new(-55.0, -49.5, -2.0),
                            generate_mesh,
                        ))
                        .unwrap(),
                    state
                )
            }),

            turning: false,
            char_ori: /*0.0*/-start_angle,
        }
    }

    pub fn globals(&self) -> &Consts<Globals> { &self.globals }

    pub fn camera_mut(&mut self) -> &mut Camera { &mut self.camera }

    /// Handle an incoming user input event (e.g.: cursor moved, key pressed,
    /// window closed).
    ///
    /// If the event is handled, return true.
    pub fn handle_input_event(&mut self, event: Event) -> bool {
        match event {
            // When the window is resized, change the camera's aspect ratio
            Event::Resize(dims) => {
                self.camera.set_aspect_ratio(dims.x as f32 / dims.y as f32);
                true
            },
            Event::MouseButton(_, state) => {
                self.turning = state == PressState::Pressed;
                true
            },
            Event::CursorMove(delta) if self.turning => {
                self.char_ori += delta.x * 0.01;
                true
            },
            // All other events are unhandled
            _ => false,
        }
    }

    pub fn maintain(&mut self, renderer: &mut Renderer, scene_data: SceneData) {
        self.camera.update(
            scene_data.time,
            /* 1.0 / 60.0 */ scene_data.delta_time,
            scene_data.mouse_smoothing,
        );

        self.camera.compute_dependents(&VoidVol);
        let camera::Dependents {
            view_mat,
            proj_mat,
            cam_pos,
        } = self.camera.dependents();
        const VD: f32 = 115.0; // View Distance
        // const MAP_BOUNDS: Vec2<f32> = Vec2::new(140.0, 2048.0);
        const TIME: f64 = 10.0 * 60.0 * 60.0; //43200.0; // 12 hours*3600 seconds
        const SHADOW_NEAR: f32 = 1.0;
        const SHADOW_FAR: f32 = 25.0;

        if let Err(err) = renderer.update_consts(&mut self.globals, &[Globals::new(
            view_mat,
            proj_mat,
            cam_pos,
            self.camera.get_focus_pos(),
            VD,
            self.lod.tgt_detail as f32,
            self.map_bounds, //MAP_BOUNDS,
            TIME,
            scene_data.time,
            renderer.get_resolution(),
            Vec2::new(SHADOW_NEAR, SHADOW_FAR),
            0,
            0,
            0,
            BlockKind::Air,
            None,
            scene_data.gamma,
            self.camera.get_mode(),
            250.0,
        )]) {
            error!("Renderer failed to update: {:?}", err);
        }

        self.figure_model_cache.clean(scene_data.tick);

        if let Some(body) = scene_data.body {
            let tgt_skeleton = IdleAnimation::update_skeleton(
                self.figure_state.skeleton_mut(),
                scene_data.time,
                scene_data.time,
                &mut 0.0,
                &SkeletonAttr::from(&body),
            );
            self.figure_state
                .skeleton_mut()
                .interpolate(&tgt_skeleton, scene_data.delta_time);
        }

        self.figure_state.update(
            renderer,
            Vec3::zero(),
            Vec3::new(self.char_ori.sin(), -self.char_ori.cos(), 0.0),
            1.0,
            Rgba::broadcast(1.0),
            scene_data.delta_time,
            1.0,
            0,
            true,
            false,
        );
    }

    pub fn render(
        &mut self,
        renderer: &mut Renderer,
        tick: u64,
        body: Option<humanoid::Body>,
        loadout: Option<&Loadout>,
    ) {
        renderer.render_skybox(
            &self.skybox.model,
            &self.globals,
            &self.skybox.locals,
            &self.lod.map,
            &self.lod.horizon,
        );

        if let Some(body) = body {
            let model = &self
                .figure_model_cache
                .get_or_create_model(
                    renderer,
                    Body::Humanoid(body),
                    loadout,
                    tick,
                    CameraMode::default(),
                    None,
                )
                .0;

            renderer.render_figure(
                &model[0],
                &self.globals,
                self.figure_state.locals(),
                self.figure_state.bone_consts(),
                &self.lights,
                &self.shadows,
                &self.lod.map,
                &self.lod.horizon,
            );
        }

        if let Some((model, state)) = &self.backdrop {
            renderer.render_figure(
                model,
                &self.globals,
                state.locals(),
                state.bone_consts(),
                &self.lights,
                &self.shadows,
                &self.lod.map,
                &self.lod.horizon,
            );
        }

        renderer.render_post_process(
            &self.postprocess.model,
            &self.globals,
            &self.postprocess.locals,
        );
    }
}
