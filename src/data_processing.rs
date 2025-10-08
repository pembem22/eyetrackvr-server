use std::time::Duration;

use async_broadcast::{Receiver, RecvError, Sender};
use log::{error, warn};
use tokio::task::JoinHandle;

use crate::structs::{CombinedEyeGazeState, Eye, EyeGazeState, EyesGazeState, ZERO_TIMESTAMP};

const EYE_TIMEOUT: Duration = Duration::from_millis(50);

pub fn process_gaze(
    mut rx: Receiver<EyesGazeState>,
    tx: Sender<CombinedEyeGazeState>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // const PY_BETA: f32 = 0.3;
        // const PY_FCMIN: f32 = 0.5;
        // let mut filter_pitch = OneEuroFilter::new(0.0, PY_FCMIN, PY_FCMIN, PY_BETA);
        // let mut filter_yaw = OneEuroFilter::new(0.0, PY_FCMIN, PY_FCMIN, PY_BETA);

        // const EL_BETA: f32 = 1.0;
        // const EL_FCMIN: f32 = 3.0;
        // let mut filter_eyelid = OneEuroFilter::new(0.0, EL_FCMIN, EL_FCMIN, EL_BETA);

        // let last_timestamp = SystemTime::now();

        let mut l_state = EyeGazeState::default();
        let mut l_time = ZERO_TIMESTAMP;

        let mut r_state = EyeGazeState::default();
        let mut r_time = ZERO_TIMESTAMP;

        loop {
            let eyes_gaze = loop {
                match rx.recv_direct().await {
                    Ok(eyes_frame) => break eyes_frame,
                    Err(e) => match e {
                        RecvError::Overflowed(skipped) => {
                            warn!("Skipped {skipped} frames");
                            continue;
                        }
                        RecvError::Closed => {
                            error!("Channel closed");
                            return;
                        }
                    },
                }
            };

            match eyes_gaze {
                EyesGazeState::Both {
                    l_state: new_l_state,
                    r_state: new_r_state,
                    timestamp,
                } => {
                    l_state = new_l_state;
                    r_state = new_r_state;

                    l_time = timestamp;
                    r_time = timestamp;
                }
                EyesGazeState::Mono {
                    eye,
                    state,
                    timestamp,
                } => match eye {
                    Eye::L => {
                        l_state = state;
                        l_time = timestamp;
                    }
                    Eye::R => {
                        r_state = state;
                        r_time = timestamp;
                    }
                },
            };

            let combined_gaze = 'combined: {
                // Left eye has timed out.
                if r_time > l_time && r_time.duration_since(l_time).unwrap() > EYE_TIMEOUT {
                    break 'combined CombinedEyeGazeState {
                        pitch: r_state.pitch,
                        l_yaw: r_state.yaw,
                        r_yaw: r_state.yaw,
                        l_eyelid: r_state.eyelid,
                        r_eyelid: r_state.eyelid,

                        gaze_pitch: r_state.pitch,
                        gaze_yaw: r_state.yaw,

                        timestamp: r_time,
                    };
                }

                // Right eye has timed out.
                if l_time > r_time && l_time.duration_since(r_time).unwrap() > EYE_TIMEOUT {
                    break 'combined CombinedEyeGazeState {
                        pitch: l_state.pitch,
                        l_yaw: l_state.yaw,
                        r_yaw: l_state.yaw,
                        l_eyelid: l_state.eyelid,
                        r_eyelid: l_state.eyelid,

                        gaze_pitch: l_state.pitch,
                        gaze_yaw: l_state.yaw,

                        timestamp: l_time,
                    };
                }

                // Left eye is behind.
                if l_time < r_time {
                    // TODO: apply prediction.
                }
                // Right eye is behind.
                if r_time < l_time {
                    // TODO: apply prediction.
                }

                let timestamp = std::cmp::max(l_time, r_time);

                let avg_pitch = (l_state.pitch + r_state.pitch) / 2.0;

                let avg_yaw = (l_state.yaw + r_state.yaw) / 2.0;

                // TODO: this is basically convergence distance, smooth it.
                let yaw_diff = l_state.yaw - r_state.yaw;
                // Slightly nudge the eyes together. Otherwise Steam Link refuses
                // to use eye tracking from the `XR_FB_eye_tracking_social` extension.
                // Probably tries to calculate convergence distance.
                let yaw_diff = yaw_diff.max(0.05);

                let l_yaw = avg_yaw + yaw_diff / 2.0;
                let r_yaw = avg_yaw - yaw_diff / 2.0;

                // let pitch = filter_pitch.filter_with_delta(eye.pitch, filter_secs);
                // let yaw = filter_yaw.filter_with_delta(eye.yaw, filter_secs);
                // let eyelid = filter_eyelid.filter_with_delta(eye.eyelid, filter_secs);

                CombinedEyeGazeState {
                    pitch: avg_pitch,
                    l_yaw,
                    r_yaw,
                    l_eyelid: l_state.eyelid,
                    r_eyelid: r_state.eyelid,

                    gaze_pitch: avg_pitch,
                    gaze_yaw: avg_yaw,

                    timestamp,
                }
            };

            // println!("{:#?}", combined_gaze);

            tx.broadcast_direct(combined_gaze).await.unwrap();
        }
    })
}

// pub fn filter_eye(mut rx: Receiver<EyesGazeState>, tx: Sender<EyesGazeState>) -> JoinHandle<()> {
//     tokio::spawn(async move {
//         const PY_BETA: f32 = 0.3;
//         const PY_FCMIN: f32 = 0.5;
//         let mut filter_pitch = OneEuroFilter::new(0.0, PY_FCMIN, PY_FCMIN, PY_BETA);
//         let mut filter_yaw = OneEuroFilter::new(0.0, PY_FCMIN, PY_FCMIN, PY_BETA);

//         const EL_BETA: f32 = 1.0;
//         const EL_FCMIN: f32 = 3.0;
//         let mut filter_eyelid = OneEuroFilter::new(0.0, EL_FCMIN, EL_FCMIN, EL_BETA);

//         let last_timestamp = SystemTime::now();

//         while let Ok(eye) = rx.recv_direct().await {
//             let filter_secs = eye
//                 .timestamp
//                 .duration_since(last_timestamp)
//                 .unwrap()
//                 .as_secs_f32();

//             let pitch = filter_pitch.filter_with_delta(eye.pitch, filter_secs);
//             let yaw = filter_yaw.filter_with_delta(eye.yaw, filter_secs);
//             let eyelid = filter_eyelid.filter_with_delta(eye.eyelid, filter_secs);

//             tx.broadcast_direct(EyesGazeState {
//                 pitch,
//                 yaw,
//                 eyelid,
//                 timestamp: eye.timestamp,
//             })
//             .await
//             .unwrap();
//         }
//     })
// }

// pub fn merge_eyes(
//     l_rx: Receiver<EyesGazeState>,
//     r_rx: Receiver<EyesGazeState>,
//     tx: Sender<(EyesGazeState, EyesGazeState)>,
// ) -> JoinHandle<()> {
//     tokio::spawn(async move {
//         let l_rx = l_rx.map(|es| (Eye::L, es));
//         let r_rx = r_rx.map(|es| (Eye::R, es));

//         let mut eyes_rx = l_rx.merge(r_rx);

//         let mut l_eye_state: EyesGazeState = Default::default();
//         let mut r_eye_state: EyesGazeState = Default::default();

//         while let Some((eye, state)) = eyes_rx.next().await {
//             println!("{eye:?} {state:?}");

//             match eye {
//                 Eye::L => l_eye_state = state,
//                 Eye::R => r_eye_state = state,
//             };

//             // Create copies to not modify original values.
//             let mut l_eye_state = l_eye_state;
//             let mut r_eye_state = r_eye_state;

//             // Average pitch.
//             {
//                 let avg_pitch = (l_eye_state.pitch + r_eye_state.pitch) / 2.0;
//                 l_eye_state.pitch = avg_pitch;
//                 r_eye_state.pitch = avg_pitch;
//             }

//             // // Makes the result jittery, probably cause of not synchronous data from L and R cams.

//             // Clamp convergence point at infinity.
//             if l_eye_state.yaw < r_eye_state.yaw {
//                 let avg_yaw = (l_eye_state.yaw + r_eye_state.yaw) / 2.0;
//                 l_eye_state.yaw = avg_yaw;
//                 r_eye_state.yaw = avg_yaw;
//             }

//             // // Clamp converge point up close.
//             // {
//             //     // TODO: Make IPD configurable.
//             //     const IPD: f32 = 0.063; // Median IPD 63mm
//             //     const MIN_FOCUS_DIST: f32 = 0.03; // 3cm
//             //     let max_angle: f32 = f32::atan(IPD / (2.0 * MIN_FOCUS_DIST));
//             //     if (l_eye_state.yaw - r_eye_state.yaw).abs() > 2.0 * max_angle {
//             //         // Clamp both yaws symmetrically around their average.
//             //         let avg_yaw = (l_eye_state.yaw + r_eye_state.yaw) / 2.0;
//             //         l_eye_state.yaw = avg_yaw - max_angle;
//             //         r_eye_state.yaw = avg_yaw + max_angle;
//             //     }
//             // }

//             tx.broadcast_direct((l_eye_state, r_eye_state))
//                 .await
//                 .unwrap();
//         }
//     })
// }
