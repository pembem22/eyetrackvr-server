use crate::camera::CAMERA_FRAME_SIZE;
use crate::inference::{
    EyeState, FRAME_CROP_H, FRAME_CROP_W, FRAME_CROP_X, FRAME_CROP_Y, FRAME_RESIZE_H,
    FRAME_RESIZE_W,
};
use crate::{Frame, camera_texture::CameraTexture};
use async_broadcast::Receiver;
use imgui::ImColor32;

pub struct AppRendererContext {
    pub l_rx: Receiver<Frame>,
    pub r_rx: Receiver<Frame>,
    pub f_rx: Receiver<Frame>,

    pub l_raw_rx: Receiver<EyeState>,
    pub r_raw_rx: Receiver<EyeState>,
    pub filtered_eyes_rx: Receiver<(EyeState, EyeState)>,
}
pub(crate) struct AppRenderer {
    r_texture: CameraTexture,
    f_texture: CameraTexture,
    l_texture: CameraTexture,

    l_raw_eye: EyeState,
    r_raw_eye: EyeState,
    filtered_eyes: (EyeState, EyeState),
}

impl AppRenderer {
    pub(crate) fn new(device: &mut wgpu::Device, renderer: &mut imgui_wgpu::Renderer) -> Self {
        AppRenderer {
            l_texture: CameraTexture::new(device, renderer, Some("L texture")),
            r_texture: CameraTexture::new(device, renderer, Some("R texture")),
            f_texture: CameraTexture::new(device, renderer, Some("F texture")),

            l_raw_eye: EyeState::default(),
            r_raw_eye: EyeState::default(),
            filtered_eyes: (EyeState::default(), EyeState::default()),
        }
    }

    pub(crate) fn update(
        &mut self,
        renderer_context: &mut AppRendererContext,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        self.l_texture
            .update_texture(&mut renderer_context.l_rx, queue, renderer);
        self.r_texture
            .update_texture(&mut renderer_context.r_rx, queue, renderer);
        self.f_texture
            .update_texture(&mut renderer_context.f_rx, queue, renderer);

        self.l_raw_eye = renderer_context
            .l_raw_rx
            .try_recv()
            .unwrap_or(self.l_raw_eye);
        self.r_raw_eye = renderer_context
            .r_raw_rx
            .try_recv()
            .unwrap_or(self.r_raw_eye);

        self.filtered_eyes = renderer_context
            .filtered_eyes_rx
            .try_recv()
            .unwrap_or(self.filtered_eyes);
    }

    pub(crate) fn render(&self, ui: &imgui::Ui) {
        ui.window("Camera Feeds").build(move || {
            let group = ui.begin_group();
            self.l_texture.build(ui);
            let l_fps = self.l_texture.get_fps();
            ui.text(format!("Left Eye, fps: {l_fps:03.1}"));
            group.end();

            ui.same_line();

            let group = ui.begin_group();
            self.r_texture.build(ui);
            let r_fps = self.r_texture.get_fps();
            ui.text(format!("Right Eye, fps: {r_fps:03.1}"));
            group.end();

            ui.same_line();

            let group = ui.begin_group();
            self.f_texture.build(ui);
            let f_fps = self.f_texture.get_fps();
            ui.text(format!("Face, fps: {f_fps:03.1}"));
            group.end();
        });

        ui.window("Inference").build(move || {
            // Cropped Camera Feeds

            let draw_cropped_feed = |camera_texture: CameraTexture| {
                imgui::Image::new(
                    camera_texture.get_texture_id(),
                    [FRAME_RESIZE_W as f32, FRAME_RESIZE_H as f32],
                )
                .uv0([
                    1.0 - FRAME_CROP_X as f32 / CAMERA_FRAME_SIZE as f32,
                    FRAME_CROP_Y as f32 / CAMERA_FRAME_SIZE as f32,
                ])
                .uv1([
                    1.0 - (FRAME_CROP_X + FRAME_CROP_W) as f32 / CAMERA_FRAME_SIZE as f32,
                    (FRAME_CROP_Y + FRAME_CROP_H) as f32 / CAMERA_FRAME_SIZE as f32,
                ])
                .build(ui);
            };

            ui.text(format!("Cropped Camera Feeds"));
            let group = ui.begin_group();
            draw_cropped_feed(self.l_texture);
            ui.same_line();
            draw_cropped_feed(self.r_texture);
            group.end();

            // Generic eye state drawer

            let draw_eyelid_state = |state: EyeState| {
                const WIDGET_W: f32 = 10.0;
                const WIDGET_H: f32 = 150.0;

                const COLOR_NORMAL: ImColor32 = ImColor32::from_rgb(0, 148, 255);
                const COLOR_WIDE: ImColor32 = ImColor32::from_rgb(127, 201, 255);

                const SPLIT_POINT: f32 = 0.75;

                let progress = state.eyelid;

                let draw_list = ui.get_window_draw_list();
                let position = ui.cursor_screen_pos();

                let zero_y = position[1] + WIDGET_H;
                let split_y = position[1] + WIDGET_H * (1.0 - progress.min(SPLIT_POINT));
                let one_y = position[1] + WIDGET_H * (1.0 - progress);

                draw_list
                    .add_rect(
                        [position[0], zero_y],
                        [position[0] + WIDGET_W, split_y],
                        COLOR_NORMAL,
                    )
                    .filled(true)
                    .build();
                draw_list
                    .add_rect(
                        [position[0], split_y],
                        [position[0] + WIDGET_W, one_y],
                        COLOR_WIDE,
                    )
                    .filled(true)
                    .build();

                // Advance cursor to avoid overlapping with next UI element
                ui.dummy([WIDGET_W, WIDGET_H]);
            };

            let draw_gaze_state = |state: EyeState| {
                const WIDGET_SIZE: f32 = 150.0;
                const FOV_SIZE: f32 = 0.95;
                const FOV_RANGE: f32 = 90.0;
                const FOV_RANGE_DIV_2: f32 = FOV_RANGE / 2.0;

                const GAZE_RADIUS: f32 = 5.0;

                const COLOR_BACKGROUND: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
                const COLOR_AXES: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
                const COLOR_CIRCLES: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
                const COLOR_GAZE: [f32; 4] = [0.0, 0.1, 0.4, 1.0];

                let position = ui.cursor_screen_pos();
                let size = WIDGET_SIZE;

                let draw_list = ui.get_window_draw_list();

                // Define the center of the drawing area
                let center = [position[0] + size * 0.5, position[1] + size * 0.5];

                // Define square corners
                let top_left = [center[0] - size * 0.5, center[1] - size * 0.5];
                let bottom_right = [center[0] + size * 0.5, center[1] + size * 0.5];

                // Draw white square
                draw_list
                    .add_rect(top_left, bottom_right, COLOR_BACKGROUND)
                    .filled(true)
                    .build();

                // Draw axes
                draw_list
                    .add_line(
                        [center[0], top_left[1]],
                        [center[0], bottom_right[1]],
                        COLOR_AXES,
                    )
                    .build(); // Vertical axis
                draw_list
                    .add_line(
                        [top_left[0], center[1]],
                        [bottom_right[0], center[1]],
                        COLOR_AXES,
                    )
                    .build(); // Horizontal axis

                let max_radius = size * FOV_SIZE / 2.0;
                draw_list
                    .add_circle(center, max_radius, COLOR_CIRCLES)
                    .build();

                for i in (15..FOV_RANGE_DIV_2 as i32).step_by(15) {
                    draw_list
                        .add_circle(
                            center,
                            i as f32 / FOV_RANGE_DIV_2 * max_radius,
                            COLOR_CIRCLES,
                        )
                        .build();
                }

                draw_list
                    .add_circle(
                        [
                            center[0] + state.yaw / FOV_RANGE_DIV_2 * max_radius,
                            center[1] + state.pitch / FOV_RANGE_DIV_2 * max_radius,
                        ],
                        GAZE_RADIUS,
                        COLOR_GAZE,
                    )
                    .filled(true)
                    .build();

                // Advance cursor to avoid overlapping with next UI element
                ui.dummy([size, size]);
            };

            // Raw Eye State

            ui.text(format!("Raw Eye State"));
            let group = ui.begin_group();
            draw_eyelid_state(self.l_raw_eye);
            ui.same_line();
            draw_gaze_state(self.l_raw_eye);
            ui.same_line();
            draw_gaze_state(self.r_raw_eye);
            ui.same_line();
            draw_eyelid_state(self.r_raw_eye);
            group.end();

            // Filtered Eye State

            ui.text(format!("Filtered Eye State"));
            let group = ui.begin_group();
            draw_eyelid_state(self.filtered_eyes.0);
            ui.same_line();
            draw_gaze_state(self.filtered_eyes.0);
            ui.same_line();
            draw_gaze_state(self.filtered_eyes.1);
            ui.same_line();
            draw_eyelid_state(self.filtered_eyes.1);
            group.end();
        });
    }
}
