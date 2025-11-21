use openxr as xr;

#[derive(Debug, Clone, Copy)]
pub struct QuadDesc {
    pub pose: xr::Posef, // quad pose in VIEW space
    pub size: xr::Extent2Df,
}

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub dir: xr::Vector3f,
    /// MUST be normalized
    pub origin: xr::Vector3f,
}

fn dot(a: xr::Vector3f, b: xr::Vector3f) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn sub(a: xr::Vector3f, b: xr::Vector3f) -> xr::Vector3f {
    xr::Vector3f {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

fn add(a: xr::Vector3f, b: xr::Vector3f) -> xr::Vector3f {
    xr::Vector3f {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

fn mul(a: xr::Vector3f, f: f32) -> xr::Vector3f {
    xr::Vector3f {
        x: a.x * f,
        y: a.y * f,
        z: a.z * f,
    }
}

/// rotate vector by quaternion
fn qrot(q: xr::Quaternionf, v: xr::Vector3f) -> xr::Vector3f {
    let u = xr::Vector3f {
        x: q.x,
        y: q.y,
        z: q.z,
    };
    let s = q.w;

    // v' = 2*dot(u,v)*u + (s*s - dot(u,u))*v + 2*s*cross(u,v)
    let uv = xr::Vector3f {
        x: u.y * v.z - u.z * v.y,
        y: u.z * v.x - u.x * v.z,
        z: u.x * v.y - u.y * v.x,
    };
    let uuv = xr::Vector3f {
        x: u.y * uv.z - u.z * uv.y,
        y: u.z * uv.x - u.x * uv.z,
        z: u.x * uv.y - u.y * uv.x,
    };

    add(add(mul(uv, 2.0 * s), mul(uuv, 2.0)), v)
}

/// Main function: intersect ray with OpenXR quad layer.
/// Returns Some((pixel_x, pixel_y)) if hit inside the quad.
pub fn ray_intersect_quad(ray: Ray, quad: QuadDesc) -> Option<(f32, f32)> {
    // --- 1. Build quad basis in view space ---
    let ori = quad.pose.orientation;

    let right = qrot(
        ori,
        xr::Vector3f {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
    );
    let up = qrot(
        ori,
        xr::Vector3f {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
    );
    let normal = qrot(
        ori,
        xr::Vector3f {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
    ); // viewer-facing

    let center = quad.pose.position;

    // --- 2. Ray-plane intersection ---
    let denom = dot(normal, ray.dir);
    if denom.abs() < 1e-5 {
        return None; // parallel
    }

    let t = dot(sub(center, ray.origin), normal) / denom;
    if t < 0.0 {
        return None; // behind ray
    }

    let hit = add(ray.origin, mul(ray.dir, t));

    // --- 3. Project hit point into quad local coordinates ---
    let rel = sub(hit, center);

    let local_x = dot(rel, right);
    let local_y = dot(rel, up);

    // Quad extends from -width/2..+width/2 and same for height
    if local_x.abs() > quad.size.width * 0.5 || local_y.abs() > quad.size.height * 0.5 {
        return None; // outside quad bounds
    }

    // --- 4. Convert to UV (0..1 range) ---
    let u = (local_x / quad.size.width) + 0.5;
    let v = (local_y / quad.size.height) + 0.5;

    Some((u, v))
}
