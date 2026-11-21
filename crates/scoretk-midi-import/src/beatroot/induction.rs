// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Induction.h` / `Induction.cpp` du plug-in BeatRoot (Vamp, Queen
// Mary University of London), copyright 2011 Simon Dixon, Chris Cannam et
// QMUL, licence GPL-2.0-ou-ultérieure, modifié (réécrit en Rust) en 2026.
// Référence : `src/importexport/midi/thirdparty/beatroot/Induction.{h,cpp}`.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see the LICENSE file at the root of this repository or
// <https://www.gnu.org/licenses/gpl-3.0.html>.

//! Induction de tempo (`Induction.cpp`).

use super::agent::{Agent, AgentParameters};
use super::agent_list::AgentList;
use super::event::Event;

const CLUSTER_WIDTH: f64 = 0.025;
const MIN_IOI: f64 = 0.070;
const MAX_IOI: f64 = 2.500;
const MIN_IBI: f64 = 0.3;
const MAX_IBI: f64 = 1.0;
const TOP_N: usize = 10;

/// `Induction::beatInduction` (`Induction.cpp:39-204`).
pub fn beat_induction(params: AgentParameters, events: &[Event]) -> AgentList {
    let mut result = AgentList::new();
    if events.len() < 2 {
        return result;
    }
    let max_cluster_count = ((MAX_IOI - MIN_IOI) / CLUSTER_WIDTH).ceil() as usize;
    let mut mean = vec![0.0; max_cluster_count];
    let mut size = vec![0_i32; max_cluster_count];
    let mut intervals = 0_usize;

    // Regroupement des intervalles inter-attaques.
    for (i, first) in events.iter().enumerate() {
        for second in events.iter().skip(i + 1) {
            let ioi = second.time - first.time;
            if ioi < MIN_IOI {
                continue;
            }
            if ioi > MAX_IOI {
                break;
            }
            let mut b = 0;
            while b < intervals {
                if (mean[b] - ioi).abs() < CLUSTER_WIDTH {
                    if b < intervals - 1 && (mean[b + 1] - ioi).abs() < (mean[b] - ioi).abs() {
                        b += 1;
                    }
                    mean[b] = (mean[b] * size[b] as f64 + ioi) / (size[b] + 1) as f64;
                    size[b] += 1;
                    break;
                }
                b += 1;
            }
            if b == intervals {
                if intervals == max_cluster_count {
                    continue;
                }
                intervals += 1;
                while b > 0 && mean[b - 1] > ioi {
                    mean[b] = mean[b - 1];
                    size[b] = size[b - 1];
                    b -= 1;
                }
                mean[b] = ioi;
                size[b] = 1;
            }
        }
    }

    // Fusion des amas voisins.
    let mut b = 0;
    while b < intervals {
        let mut i = b + 1;
        while i < intervals {
            if (mean[b] - mean[i]).abs() < CLUSTER_WIDTH {
                mean[b] = (mean[b] * size[b] as f64 + mean[i] * size[i] as f64)
                    / (size[b] + size[i]) as f64;
                size[b] += size[i];
                intervals -= 1;
                for j in (i + 1)..=intervals {
                    mean[j - 1] = mean[j];
                    size[j - 1] = size[j];
                }
            } else {
                i += 1;
            }
        }
        b += 1;
    }
    if intervals == 0 {
        return result;
    }

    let mut score: Vec<i32> = size.iter().map(|count| 10 * count).collect();
    let mut best_n = [0_usize; TOP_N];
    best_n[0] = 0;
    let mut best_count = 1_usize;
    let mut b = 0;
    while b < intervals {
        let mut i = 0;
        while i <= best_count {
            if i < TOP_N && (i == best_count || score[b] > score[best_n[i]]) {
                if best_count < TOP_N {
                    best_count += 1;
                }
                for j in (i + 1..best_count).rev() {
                    best_n[j] = best_n[j - 1];
                }
                best_n[i] = b;
                break;
            }
            i += 1;
        }
        b += 1;
    }

    // Relations harmoniques entre amas.
    for b in 0..intervals {
        for i in (b + 1)..intervals {
            let ratio = mean[b] / mean[i];
            let submult = ratio < 1.0;
            let degree = if submult {
                (1.0 / ratio).round() as i32
            } else {
                ratio.round() as i32
            };
            if (2..=8).contains(&degree) {
                let err = if submult {
                    (mean[b] * degree as f64 - mean[i]).abs()
                } else {
                    (mean[b] - mean[i] * degree as f64).abs()
                };
                if err
                    < if submult {
                        CLUSTER_WIDTH
                    } else {
                        CLUSTER_WIDTH * degree as f64
                    }
                {
                    let weight = if degree >= 5 { 1 } else { 6 - degree };
                    score[b] += weight * size[i];
                    score[i] += weight * size[b];
                }
            }
        }
    }

    for &b in &best_n[..best_count] {
        let mut new_sum = mean[b] * score[b] as f64;
        let mut new_weight = score[b];
        for i in 0..intervals {
            if i == b {
                continue;
            }
            let ratio = mean[b] / mean[i];
            let degree = if ratio < 1.0 {
                (1.0 / ratio).round()
            } else {
                ratio.round()
            } as i32;
            if !(2..=8).contains(&degree) {
                continue;
            }
            let err = if ratio < 1.0 {
                (mean[b] * degree as f64 - mean[i]).abs()
            } else {
                (mean[b] - mean[i] * degree as f64).abs()
            };
            if err
                < if ratio < 1.0 {
                    CLUSTER_WIDTH
                } else {
                    CLUSTER_WIDTH * degree as f64
                }
            {
                if ratio < 1.0 {
                    new_sum += mean[i] / degree as f64 * score[i] as f64;
                } else {
                    new_sum += mean[i] * degree as f64 * score[i] as f64;
                }
                new_weight += score[i];
            }
        }
        let mut beat = new_sum / new_weight as f64;
        while beat < MIN_IBI {
            beat *= 2.0;
        }
        while beat > MAX_IBI {
            beat /= 2.0;
        }
        if beat >= MIN_IBI {
            result.list.push(Agent::new(params, beat));
        }
    }
    result
}
