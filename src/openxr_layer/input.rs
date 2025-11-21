use log::debug;
use openxr::{self as xr, Vector3f};
use openxr_sys as xr_sys;
use quaternion_core as quat;
use std::time::{Duration, Instant};

use crate::openxr_layer::raycast::{QuadDesc, Ray, ray_intersect_quad};

const CLICK_THRESHOLD: f32 = 0.8;

/// Which hand currently acts as the UI cursor
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ActiveHand {
    Left,
    Right,
}

/// Per-frame derived input values
#[derive(Copy, Clone, Debug)]
pub struct HandFrameState {
    pub click: bool,
    pub grip: bool,
    pub pose: Option<xr::Posef>,
}

/// Internal state for click transitions, gesture detection, etc.
#[derive(Debug)]
pub struct InputState {
    pub active_hand: ActiveHand,

    pub last_click_down: [bool; 2],
    pub last_grip_down: [bool; 2],

    // For double-tap detection:
    pub last_combo_time: [Option<Instant>; 2], // per hand
    pub combo_count: [u8; 2],                  // per hand
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            active_hand: ActiveHand::Right, // default
            last_click_down: [false; 2],
            last_grip_down: [false; 2],
            last_combo_time: [None, None],
            combo_count: [0, 0],
        }
    }
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    PointerMove { x: f32, y: f32 },
    PointerButton { down: bool },
}

pub struct Inputs {
    pub action_set: xr::ActionSet,
    pub lr_subactions: [xr::Path; 2],
    pub hand_spaces: [xr::Space; 2],

    pub click_action: xr::Action<f32>,
    pub grip_action: xr::Action<f32>,
    pub pose_action: xr::Action<xr_sys::Posef>,
    pub vibrate_action: xr::Action<xr::Haptic>,

    pub input_state: InputState,

    pub events: Vec<UiEvent>,
}

impl Inputs {
    pub fn process_inputs<G>(
        &mut self,
        session: &xr::Session<G>,
        quad_desc: QuadDesc,
        view_space: &xr::Space,
        predicted_display_time: xr::Time,
    ) {
        let now = Instant::now();

        // --- 0. Clear the event queue. ---

        self.events.clear();

        // --- 1. Query runtime action state ---

        session
            .sync_actions(&[xr::ActiveActionSet::new(&self.action_set)])
            .unwrap();

        // for subaction in self.lr_subactions {
        //     debug!(
        //         "current interaction profile: {}",
        //         session
        //             .instance()
        //             .path_to_string(session.current_interaction_profile(subaction).unwrap())
        //             .unwrap()
        //     );
        // }

        let mut hands = [HandFrameState {
            click: false,
            grip: false,
            pose: None,
        }; 2];

        for hand_i in 0..2 {
            let sub_path = self.lr_subactions[hand_i];

            // Trigger → click
            let click_state = self
                .click_action
                .state(session, sub_path)
                .expect("click state");
            hands[hand_i].click = click_state.current_state > CLICK_THRESHOLD;

            // Grip
            let grip_state = self
                .grip_action
                .state(session, sub_path)
                .expect("grip state");
            hands[hand_i].grip = grip_state.current_state > CLICK_THRESHOLD;

            // Pose
            let pose_active = self.pose_action.is_active(session, sub_path);

            // debug!("{:#?}", click_state);
            // debug!("{:#?}", grip_state);
            // debug!("{:#?}", pose_active);

            // Pose (aim pose)
            let pose_value = self.hand_spaces[hand_i]
                .locate(view_space, predicted_display_time)
                .expect("pose state");
            // debug!("{:#?} {:#?}", pose_value.location_flags, pose_value.pose);
            if pose_value.location_flags.contains(
                xr::SpaceLocationFlags::POSITION_VALID | xr::SpaceLocationFlags::ORIENTATION_VALID,
            ) {
                hands[hand_i].pose = Some(pose_value.pose);
            }
        }

        // --- 2. Cursor activation logic ---

        for hand_i in 0..2 {
            let click_down = hands[hand_i].click;
            let last_click = self.input_state.last_click_down[hand_i];

            let is_active = match self.input_state.active_hand {
                ActiveHand::Left => hand_i == 0,
                ActiveHand::Right => hand_i == 1,
            };

            // If inactive controller clicked → becomes active cursor
            if click_down && !last_click && !is_active {
                self.input_state.active_hand = match hand_i {
                    0 => ActiveHand::Left,
                    _ => ActiveHand::Right,
                };
            }
        }

        // --- 3. Handle cursor movement ---
        {
            let active = match self.input_state.active_hand {
                ActiveHand::Left => 0,
                ActiveHand::Right => 1,
            };

            if let Some(pose) = hands[active].pose {
                // Pointer direction = -Z of aim pose.
                let forward = {
                    let mut ori = pose.orientation;
                    // transform local -Z
                    ori.z *= -1.0;
                    ori
                };

                let ray_dir =
                    quat::to_rotation_vector((forward.w, [forward.x, forward.y, forward.z]));
                let ray_dir = Vector3f {
                    x: ray_dir[0],
                    y: ray_dir[1],
                    z: ray_dir[2],
                };

                if let Some((x, y)) = ray_intersect_quad(
                    Ray {
                        dir: ray_dir,
                        origin: pose.position,
                    },
                    quad_desc,
                ) {
                    self.events.push(UiEvent::PointerMove { x, y });
                }
            }
        }

        // --- 4. Detect click transitions ---
        for hand_i in 0..2 {
            let now_click = hands[hand_i].click;
            let last_click = self.input_state.last_click_down[hand_i];

            if now_click && !last_click {
                // CLICK DOWN EVENT
                // trigger UI click if this is the active hand
                if matches!(
                    (self.input_state.active_hand, hand_i),
                    (ActiveHand::Left, 0) | (ActiveHand::Right, 1)
                ) {
                    self.events.push(UiEvent::PointerButton { down: true });
                }
            }

            if !now_click && last_click {
                // CLICK UP EVENT
                self.events.push(UiEvent::PointerButton { down: false });
            }

            self.input_state.last_click_down[hand_i] = now_click;
        }

        // --- 5. Special gesture detection: double trigger+grip combo ---
        //
        // Condition: trigger AND grip pressed simultaneously,
        // twice within 180 ms ± small tolerance.

        const COMBO_WINDOW: Duration = Duration::from_millis(180);
        const COMBO_TOLERANCE: Duration = Duration::from_millis(40);

        for hand_i in 0..2 {
            let combo_down = hands[hand_i].click && hands[hand_i].grip;
            let last_combo_down =
                self.input_state.last_grip_down[hand_i] && self.input_state.last_click_down[hand_i];

            if combo_down && !last_combo_down {
                // combo pressed NOW
                match self.input_state.last_combo_time[hand_i] {
                    None => {
                        // first press
                        self.input_state.last_combo_time[hand_i] = Some(now);
                        self.input_state.combo_count[hand_i] = 1;
                    }
                    Some(prev_time) => {
                        let elapsed = now.saturating_duration_since(prev_time);

                        if elapsed <= COMBO_WINDOW + COMBO_TOLERANCE {
                            // double-tap recognized
                            self.input_state.combo_count[hand_i] += 1;
                            if self.input_state.combo_count[hand_i] >= 2 {
                                // FIRE SPECIAL GESTURE
                                // special_gesture_triggered();
                                self.input_state.combo_count[hand_i] = 0;
                                self.input_state.last_combo_time[hand_i] = None;
                            }
                        } else {
                            // too slow → restart
                            self.input_state.combo_count[hand_i] = 1;
                            self.input_state.last_combo_time[hand_i] = Some(now);
                        }
                    }
                }
            }

            self.input_state.last_grip_down[hand_i] = hands[hand_i].grip;
            self.input_state.last_click_down[hand_i] = hands[hand_i].click;
        }

        // debug!("{:#?}", hands);
        // debug!("{:#?}", self.input_state);
    }
}
