//! The Fontabulous logo (from the person's SVG): two filled outlines, a rounded
//! diagonal bar and a rounded hook, in the app accent. Pure geometry plus a small
//! scanline rasteriser so build.rs can make the program icon from it too.

#![allow(dead_code)]

/// SVG viewBox size.
pub const VIEW_W: f32 = 26.229;
pub const VIEW_H: f32 = 34.284;
/// The app's accent (reddish-orange used for selections).
pub const COLOR: [u8; 3] = [0xe8, 0x57, 0x3f];

type P = [f32; 2];

enum Cmd {
    L(P),
    C(P, P, P),
}

fn flatten(start: P, cmds: &[Cmd]) -> Vec<P> {
    let mut out = vec![start];
    let mut cur = start;
    for c in cmds {
        match c {
            Cmd::L(p) => {
                out.push(*p);
                cur = *p;
            }
            Cmd::C(p1, p2, p3) => {
                let steps = 16;
                for i in 1..=steps {
                    let t = i as f32 / steps as f32;
                    let u = 1.0 - t;
                    let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    out.push([
                        a * cur[0] + b * p1[0] + cc * p2[0] + d * p3[0],
                        a * cur[1] + b * p1[1] + cc * p2[1] + d * p3[1],
                    ]);
                }
                cur = *p3;
            }
        }
    }
    out
}

/// Closed outlines in viewBox units.
pub fn polygons() -> Vec<Vec<P>> {
    use Cmd::*;
    let bar = flatten(
        [15.021, 0.879],
        &[
            C([16.192, -0.293], [18.092, -0.293], [19.263, 0.879]),
            C([20.435, 2.05], [20.435, 3.95], [19.263, 5.121]),
            L([5.121, 19.263]),
            C([3.95, 20.435], [2.05, 20.435], [0.879, 19.263]),
            C([-0.293, 18.092], [-0.293, 16.192], [0.879, 15.021]),
        ],
    );
    let hook = flatten(
        [21.108, 6.966],
        &[
            C([22.279, 5.794], [24.179, 5.794], [25.35, 6.966]),
            C([26.522, 8.137], [26.522, 10.037], [25.35, 11.208]),
            L([14.446, 22.112]),
            C([13.864, 22.694], [14.01, 23.91], [14.862, 24.762]),
            L([19.263, 29.163]),
            C([20.435, 30.335], [20.435, 32.234], [19.263, 33.406]),
            C([18.092, 34.577], [16.192, 34.577], [15.021, 33.406]),
            L([10.62, 29.004]),
            C([7.43, 25.815], [7.244, 20.83], [10.204, 17.87]),
        ],
    );
    vec![bar, hook]
}

/// Anti-aliased coverage (0-255) of the logo centred in a `w`×`h` image with
/// `pad` (fraction of the size) around it.
pub fn mask(w: u32, h: u32, pad: f32) -> Vec<u8> {
    let (wf, hf) = (w as f32, h as f32);
    let scale = ((wf * (1.0 - 2.0 * pad)) / VIEW_W).min((hf * (1.0 - 2.0 * pad)) / VIEW_H);
    let ox = (wf - VIEW_W * scale) / 2.0;
    let oy = (hf - VIEW_H * scale) / 2.0;
    let polys: Vec<Vec<P>> =
        polygons().into_iter().map(|p| p.into_iter().map(|q| [ox + q[0] * scale, oy + q[1] * scale]).collect()).collect();
    const SS: usize = 5;
    let (wn, hn) = (w as usize, h as usize);
    let mut cov = vec![0f32; wn * hn];
    let mut xs: Vec<(f32, i32)> = Vec::new();
    for y in 0..hn {
        for s in 0..SS {
            let yy = y as f32 + (s as f32 + 0.5) / SS as f32;
            xs.clear();
            for poly in &polys {
                for i in 0..poly.len() {
                    let a = poly[i];
                    let b = poly[(i + 1) % poly.len()];
                    if (a[1] <= yy && yy < b[1]) || (b[1] <= yy && yy < a[1]) {
                        let t = (yy - a[1]) / (b[1] - a[1]);
                        xs.push((a[0] + t * (b[0] - a[0]), if b[1] > a[1] { 1 } else { -1 }));
                    }
                }
            }
            xs.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut wind = 0;
            let mut start = 0.0;
            for &(x, d) in &xs {
                let before = wind;
                wind += d;
                if before == 0 && wind != 0 {
                    start = x;
                } else if before != 0 && wind == 0 {
                    add_span(&mut cov[y * wn..(y + 1) * wn], start, x, 1.0 / SS as f32);
                }
            }
        }
    }
    cov.into_iter().map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
}

fn add_span(row: &mut [f32], x0: f32, x1: f32, weight: f32) {
    let n = row.len() as f32;
    let (x0, x1) = (x0.clamp(0.0, n), x1.clamp(0.0, n));
    if x1 <= x0 {
        return;
    }
    let (i0, i1) = (x0.floor() as usize, x1.floor() as usize);
    if i0 == i1 {
        row[i0.min(row.len() - 1)] += (x1 - x0) * weight;
        return;
    }
    row[i0] += (i0 as f32 + 1.0 - x0) * weight;
    for v in row.iter_mut().take(i1).skip(i0 + 1) {
        *v += weight;
    }
    if i1 < row.len() {
        row[i1] += (x1 - i1 as f32) * weight;
    }
}

/// Square RGBA image of the logo (accent colour) on a transparent background.
pub fn rasterize(size: u32) -> Vec<u8> {
    let m = mask(size, size, 0.04);
    let mut rgba = Vec::with_capacity(m.len() * 4);
    for a in m {
        rgba.extend_from_slice(&[COLOR[0], COLOR[1], COLOR[2], a]);
    }
    rgba
}
