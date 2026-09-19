//! Original shortest-path encodation of ISO/IEC 24778:2008 §7.3.1.1, Table 2.
//! https://www.iso.org/standard/41548.html
//! The five states are Upper, Lower, Mixed, Punctuation and Digit. Edges consume
//! one character, one punctuation pair or a bounded binary run; no encoder
//! implementation or algorithm source from a third-party library is used.
use super::bits;
const U: usize = 0;
const L: usize = 1;
const M: usize = 2;
const P: usize = 3;
const D: usize = 4;

#[derive(Clone, Copy)]
struct Prefix {
    value: usize,
    width: usize,
}
impl Prefix {
    fn then(self, value: usize, width: usize) -> Self {
        Self {
            value: (self.value << width) | value,
            width: self.width + width,
        }
    }
}
#[derive(Clone, Copy)]
struct Step {
    prefix: Prefix,
    end: usize,
    state: usize,
    binary: bool,
}
fn width(state: usize) -> usize {
    if state == D {
        4
    } else {
        5
    }
}
fn character(state: usize, c: u8) -> Option<usize> {
    match state {
        U if c.is_ascii_uppercase() => Some((c - b'A') as usize + 2),
        L if c.is_ascii_lowercase() => Some((c - b'a') as usize + 2),
        U | L | M | D if c == b' ' => Some(1),
        D if c.is_ascii_digit() => Some((c - b'0') as usize + 2),
        D if c == b',' => Some(12),
        D if c == b'.' => Some(13),
        M => {
            b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x1b\x1c\x1d\x1e\x1f@\\^_`|~\x7f"
                .iter()
                .position(|&v| v == c)
                .map(|i| i + 2)
        }
        P if c == b'\r' => Some(1),
        P => b"!\"#$%&'()*+,-./:;<=>?[]{}"
            .iter()
            .position(|&v| v == c)
            .map(|i| i + 6),
        _ => None,
    }
}
fn pair(data: &[u8]) -> Option<usize> {
    match data.get(..2)? {
        b"\r\n" => Some(2),
        b". " => Some(3),
        b", " => Some(4),
        b": " => Some(5),
        _ => None,
    }
}

pub(super) fn encode(data: &[u8], preserve_binary_runs: bool) -> Vec<bool> {
    let empty = Prefix { value: 0, width: 0 };
    let mut routes = [[Prefix {
        value: 0,
        width: 100,
    }; 5]; 5];
    for (i, row) in routes.iter_mut().enumerate() {
        row[i] = empty;
    }
    for (from, to, code) in [
        (U, L, 28),
        (U, M, 29),
        (U, D, 30),
        (L, M, 29),
        (L, D, 30),
        (M, L, 28),
        (M, U, 29),
        (M, P, 30),
        (P, U, 31),
        (D, U, 14),
    ] {
        routes[from][to] = empty.then(code, width(from));
    }
    // All-pairs paths contain only latches, so their final state is unambiguous.
    for k in 0..5 {
        for i in 0..5 {
            for j in 0..5 {
                if routes[i][k].width + routes[k][j].width < routes[i][j].width {
                    routes[i][j] = routes[i][k].then(routes[k][j].value, routes[k][j].width);
                }
            }
        }
    }
    let n = data.len();
    let mut costs = vec![[0usize; 5]; n + 1];
    let mut steps = vec![
        [Step {
            prefix: empty,
            end: n,
            state: U,
            binary: false
        }; 5];
        n
    ];
    for i in (0..n).rev() {
        // A binary run returns to U/L/M. Find its best end once per return
        // state, then add each starting state's latch cost. Maximum 2078 bytes
        // follows the 11-bit extended count plus 31 in Table 2.
        let mut binary = [(usize::MAX, i); 3];
        for (state, best) in binary.iter_mut().enumerate() {
            for len in 1..=(n - i).min(2078) {
                if preserve_binary_runs
                    && len < 2078
                    && i + len < n
                    && !(0..5).any(|s| character(s, data[i + len]).is_some())
                {
                    continue;
                }
                let cost = 5 + if len <= 31 { 5 } else { 16 } + 8 * len + costs[i + len][state];
                if cost < best.0 {
                    *best = (cost, i + len);
                }
            }
        }
        for (state, route) in routes.iter().enumerate() {
            let mut best = usize::MAX;
            let mut offer =
                |prefix: Prefix, end: usize, next: usize, binary: bool, payload_cost: usize| {
                    let cost = prefix.width + payload_cost + costs[end][next];
                    // Latch before a shared/shifted character when that latch
                    // is inevitable and total length ties. This gives a stable
                    // earliest-transition choice, also seen in printer streams.
                    if cost < best || (cost == best && prefix.width > steps[i][state].prefix.width)
                    {
                        best = cost;
                        steps[i][state] = Step {
                            prefix,
                            end,
                            state: next,
                            binary,
                        };
                    }
                };
            for (target, &prefix) in route.iter().enumerate() {
                if let Some(c) = character(target, data[i]) {
                    offer(prefix.then(c, width(target)), i + 1, target, false, 0);
                }
                if target == P {
                    if let Some(c) = pair(&data[i..]) {
                        offer(prefix.then(c, 5), i + 2, target, false, 0);
                    }
                }
                // One-symbol shifts return to the latched state, including
                // punctuation pairs (which count as one codeword).
                if target != P {
                    let shifted = prefix.then(0, width(target));
                    if let Some(c) = character(P, data[i]) {
                        offer(shifted.then(c, 5), i + 1, target, false, 0);
                    }
                    if let Some(c) = pair(&data[i..]) {
                        offer(shifted.then(c, 5), i + 2, target, false, 0);
                    }
                }
                if target == L || target == D {
                    if let Some(c) = character(U, data[i]) {
                        offer(
                            prefix
                                .then(if target == L { 28 } else { 15 }, width(target))
                                .then(c, 5),
                            i + 1,
                            target,
                            false,
                            0,
                        );
                    }
                }
            }
            for (target, &(cost, end)) in binary.iter().enumerate() {
                let prefix = route[target].then(31, 5);
                offer(prefix, end, target, true, cost - 5 - costs[end][target]);
            }
            costs[i][state] = best;
        }
    }
    let mut result = Vec::with_capacity(costs[0][U]);
    let (mut i, mut state) = (0, U);
    while i < n {
        let step = steps[i][state];
        bits::push(&mut result, step.prefix.value, step.prefix.width);
        if step.binary {
            let len = step.end - i;
            if len <= 31 {
                bits::push(&mut result, len, 5);
            } else {
                bits::push(&mut result, 0, 5);
                bits::push(&mut result, len - 31, 11);
            }
            for &c in &data[i..step.end] {
                bits::push(&mut result, c as usize, 8);
            }
        }
        i = step.end;
        state = step.state;
    }
    debug_assert_eq!(result.len(), costs[0][U]);
    result
}
