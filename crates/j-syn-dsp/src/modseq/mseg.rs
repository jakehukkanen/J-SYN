//! Continuous multi-segment breakpoint envelope (`MSEG` / `LFO` mode) types, presets,
//! and curve evaluation.

use serde::{Deserialize, Serialize};

pub const MAX_MSEG_NODES: usize = 16;

/// Breakpoint preset templates for `LFO` (MSEG) mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoPreset {
    Flat,
    RampUp,
    RampDown,
    ConcaveArc,
    ConvexArc,
    SCurve,
    Triangle,
    Sine,
    VShape,
}

impl LfoPreset {
    pub const ALL: [Self; 9] = [
        Self::Flat,
        Self::RampUp,
        Self::RampDown,
        Self::ConcaveArc,
        Self::ConvexArc,
        Self::SCurve,
        Self::Triangle,
        Self::Sine,
        Self::VShape,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Flat => "FLAT",
            Self::RampUp => "RAMP+",
            Self::RampDown => "RAMP-",
            Self::ConcaveArc => "CONCV",
            Self::ConvexArc => "CONVX",
            Self::SCurve => "S-CRV",
            Self::Triangle => "TRI",
            Self::Sine => "SINE",
            Self::VShape => "V-SHP",
        }
    }

    pub fn to_mseg(self) -> MsegCurve {
        let mut nodes = [MsegNode::default(); MAX_MSEG_NODES];
        let num_nodes = match self {
            Self::Flat => {
                nodes[0] = MsegNode {
                    time_x: 0.0,
                    level_y: 0.5,
                    tension: 0.0,
                };
                nodes[1] = MsegNode {
                    time_x: 1.0,
                    level_y: 0.5,
                    tension: 0.0,
                };
                2
            }
            Self::RampUp => {
                nodes[0] = MsegNode {
                    time_x: 0.0,
                    level_y: 0.0,
                    tension: 0.0,
                };
                nodes[1] = MsegNode {
                    time_x: 1.0,
                    level_y: 1.0,
                    tension: 0.0,
                };
                2
            }
            Self::RampDown => {
                nodes[0] = MsegNode {
                    time_x: 0.0,
                    level_y: 1.0,
                    tension: 0.0,
                };
                nodes[1] = MsegNode {
                    time_x: 1.0,
                    level_y: 0.0,
                    tension: 0.0,
                };
                2
            }
            Self::ConcaveArc => {
                nodes[0] = MsegNode {
                    time_x: 0.0,
                    level_y: 0.0,
                    tension: 0.55,
                };
                nodes[1] = MsegNode {
                    time_x: 1.0,
                    level_y: 1.0,
                    tension: 0.0,
                };
                2
            }
            Self::ConvexArc => {
                nodes[0] = MsegNode {
                    time_x: 0.0,
                    level_y: 0.0,
                    tension: -0.55,
                };
                nodes[1] = MsegNode {
                    time_x: 1.0,
                    level_y: 1.0,
                    tension: 0.0,
                };
                2
            }
            Self::SCurve => {
                nodes[0] = MsegNode {
                    time_x: 0.0,
                    level_y: 0.0,
                    tension: 0.42,
                };
                nodes[1] = MsegNode {
                    time_x: 0.5,
                    level_y: 0.5,
                    tension: -0.42,
                };
                nodes[2] = MsegNode {
                    time_x: 1.0,
                    level_y: 1.0,
                    tension: 0.0,
                };
                3
            }
            Self::Triangle => {
                nodes[0] = MsegNode {
                    time_x: 0.0,
                    level_y: 0.0,
                    tension: 0.0,
                };
                nodes[1] = MsegNode {
                    time_x: 0.5,
                    level_y: 1.0,
                    tension: 0.0,
                };
                nodes[2] = MsegNode {
                    time_x: 1.0,
                    level_y: 0.0,
                    tension: 0.0,
                };
                3
            }
            Self::Sine => {
                nodes[0] = MsegNode {
                    time_x: 0.0,
                    level_y: 0.5,
                    tension: -0.35,
                };
                nodes[1] = MsegNode {
                    time_x: 0.25,
                    level_y: 1.0,
                    tension: 0.33,
                };
                nodes[2] = MsegNode {
                    time_x: 0.5,
                    level_y: 0.5,
                    tension: -0.35,
                };
                nodes[3] = MsegNode {
                    time_x: 0.75,
                    level_y: 0.0,
                    tension: 0.33,
                };
                nodes[4] = MsegNode {
                    time_x: 1.0,
                    level_y: 0.5,
                    tension: 0.0,
                };
                5
            }
            Self::VShape => {
                nodes[0] = MsegNode {
                    time_x: 0.0,
                    level_y: 1.0,
                    tension: 0.0,
                };
                nodes[1] = MsegNode {
                    time_x: 0.5,
                    level_y: 0.0,
                    tension: 0.0,
                };
                nodes[2] = MsegNode {
                    time_x: 1.0,
                    level_y: 1.0,
                    tension: 0.0,
                };
                3
            }
        };
        MsegCurve { nodes, num_nodes }
    }
}

/// Movable breakpoint node in a continuous `MSEG` envelope (`time_x`, `level_y`, `tension`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct MsegNode {
    pub time_x: f32,
    pub level_y: f32,
    pub tension: f32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MsegNodeWire {
    time_x: f32,
    level_y: f32,
    tension: f32,
}

impl<'de> Deserialize<'de> for MsegNode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<MsegNodeWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        Ok(Self {
            time_x: crate::serde_lenient::finite_clamped(w.time_x, 0.0, 0.0, 1.0),
            level_y: crate::serde_lenient::finite_clamped(w.level_y, 0.0, 0.0, 1.0),
            tension: crate::serde_lenient::finite_clamped(w.tension, 0.0, -1.0, 1.0),
        })
    }
}

/// Fixed-size zero-allocation multi-segment envelope (`2..=16` nodes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MsegCurve {
    pub nodes: [MsegNode; MAX_MSEG_NODES],
    pub num_nodes: usize,
}

#[derive(Deserialize)]
#[serde(default)]
struct MsegCurveWire {
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")]
    nodes: [MsegNode; MAX_MSEG_NODES],
    num_nodes: usize,
}

impl Default for MsegCurveWire {
    fn default() -> Self {
        let d = MsegCurve::default();
        Self {
            nodes: d.nodes,
            num_nodes: d.num_nodes,
        }
    }
}

impl<'de> Deserialize<'de> for MsegCurve {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(mut w) = Option::<MsegCurveWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        if w.num_nodes < 2 {
            return Ok(Self::default());
        }
        let n = w.num_nodes.clamp(2, MAX_MSEG_NODES);
        w.nodes[0].time_x = 0.0;
        let mut prev_t = 0.0_f32;
        for node in &mut w.nodes[..n] {
            node.time_x = node.time_x.clamp(prev_t, 1.0);
            prev_t = node.time_x;
        }
        w.nodes[n - 1].time_x = 1.0;
        Ok(Self {
            nodes: w.nodes,
            num_nodes: n,
        })
    }
}

impl Default for MsegCurve {
    fn default() -> Self {
        let mut nodes = [MsegNode::default(); MAX_MSEG_NODES];
        nodes[0] = MsegNode {
            time_x: 0.0,
            level_y: 0.0,
            tension: -0.35,
        };
        nodes[1] = MsegNode {
            time_x: 0.15,
            level_y: 1.0,
            tension: 0.45,
        };
        nodes[2] = MsegNode {
            time_x: 0.55,
            level_y: 0.35,
            tension: 0.25,
        };
        nodes[3] = MsegNode {
            time_x: 1.0,
            level_y: 0.0,
            tension: 0.0,
        };
        Self {
            nodes,
            num_nodes: 4,
        }
    }
}

/// Evaluates a continuous multi-segment `MSEG` curve at normalized phase in `[0.0, 1.0]`.
#[inline]
pub fn evaluate_mseg_curve(mseg: &MsegCurve, phase: f32) -> f32 {
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    let p = phase.clamp(0.0, 1.0);
    if p <= mseg.nodes[0].time_x {
        return mseg.nodes[0].level_y.clamp(0.0, 1.0);
    }
    for i in 0..(n - 1) {
        let a = mseg.nodes[i];
        let b = mseg.nodes[i + 1];
        if p <= b.time_x || i + 2 == n {
            let span = (b.time_x - a.time_x).max(1e-5);
            let local_t = ((p - a.time_x) / span).clamp(0.0, 1.0);
            let exp = 2.0_f32.powf(a.tension.clamp(-1.0, 1.0) * 2.5);
            let curved_t = local_t.powf(exp);
            return (a.level_y + (b.level_y - a.level_y) * curved_t).clamp(0.0, 1.0);
        }
    }
    mseg.nodes[n - 1].level_y.clamp(0.0, 1.0)
}
