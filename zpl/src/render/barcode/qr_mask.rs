//! ZD621 automatic QR mask selection. Normative penalty categories:
//! ISO/IEC 18004:2000 §8.8.2, https://www.iso.org/standard/30789.html.
//! Printer departures and offline firmware evidence: docs/qr-mask-selection.md.
use super::Matrix;

pub(super) fn select(mut candidates: Vec<Matrix>) -> Matrix {
    let mut ranks: Vec<_> = candidates
        .iter()
        .enumerate()
        .map(|(mask, m)| {
            let mut blocks = 0;
            for y in 0..m.h - 1 {
                for x in 0..m.w - 1 {
                    let c = m.get(x, y);
                    if c == m.get(x + 1, y) && c == m.get(x, y + 1) && c == m.get(x + 1, y + 1) {
                        blocks += 3;
                    }
                }
            }
            (mask, blocks)
        })
        .collect();
    // Stable ties retain ascending mask order. Only the first three masks
    // survive the printer's initial 2x2-block ranking.
    ranks.sort_by_key(|&(_, blocks)| blocks);
    let selected = ranks[..3]
        .iter()
        .min_by_key(|&&(mask, blocks)| blocks + remaining_penalty(&candidates[mask]))
        .unwrap()
        .0;
    candidates.swap_remove(selected)
}

fn remaining_penalty(m: &Matrix) -> usize {
    let mut penalty = 0;
    let mut dark = 0;
    for vertical in [false, true] {
        for line in 0..m.w {
            let get = |i| {
                if vertical {
                    m.get(line, i)
                } else {
                    m.get(i, line)
                }
            };
            // Even run indices are white. Keep a zero-length leading white
            // run when the first module is black.
            let mut runs = vec![0usize];
            let mut black = false;
            for i in 0..m.w {
                if get(i) != black {
                    if black && !vertical {
                        dark += *runs.last().unwrap();
                    }
                    runs.push(0);
                    black = !black;
                }
                *runs.last_mut().unwrap() += 1;
            }
            // The native balance counter omits a terminal black run.
            // Firmware stores individual runs in signed bytes. Long runs in
            // extended Model 1 symbols wrap and do not contribute to N1.
            let mut runs: Vec<_> = runs.into_iter().map(|n| n as i8 as i32).collect();
            penalty += runs
                .iter()
                .filter(|&&n| n >= 5)
                .map(|&n| (n - 2) as usize)
                .sum::<usize>();
            if black {
                runs.push(0);
            }
            // Native finder-pattern evaluation treats exterior white runs
            // as 127 modules, counts a qualifying pattern once, and accepts
            // scaled 1:1:3:1:1 runs with four units of white on either side.
            runs[0] = 127;
            *runs.last_mut().unwrap() = 127;
            for i in (1..runs.len().saturating_sub(5)).step_by(2) {
                let n = runs[i];
                if runs[i + 1] == n
                    && runs[i + 2] == n * 3
                    && runs[i + 3] == n
                    && runs[i + 4] == n
                    && (runs[i - 1] >= n * 4 || runs[i + 5] >= n * 4)
                {
                    penalty += 40;
                }
            }
        }
    }
    // Deliberately divide the area first, matching the integer arithmetic
    // observed in firmware and independently verified native mask choices.
    let percent = dark / (m.w * m.h / 100);
    penalty + percent.abs_diff(50) / 5 * 10
}
