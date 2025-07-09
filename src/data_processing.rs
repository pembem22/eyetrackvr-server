use std::time::SystemTime;

use async_broadcast::{Receiver, Sender};
use tokio::task::JoinHandle;

use one_euro_rs::OneEuroFilter;
use tokio_stream::StreamExt;

use crate::Eye;
use crate::inference::EyeState;

pub fn filter_eye(mut rx: Receiver<EyeState>, tx: Sender<EyeState>) -> JoinHandle<()> {
    tokio::spawn(async move {
        const PY_BETA: f32 = 0.3;
        const PY_FCMIN: f32 = 0.5;
        let mut filter_pitch = OneEuroFilter::new(0.0, PY_FCMIN, PY_FCMIN, PY_BETA);
        let mut filter_yaw = OneEuroFilter::new(0.0, PY_FCMIN, PY_FCMIN, PY_BETA);

        const EL_BETA: f32 = 1.0;
        const EL_FCMIN: f32 = 3.0;
        let mut filter_eyelid = OneEuroFilter::new(0.0, EL_FCMIN, EL_FCMIN, EL_BETA);

        let last_timestamp = SystemTime::now();

        while let Ok(eye) = rx.recv_direct().await {
            let filter_secs = eye
                .timestamp
                .duration_since(last_timestamp)
                .unwrap()
                .as_secs_f32();

            let pitch = filter_pitch.filter_with_delta(eye.pitch, filter_secs);
            let yaw = filter_yaw.filter_with_delta(eye.yaw, filter_secs);
            let eyelid = filter_eyelid.filter_with_delta(eye.eyelid, filter_secs);

            tx.broadcast_direct(EyeState {
                pitch,
                yaw,
                eyelid,
                timestamp: eye.timestamp,
            })
            .await
            .unwrap();
        }
    })
}

pub fn merge_eyes(
    l_rx: Receiver<EyeState>,
    r_rx: Receiver<EyeState>,
    tx: Sender<(EyeState, EyeState)>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let l_rx = l_rx.map(|es| (Eye::L, es));
        let r_rx = r_rx.map(|es| (Eye::R, es));

        let mut eyes_rx = l_rx.merge(r_rx);

        let mut l_eye_state: EyeState = Default::default();
        let mut r_eye_state: EyeState = Default::default();

        while let Some((eye, state)) = eyes_rx.next().await {
            println!("{eye:?} {state:?}");

            match eye {
                Eye::L => l_eye_state = state,
                Eye::R => r_eye_state = state,
            };

            // Average pitch.
            {
                let avg_pitch = (l_eye_state.pitch + r_eye_state.pitch) / 2.0;
                l_eye_state.pitch = avg_pitch;
                r_eye_state.pitch = avg_pitch;
            }

            // Makes the result jittery, probably cause of not synchronous data from L and R cams.
            /*
            // Clamp convergence point at infinity.
            if l_eye_state.yaw < r_eye_state.yaw {
                let avg_yaw = (l_eye_state.yaw + l_eye_state.yaw) / 2.0;
                l_eye_state.yaw = avg_yaw;
                r_eye_state.yaw = avg_yaw;
            }

            // Clamp converge point up close.
            {
                // TODO: Make IPD configurable.
                const IPD: f32 = 0.063; // Median IPD 63mm
                const MIN_FOCUS_DIST: f32 = 0.03; // 3cm
                let max_angle: f32 = f32::atan(IPD / (2.0 * MIN_FOCUS_DIST));
                if (l_eye_state.yaw - r_eye_state.yaw).abs() > 2.0 * max_angle {
                    // Clamp both yaws symmetrically around their average.
                    let avg_yaw = (l_eye_state.yaw + l_eye_state.yaw) / 2.0;
                    l_eye_state.yaw = avg_yaw - max_angle;
                    r_eye_state.yaw = avg_yaw + max_angle;
                }
            }
            */

            tx.broadcast_direct((l_eye_state, r_eye_state))
                .await
                .unwrap();
        }
    })
}
