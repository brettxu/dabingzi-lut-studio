//! lut-core: 颜色分级 LUT 核心引擎
//!
//! 从 lut-studio/index.html 的前端 JS 算法逐行移植而来。
//! 所有计算使用 f64 以与 JS number 保持一致的运算精度，仅在存储时降为 f32
//! （对应 JS 中的 Float32Array）。目标：逐像素输出与 JS 版完全一致。

/// 调色参数集（对应 index.html 的 getParams()）
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    pub exposure: f64,
    pub contrast: f64,
    pub highlight: f64,
    pub shadow: f64,
    pub whites: f64,
    pub blacks: f64,
    pub texture: f64,
    pub clarity: f64,
    pub dehaze: f64,
    pub vibrance: f64,
    pub saturation: f64,
    pub color_temp: f64,
    pub color_tint: f64,
    pub shadow_temp: f64,
    pub highlight_temp: f64,
    pub shadow_tone: f64,
    pub highlight_tone: f64,
    pub hue_red: f64,
    pub sat_red: f64,
    pub hue_green: f64,
    pub sat_green: f64,
    pub hue_blue: f64,
    pub sat_blue: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            exposure: 0.0, contrast: 0.0, highlight: 0.0, shadow: 0.0,
            whites: 0.0, blacks: 0.0, texture: 0.0, clarity: 0.0, dehaze: 0.0,
            vibrance: 0.0, saturation: 0.0, color_temp: 0.0, color_tint: 0.0,
            shadow_temp: 0.0, highlight_temp: 0.0, shadow_tone: 0.0, highlight_tone: 0.0,
            hue_red: 0.0, sat_red: 0.0, hue_green: 0.0, sat_green: 0.0,
            hue_blue: 0.0, sat_blue: 0.0,
        }
    }
}

/// 参考图匹配数据（对应 index.html 的 matchStats）
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchStats {
    pub strength: f64,
    pub src_mean: [f64; 3],
    pub src_std: [f64; 3],
    pub ref_mean: [f64; 3],
    pub ref_std: [f64; 3],
}

/// 缩略图均值/标准差统计（对应 computeStats 的数学核心）
pub fn compute_stats(rgba: &[u8]) -> ([f64; 3], [f64; 3]) {
    let n = rgba.len() / 4;
    let mut sr = 0.0f64; let mut sg = 0.0f64; let mut sb = 0.0f64;
    for i in 0..rgba.len() / 4 {
        sr += rgba[i * 4] as f64;
        sg += rgba[i * 4 + 1] as f64;
        sb += rgba[i * 4 + 2] as f64;
    }
    let mr = sr / n as f64; let mg = sg / n as f64; let mb = sb / n as f64;
    let mut vr = 0.0f64; let mut vg = 0.0f64; let mut vb = 0.0f64;
    for i in 0..rgba.len() / 4 {
        let dr = rgba[i * 4] as f64 - mr; vr += dr * dr;
        let dg = rgba[i * 4 + 1] as f64 - mg; vg += dg * dg;
        let db = rgba[i * 4 + 2] as f64 - mb; vb += db * db;
    }
    ([mr, mg, mb], [(vr / n as f64).sqrt(), (vg / n as f64).sqrt(), (vb / n as f64).sqrt()])
}

pub fn clamp255(v: f64) -> f64 {
    if v < 0.0 { 0.0 } else if v > 255.0 { 255.0 } else { v }
}

/// 复刻 JS Math.max / Math.min 的 NaN 语义：
/// 只要任一操作数为 NaN，结果即为 NaN（Rust 原生 max/min 会返回另一操作数，需替换）。
fn jmax(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else if a > b { a } else { b }
}
fn jmin(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else if a < b { a } else { b }
}

pub fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = jmax(0.0, jmin(1.0, (x - a) / (b - a)));
    t * t * (3.0 - 2.0 * t)
}

/// 亮度曲线（256 点，对应 buildLumCurve，存 f32 = JS Float32Array）
pub fn build_lum_curve(p: &Params) -> Vec<f32> {
    let ev = 2f64.powf(p.exposure);
    let c = 1.0 + p.contrast / 100.0;
    let cl = 1.0 + (p.clarity / 100.0) * 0.55;
    let tx = 1.0 + (p.texture / 100.0) * 0.28;
    let dh = p.dehaze / 100.0;
    let mut curve = vec![0f32; 256];
    for i in 0..256 {
        let mut l = (i as f64) / 255.0;
        l *= ev;
        l = 0.5 + (l - 0.5) * c;
        l = 0.5 + (l - 0.5) * cl;
        l = 0.5 + (l - 0.5) * tx;
        l += dh * 0.08 * (l - 0.5);
        let hlm = smoothstep(0.55, 1.0, l);
        let shm = 1.0 - smoothstep(0.0, 0.55, l);
        let wm = smoothstep(0.82, 1.0, l);
        let km = 1.0 - smoothstep(0.0, 0.18, l);
        l += (p.highlight / 100.0) * 0.34 * hlm;
        l += (p.shadow / 100.0) * 0.34 * shm;
        l += (p.whites / 100.0) * 0.30 * wm;
        l += (p.blacks / 100.0) * 0.30 * km;
        l = if l < 0.0 { 0.0 } else if l > 1.0 { 1.0 } else { l }; // JS 三元，NaN 保留
        curve[i] = l as f32;
    }
    curve
}

pub fn rgb2hsl(r: f64, g: f64, b: f64) -> [f64; 3] {
    let r = r / 255.0; let g = g / 255.0; let b = b / 255.0;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let l = (mx + mn) / 2.0;
    if mx == mn { return [0.0, 0.0, l]; }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h;
    if mx == r {
        h = (g - b) / d + if g < b { 6.0 } else { 0.0 };
    } else if mx == g {
        h = (b - r) / d + 2.0;
    } else {
        h = (r - g) / d + 4.0;
    }
    [h / 6.0, s, l]
}

pub fn hsl2rgb(h: f64, s: f64, l: f64) -> [f64; 3] {
    if s == 0.0 {
        let v = (l * 255.0).round();
        return [v, v, v];
    }
    let h = (h % 1.0 + 1.0) % 1.0;
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |t: f64| -> f64 {
        let t = (t % 1.0 + 1.0) % 1.0;
        if t < 1.0 / 6.0 { p + (q - p) * 6.0 * t }
        else if t < 0.5 { q }
        else if t < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - t) * 6.0 }
        else { p }
    };
    [
        (f(h + 1.0 / 3.0) * 255.0).round(),
        (f(h) * 255.0).round(),
        (f(h - 1.0 / 3.0) * 255.0).round(),
    ]
}

pub fn hue_weight(h: f64, center: f64, width_deg: f64) -> f64 {
    let c = center / 360.0;
    let w = width_deg / 360.0;
    let mut d = (h - c).abs();
    if d > 0.5 { d = 1.0 - d; }
    jmax(0.0, 1.0 - d / w)
}

/// 单像素调色（对应 gradeColor）
pub fn grade_color(r: f64, g: f64, b: f64, p: &Params, curve: &[f32], m: Option<&MatchStats>) -> [f64; 3] {
    let mut rf = r; let mut gf = g; let mut bf = b;
    if let Some(mm) = m {
        if mm.strength > 0.0 {
            let s = mm.strength;
            let c_arr = [rf, gf, bf];
            let mut out = [0.0f64; 3];
            for c in 0..3 {
                let v = c_arr[c];
                let matched = mm.src_mean[c]
                    + (v - mm.src_mean[c]) * (mm.ref_std[c] / (mm.src_std[c] + 1e-6))
                    + (mm.ref_mean[c] - mm.src_mean[c]);
                out[c] = v * (1.0 - s) + matched * s;
            }
            rf = out[0]; gf = out[1]; bf = out[2];
        }
    }
    let temp = p.color_temp / 100.0;
    let tint = p.color_tint / 100.0;
    rf += temp * 26.0; bf -= temp * 26.0;
    gf -= tint * 22.0; rf += tint * 8.0; bf += tint * 8.0;
    let lcur = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;
    let s_t = p.shadow_temp / 100.0;
    let h_t = p.highlight_temp / 100.0;
    if s_t != 0.0 && lcur < 128.0 {
        let w = (128.0 - lcur) / 128.0;
        let d = s_t * 26.0 * w;
        rf += d; bf -= d;
    }
    if h_t != 0.0 && lcur > 128.0 {
        let w = (lcur - 128.0) / 128.0;
        let d = h_t * 26.0 * w;
        rf += d; bf -= d;
    }
    let li = jmax(0.0, jmin(255.0, 0.2126 * rf + 0.7152 * gf + 0.0722 * bf)).round() as usize;
    let new_l = curve[li] as f64;
    let k = (new_l + 0.001) / (li as f64 / 255.0 + 0.001);
    rf *= k; gf *= k; bf *= k;
    let l2 = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;
    let mx = rf.max(gf).max(bf);
    let mn = rf.min(gf).min(bf);
    let sat = (mx - mn) / (mx + 1e-6);
    let vib = p.vibrance / 100.0;
    let sat_g = p.saturation / 100.0;
    let dh = p.dehaze / 100.0;
    let f = 1.0 + sat_g + vib * (1.0 - sat) + dh * 0.10;
    if f != 1.0 {
        rf = l2 + (rf - l2) * f;
        gf = l2 + (gf - l2) * f;
        bf = l2 + (bf - l2) * f;
    }
    let [h0, s0, l0] = rgb2hsl(rf, gf, bf);
    let wr = hue_weight(h0, 0.0, 46.0);
    let wg = hue_weight(h0, 120.0, 46.0);
    let wb = hue_weight(h0, 240.0, 46.0);
    let mut hh = h0; let mut ss = s0; let mut ll = l0;
    if wr > 0.0 {
        hh += (p.hue_red / 100.0) * 0.22 * wr;
        ss += (p.sat_red / 100.0) * 0.45 * s0 * wr;
    }
    if wg > 0.0 {
        hh += (p.hue_green / 100.0) * 0.22 * wg;
        ss += (p.sat_green / 100.0) * 0.45 * s0 * wg;
    }
    if wb > 0.0 {
        hh += (p.hue_blue / 100.0) * 0.22 * wb;
        ss += (p.sat_blue / 100.0) * 0.45 * s0 * wb;
    }
    let st = p.shadow_tone / 100.0;
    if st != 0.0 && ll < 0.5 { hh += st * (0.5 - ll) * 2.0 * 0.06; }
    let ht = p.highlight_tone / 100.0;
    if ht != 0.0 && ll > 0.5 { hh += ht * (ll - 0.5) * 2.0 * 0.06; }
    hh = (hh % 1.0 + 1.0) % 1.0;
    let out = hsl2rgb(hh, jmax(0.0, jmin(1.0, ss)), ll);
    [clamp255(out[0]), clamp255(out[1]), clamp255(out[2])]
}

/// 生成 3D LUT 表格（对应 buildLUT，存 f32 = JS Float32Array）
pub fn build_lut(size: usize, p: &Params, m: Option<&MatchStats>) -> Vec<f32> {
    let curve = build_lum_curve(p);
    let f = (size - 1) as f64;
    let mut lut = vec![0f32; size * size * size * 3];
    for bi in 0..size {
        let b_n = bi as f64 / f;
        for gi in 0..size {
            let g_n = gi as f64 / f;
            for ri in 0..size {
                let r_n = ri as f64 / f;
                let o = grade_color(r_n * 255.0, g_n * 255.0, b_n * 255.0, p, &curve, m);
                let idx = ((bi * size + gi) * size + ri) * 3;
                lut[idx] = (o[0] / 255.0) as f32;
                lut[idx + 1] = (o[1] / 255.0) as f32;
                lut[idx + 2] = (o[2] / 255.0) as f32;
            }
        }
    }
    lut
}

/// 三线性插值采样 LUT（对应 sampleLUT）
pub fn sample_lut(lut: &[f32], size: usize, r: f64, g: f64, b: f64) -> [f64; 3] {
    let f = (size - 1) as f64;
    let rq = r * f; let gq = g * f; let bq = b * f;
    let r0 = rq.floor(); let g0 = gq.floor(); let b0 = bq.floor();
    let r1 = if r0 + 1.0 < size as f64 { r0 + 1.0 } else { r0 };
    let g1 = if g0 + 1.0 < size as f64 { g0 + 1.0 } else { g0 };
    let b1 = if b0 + 1.0 < size as f64 { b0 + 1.0 } else { b0 };
    let (r0i, g0i, b0i, r1i, g1i, b1i) = (r0 as usize, g0 as usize, b0 as usize, r1 as usize, g1 as usize, b1 as usize);
    let dr = rq - r0; let dg = gq - g0; let db = bq - b0;
    let i000 = ((b0i * size + g0i) * size + r0i) * 3;
    let i100 = ((b0i * size + g0i) * size + r1i) * 3;
    let i010 = ((b0i * size + g1i) * size + r0i) * 3;
    let i110 = ((b0i * size + g1i) * size + r1i) * 3;
    let i001 = ((b1i * size + g0i) * size + r0i) * 3;
    let i101 = ((b1i * size + g0i) * size + r1i) * 3;
    let i011 = ((b1i * size + g1i) * size + r0i) * 3;
    let i111 = ((b1i * size + g1i) * size + r1i) * 3;
    let mut out = [0.0f64; 3];
    for c in 0..3 {
        let v000 = lut[i000 + c] as f64; let v100 = lut[i100 + c] as f64;
        let v010 = lut[i010 + c] as f64; let v110 = lut[i110 + c] as f64;
        let v001 = lut[i001 + c] as f64; let v101 = lut[i101 + c] as f64;
        let v011 = lut[i011 + c] as f64; let v111 = lut[i111 + c] as f64;
        let v00 = v000 * (1.0 - dr) + v100 * dr;
        let v10 = v010 * (1.0 - dr) + v110 * dr;
        let v01 = v001 * (1.0 - dr) + v101 * dr;
        let v11 = v011 * (1.0 - dr) + v111 * dr;
        let v0 = v00 * (1.0 - dg) + v10 * dg;
        let v1 = v01 * (1.0 - dg) + v11 * dg;
        out[c] = v0 * (1.0 - db) + v1 * db;
    }
    out
}

/// 对整幅 RGBA 图像应用 LUT（对应 applyLUTToCanvas 的逐像素部分）
/// 输入 RGBA（u8），返回应用后的 RGBA。alpha 原样保留。
pub fn apply_lut_to_image(rgba: &[u8], lut: &[f32], size: usize) -> Vec<u8> {
    let mut out = rgba.to_vec();
    let n = rgba.len() / 4;
    for i in 0..n {
        let c = sample_lut(
            lut,
            size,
            rgba[i * 4] as f64 / 255.0,
            rgba[i * 4 + 1] as f64 / 255.0,
            rgba[i * 4 + 2] as f64 / 255.0,
        );
        out[i * 4] = (c[0] * 255.0).round() as u8;
        out[i * 4 + 1] = (c[1] * 255.0).round() as u8;
        out[i * 4 + 2] = (c[2] * 255.0).round() as u8;
    }
    out
}
