//! Original bounded interpreter of the OpenType TrueType instruction set.
//! https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
//! Graphics state lifecycle:
//! https://learn.microsoft.com/en-us/typography/opentype/spec/tt_graphics_state
use super::{read::div_round, *};
use std::collections::BTreeMap;

const MAX_STACK: usize = 4096;
const MAX_STEPS: usize = 1_000_000;

#[derive(Clone)]
struct Zone {
    current: Vec<Point>,
    original: Vec<Point>,
    unscaled: Option<Vec<Point>>,
    touched: Vec<[bool; 2]>,
}
impl Zone {
    fn new(points: Vec<Point>) -> Self {
        Self {
            touched: vec![[false; 2]; points.len()],
            original: points.clone(),
            current: points,
            unscaled: None,
        }
    }
}
#[derive(Clone)]
struct State {
    pv: [i32; 2],
    fv: [i32; 2],
    dual: [i32; 2],
    zp: [usize; 3],
    rp: [usize; 3],
    loops: usize,
    round: (i32, i32, i32),
    minimum: i32,
    cutin: i32,
    single_cutin: i32,
    single_width: i32,
    auto_flip: bool,
    delta_base: i32,
    delta_shift: u32,
    scan_control: u16,
    scan_type: u16,
    inhibit: bool,
    reset: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            pv: [16384, 0],
            fv: [16384, 0],
            dual: [16384, 0],
            zp: [1; 3],
            rp: [0; 3],
            loops: 1,
            round: (64, 0, 32),
            minimum: 64,
            cutin: 68,
            single_cutin: 0,
            single_width: 0,
            auto_flip: true,
            delta_base: 9,
            delta_shift: 3,
            scan_control: 0,
            scan_type: 0,
            inhibit: false,
            reset: false,
        }
    }
}

#[derive(Clone)]
pub(super) struct Vm {
    size: Size,
    units: u16,
    environment: Environment,
    cvt: Vec<i32>,
    storage: Vec<i32>,
    functions: BTreeMap<i32, Vec<u8>>,
    state: State,
    zones: [Zone; 2],
    stack: Vec<i32>,
    ends: Vec<usize>,
    steps: usize,
    definition_bytes: usize,
}
impl Vm {
    pub(super) fn new(
        font: &Font<'_>,
        size: Size,
        hinting: Hinting,
        environment: Environment,
    ) -> Result<Self> {
        let data = font.table(b"cvt ").unwrap_or_default();
        if !data.len().is_multiple_of(2) || data.len() > 8192 {
            return Err(invalid("invalid or excessive CVT"));
        }
        let mut vm = Self {
            size,
            units: font.units,
            environment,
            cvt: data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| {
                    environment.scale(i16::from_be_bytes([b[0], b[1]]) as i32, size.y, font.units)
                })
                .collect::<Result<Vec<_>>>()?,
            storage: vec![0; font.storage],
            functions: BTreeMap::new(),
            state: State::default(),
            zones: [
                Zone::new(vec![Point::default(); font.twilight + 4]),
                Zone::new(Vec::new()),
            ],
            stack: Vec::new(),
            ends: Vec::new(),
            steps: 0,
            definition_bytes: 0,
        };
        if hinting == Hinting::Native {
            vm.run(font.table(b"fpgm").unwrap_or_default(), 0)?;
            vm.stack.clear();
            vm.state = State::default();
            vm.steps = 0;
            vm.run(font.table(b"prep").unwrap_or_default(), 0)?;
            vm.stack.clear();
            // These fields are reset even when the size program establishes
            // the other per-glyph defaults (OpenType graphics-state summary).
            vm.state.pv = [16384, 0];
            vm.state.fv = [16384, 0];
            vm.state.dual = [16384, 0];
            vm.state.rp = [0; 3];
            vm.state.zp = [1; 3];
            vm.state.loops = 1;
        }
        Ok(vm)
    }
    pub(super) fn glyph(
        &self,
        points: Vec<Point>,
        ends: &[usize],
        program: &[u8],
        unscaled: Option<Vec<Point>>,
        work: &mut usize,
    ) -> Result<(Vec<Point>, u16, u16)> {
        let mut vm = self.clone();
        vm.steps = *work;
        for point in &points {
            bounded(point.x as i64)?;
            bounded(point.y as i64)?;
        }
        vm.zones[1] = Zone::new(points);
        vm.zones[1].unscaled = unscaled;
        vm.ends = ends.to_vec();
        if vm.state.reset {
            let inhibit = vm.state.inhibit;
            vm.state = State::default();
            vm.state.inhibit = inhibit;
        }
        if !vm.state.inhibit {
            vm.run(program, 0)?;
        }
        *work = vm.steps;
        Ok((
            std::mem::take(&mut vm.zones[1].current),
            vm.state.scan_control,
            vm.state.scan_type,
        ))
    }
    fn charge(&mut self, count: usize) -> Result<()> {
        self.steps = self
            .steps
            .checked_add(count)
            .ok_or_else(|| invalid("TrueType work budget exceeded"))?;
        if self.steps > MAX_STEPS {
            return Err(invalid("TrueType instruction/work budget exceeded"));
        }
        Ok(())
    }
    fn pop(&mut self) -> Result<i32> {
        self.stack
            .pop()
            .ok_or_else(|| invalid("TrueType stack underflow"))
    }
    fn index(&mut self) -> Result<usize> {
        usize::try_from(self.pop()?).map_err(|_| invalid("negative TrueType index"))
    }
    fn push(&mut self, value: i32) -> Result<()> {
        if self.stack.len() >= MAX_STACK {
            return Err(invalid("TrueType stack limit exceeded"));
        }
        bounded(value as i64)?;
        self.stack.push(value);
        Ok(())
    }
    fn point(&self, zone: usize, point: usize, original: bool) -> Result<Point> {
        let z = self
            .zones
            .get(zone)
            .ok_or_else(|| invalid("invalid TrueType zone"))?;
        (if original { &z.original } else { &z.current })
            .get(point)
            .copied()
            .ok_or_else(|| invalid("TrueType point index out of range"))
    }
    fn project(p: Point, v: [i32; 2]) -> i32 {
        div_round(p.x as i64 * v[0] as i64 + p.y as i64 * v[1] as i64, 16384) as i32
    }
    fn distance(&self, a: (usize, usize), b: (usize, usize), original: bool) -> Result<i32> {
        if original {
            if let Some(distance) = self.design_distance(a, b)? {
                return Ok(distance.round() as i32);
            }
        }
        let p = self.point(a.0, a.1, original)?;
        let q = self.point(b.0, b.1, original)?;
        Ok(Self::project(
            Point {
                x: p.x - q.x,
                y: p.y - q.y,
                ..Point::default()
            },
            if original {
                self.state.dual
            } else {
                self.state.pv
            },
        ))
    }
    fn design_distance(&self, a: (usize, usize), b: (usize, usize)) -> Result<Option<f64>> {
        if a.0 == 1 && b.0 == 1 {
            if let Some(points) = &self.zones[1].unscaled {
                let p = points
                    .get(a.1)
                    .ok_or_else(|| invalid("original point index out of range"))?;
                let q = points
                    .get(b.1)
                    .ok_or_else(|| invalid("original point index out of range"))?;
                let v = self.state.dual;
                return Ok(Some(
                    ((p.x - q.x) as f64 * self.environment.ppem(self.size.x, false) * v[0] as f64
                        + (p.y - q.y) as f64
                            * self.environment.ppem(self.size.y, false)
                            * v[1] as f64)
                        * 64.
                        / (self.units as f64 * 16384.),
                ));
            }
        }
        Ok(None)
    }
    fn move_by(&mut self, zone: usize, point: usize, delta: i32) -> Result<()> {
        let p = self.point(zone, point, false)?;
        let fv = self.state.fv;
        let pv = self.state.pv;
        let dot = fv[0] as i64 * pv[0] as i64 + fv[1] as i64 * pv[1] as i64;
        if dot == 0 {
            return Err(invalid("orthogonal TrueType freedom/projection vectors"));
        }
        let x = p.x as i64 + div_round(delta as i64 * fv[0] as i64 * 16384, dot);
        let y = p.y as i64 + div_round(delta as i64 * fv[1] as i64 * 16384, dot);
        self.assign(zone, point, x, y)?;
        for (i, &v) in fv.iter().enumerate() {
            if v != 0 {
                self.zones[zone].touched[point][i] = true;
            }
        }
        Ok(())
    }
    fn assign(&mut self, zone: usize, point: usize, x: i64, y: i64) -> Result<()> {
        self.point(zone, point, false)?;
        self.charge(1)?;
        bounded(x)?;
        bounded(y)?;
        let p = &mut self.zones[zone].current[point];
        p.x = x as i32;
        p.y = y as i32;
        Ok(())
    }
    fn round(&self, value: i32) -> i32 {
        let (period, phase, threshold) = self.state.round;
        if period == 0 {
            return value;
        }
        let sign = if value < 0 { -1_i64 } else { 1 };
        let n = (value as i64).abs();
        let rounded = ((n - phase as i64 + threshold as i64).div_euclid(period as i64)
            * period as i64
            + phase as i64)
            .max(0);
        (rounded * sign) as i32
    }
    fn ppem(&self) -> i32 {
        let [x, y] = self.state.pv;
        (((self.environment.ppem(self.size.x, false) * x as f64).powi(2)
            + (self.environment.ppem(self.size.y, false) * y as f64).powi(2))
        .sqrt()
            / 16384.)
            .round() as i32
    }
    fn cvt_ratio(&self) -> f64 {
        let [x, y] = self.state.pv;
        (((self.environment.ppem(self.size.x, false) / self.environment.ppem(self.size.y, false)
            * x as f64)
            .powi(2)
            + (y as f64).powi(2))
        .sqrt())
            / 16384.
    }
    fn cvt(&self, index: usize) -> Result<i32> {
        let value = *self
            .cvt
            .get(index)
            .ok_or_else(|| invalid("TrueType CVT index out of range"))?;
        bounded((value as f64 * self.cvt_ratio()).round() as i64)
    }
    fn write_cvt(&mut self, index: usize, value: i32) -> Result<()> {
        let ratio = self.cvt_ratio();
        *self
            .cvt
            .get_mut(index)
            .ok_or_else(|| invalid("TrueType CVT index out of range"))? =
            bounded((value as f64 / ratio).round() as i64)?;
        Ok(())
    }
    fn normalize(x: i32, y: i32) -> Result<[i32; 2]> {
        let length = (x as f64).hypot(y as f64);
        if length == 0. {
            return Ok([16384, 0]);
        }
        Ok([
            (x as f64 / length * 16384.).round() as i32,
            (y as f64 / length * 16384.).round() as i32,
        ])
    }
    fn vector_line(
        &self,
        p1: usize,
        p2: usize,
        perpendicular: bool,
        original: bool,
    ) -> Result<[i32; 2]> {
        let a = self.point(self.state.zp[1], p2, original)?;
        let b = self.point(self.state.zp[2], p1, original)?;
        let (x, y) = (a.x - b.x, a.y - b.y);
        Self::normalize(
            if perpendicular { -y } else { x },
            if perpendicular { x } else { y },
        )
    }
    fn reference_shift(&self, bit: u8) -> Result<(usize, usize, i32)> {
        let (zone, index) = if bit == 0 {
            (self.state.zp[1], self.state.rp[2])
        } else {
            (self.state.zp[0], self.state.rp[1])
        };
        let p = self.point(zone, index, false)?;
        let q = self.point(zone, index, true)?;
        Ok((
            zone,
            index,
            Self::project(
                Point {
                    x: p.x - q.x,
                    y: p.y - q.y,
                    ..Point::default()
                },
                self.state.pv,
            ),
        ))
    }
    fn take_loops(&mut self) -> usize {
        let n = self.state.loops;
        self.state.loops = 1;
        n
    }
    fn run(&mut self, code: &[u8], depth: usize) -> Result<()> {
        if code.len() > 65536 || depth > 32 {
            return Err(invalid("TrueType program/call limit exceeded"));
        }
        let mut pc = 0;
        while pc < code.len() {
            self.charge(1)?;
            let at = pc;
            let op = code[pc];
            pc += 1;
            let execute = (|| -> Result<()> {
                match op {
                    0x40 | 0x41 | 0xb0..=0xbf => {
                        let (count, word) = match op {
                            0x40 | 0x41 => {
                                let n = *code
                                    .get(pc)
                                    .ok_or_else(|| invalid("truncated push count"))?
                                    as usize;
                                pc += 1;
                                (n, op == 0x41)
                            }
                            0xb0..=0xb7 => ((op - 0xb0 + 1) as usize, false),
                            _ => ((op - 0xb8 + 1) as usize, true),
                        };
                        let bytes = count * if word { 2 } else { 1 };
                        let data = code
                            .get(pc..pc + bytes)
                            .ok_or_else(|| invalid("truncated TrueType push"))?;
                        pc += bytes;
                        for i in 0..count {
                            self.push(if word {
                                i16::from_be_bytes([data[2 * i], data[2 * i + 1]]) as i32
                            } else {
                                data[i] as i32
                            })?;
                        }
                    }
                    0x00..=0x05 => {
                        let vector = if op & 1 != 0 { [16384, 0] } else { [0, 16384] };
                        if op <= 3 {
                            self.state.pv = vector;
                            self.state.dual = vector;
                        }
                        if op <= 1 || op >= 4 {
                            self.state.fv = vector;
                        }
                    }
                    0x06..=0x09 | 0x86 | 0x87 => {
                        let p2 = self.index()?;
                        let p1 = self.index()?;
                        let vector = self.vector_line(p1, p2, op & 1 != 0, false)?;
                        if op == 8 || op == 9 {
                            self.state.fv = vector;
                        } else {
                            self.state.pv = vector;
                            self.state.dual = if op >= 0x86 {
                                self.vector_line(p1, p2, op & 1 != 0, true)?
                            } else {
                                vector
                            };
                        }
                    }
                    0x0a | 0x0b => {
                        let y = self.pop()? as i16 as i32;
                        let x = self.pop()? as i16 as i32;
                        let v = Self::normalize(x, y)?;
                        if op == 0x0a {
                            self.state.pv = v;
                            self.state.dual = v;
                        } else {
                            self.state.fv = v;
                        }
                    }
                    0x0c | 0x0d => {
                        let v = if op == 0x0c {
                            self.state.pv
                        } else {
                            self.state.fv
                        };
                        self.push(v[0])?;
                        self.push(v[1])?;
                    }
                    0x0e => self.state.fv = self.state.pv,
                    0x10..=0x12 => self.state.rp[(op - 0x10) as usize] = self.index()?,
                    0x13..=0x16 => {
                        let zone = self.index()?;
                        if zone > 1 {
                            return Err(invalid("invalid zone pointer"));
                        }
                        if op == 0x16 {
                            self.state.zp = [zone; 3];
                        } else {
                            self.state.zp[(op - 0x13) as usize] = zone;
                        }
                    }
                    0x17 => {
                        let n = self.index()?;
                        if n > MAX_STACK {
                            return Err(invalid("TrueType loop limit exceeded"));
                        }
                        self.state.loops = n;
                    }
                    0x18 => self.state.round = (64, 0, 32),
                    0x19 => self.state.round = (64, 32, 0),
                    0x1a => self.state.minimum = self.pop()?,
                    0x1b => {
                        let next = skip_branch(code, pc, false)?;
                        self.charge(next - pc)?;
                        pc = next;
                    }
                    0x1c => {
                        let delta = self.pop()?;
                        pc = jump(code, at, delta)?;
                    }
                    0x1d => self.state.cutin = self.pop()?,
                    0x1e => self.state.single_cutin = self.pop()?,
                    0x1f => {
                        let n = self.pop()?;
                        self.state.single_width =
                            self.environment.scale(n, self.size.y, self.units)?;
                    }
                    0x20 => {
                        let a = *self
                            .stack
                            .last()
                            .ok_or_else(|| invalid("DUP stack underflow"))?;
                        self.push(a)?;
                    }
                    0x21 => {
                        self.pop()?;
                    }
                    0x22 => self.stack.clear(),
                    0x23 => {
                        let a = self.pop()?;
                        let b = self.pop()?;
                        self.push(a)?;
                        self.push(b)?;
                    }
                    0x24 => self.push(self.stack.len() as i32)?,
                    0x25 | 0x26 => {
                        let k = self.index()?;
                        if k == 0 || k > self.stack.len() {
                            return Err(invalid("invalid stack index"));
                        }
                        let i = self.stack.len() - k;
                        let v = if op == 0x25 {
                            self.stack[i]
                        } else {
                            self.stack.remove(i)
                        };
                        self.push(v)?;
                    }
                    0x27 => {
                        let p1 = self.index()?;
                        let p2 = self.index()?;
                        let distance =
                            self.distance((self.state.zp[0], p2), (self.state.zp[1], p1), false)?;
                        self.move_by(self.state.zp[1], p1, distance / 2)?;
                        self.move_by(self.state.zp[0], p2, -distance + distance / 2)?;
                    }
                    0x29 => {
                        let p = self.index()?;
                        let z = self.state.zp[0];
                        self.point(z, p, false)?;
                        for i in 0..2 {
                            if self.state.fv[i] != 0 {
                                self.zones[z].touched[p][i] = false;
                            }
                        }
                    }
                    0x2a | 0x2b => {
                        let function = self.pop()?;
                        let count = if op == 0x2a { self.index()? } else { 1 };
                        if count > MAX_STACK {
                            return Err(invalid("LOOPCALL count limit exceeded"));
                        }
                        let body = self
                            .functions
                            .get(&function)
                            .cloned()
                            .ok_or_else(|| invalid("undefined TrueType function"))?;
                        self.charge(body.len() + count)?;
                        for _ in 0..count {
                            self.run(&body, depth + 1)?;
                        }
                    }
                    0x2c => {
                        let id = self.pop()?;
                        if !(0..4096).contains(&id) || self.functions.len() >= 4096 {
                            return Err(invalid("function definition limit exceeded"));
                        }
                        let start = pc;
                        loop {
                            let n = *code
                                .get(pc)
                                .ok_or_else(|| invalid("unterminated function"))?;
                            if n == 0x2d {
                                break;
                            }
                            if n == 0x2c {
                                return Err(invalid("nested function definition"));
                            }
                            let next = next_instruction(code, pc)?;
                            self.charge(next - pc)?;
                            pc = next;
                        }
                        let old = self.functions.get(&id).map_or(0, Vec::len);
                        self.definition_bytes = self.definition_bytes - old + pc - start;
                        if self.definition_bytes > 262_144 {
                            return Err(invalid("function byte budget exceeded"));
                        }
                        self.functions.insert(id, code[start..pc].to_vec());
                        pc += 1;
                    }
                    0x2d => return Err(invalid("unexpected ENDF")),
                    0x2e | 0x2f => {
                        let p = self.index()?;
                        let z = self.state.zp[0];
                        let value = Self::project(self.point(z, p, false)?, self.state.pv);
                        self.move_by(
                            z,
                            p,
                            if op & 1 != 0 {
                                self.round(value) - value
                            } else {
                                0
                            },
                        )?;
                        self.state.rp[0] = p;
                        self.state.rp[1] = p;
                    }
                    0x30 | 0x31 => self.iup((1 - (op & 1)) as usize)?,
                    0x32 | 0x33 => {
                        let (_, _, delta) = self.reference_shift(op & 1)?;
                        let count = self.take_loops();
                        for _ in 0..count {
                            let p = self.index()?;
                            self.move_by(self.state.zp[2], p, delta)?;
                        }
                    }
                    0x34..=0x37 => {
                        let (rz, rp, delta) = self.reference_shift(op & 1)?;
                        let (zone, start, end) = if op < 0x36 {
                            let c = self.index()?;
                            let z = self.state.zp[2];
                            if z != 1 {
                                return Err(invalid("SHC on twilight zone unsupported"));
                            }
                            let end = *self
                                .ends
                                .get(c)
                                .ok_or_else(|| invalid("SHC contour out of range"))?
                                + 1;
                            let start = if c == 0 { 0 } else { self.ends[c - 1] + 1 };
                            (z, start, end)
                        } else {
                            let z = self.index()?;
                            if z > 1 {
                                return Err(invalid("SHZ invalid zone"));
                            }
                            (z, 0, self.zones[z].current.len())
                        };
                        for p in start..end {
                            if zone != rz || p != rp {
                                self.move_by(zone, p, delta)?;
                            }
                        }
                    }
                    0x38 => {
                        let delta = self.pop()?;
                        let count = self.take_loops();
                        let z = self.state.zp[2];
                        let fv = self.state.fv;
                        for _ in 0..count {
                            let i = self.index()?;
                            let p = self.point(z, i, false)?;
                            self.assign(
                                z,
                                i,
                                p.x as i64 + div_round(delta as i64 * fv[0] as i64, 16384),
                                p.y as i64 + div_round(delta as i64 * fv[1] as i64, 16384),
                            )?;
                            for (axis, &value) in fv.iter().enumerate() {
                                if value != 0 {
                                    self.zones[z].touched[i][axis] = true;
                                }
                            }
                        }
                    }
                    0x39 => {
                        let a = (self.state.zp[0], self.state.rp[1]);
                        let b = (self.state.zp[1], self.state.rp[2]);
                        let old = self
                            .design_distance(b, a)?
                            .unwrap_or(self.distance(b, a, true)? as f64);
                        let new = self.distance(b, a, false)?;
                        let count = self.take_loops();
                        for _ in 0..count {
                            let p = self.index()?;
                            let q = (self.state.zp[2], p);
                            let d = self
                                .design_distance(q, a)?
                                .unwrap_or(self.distance(q, a, true)? as f64);
                            let target = if old == 0. {
                                d.round() as i32
                            } else {
                                (d * new as f64 / old).round() as i32
                            };
                            let target = bounded(target as i64)?;
                            let current = self.distance(q, a, false)?;
                            self.move_by(q.0, q.1, target - current)?;
                        }
                    }
                    0x3a | 0x3b => {
                        let distance = self.pop()?;
                        let p = self.index()?;
                        let z = self.state.zp[1];
                        let refp = (self.state.zp[0], self.state.rp[0]);
                        if z == 0 {
                            let base = self.point(refp.0, refp.1, false)?;
                            self.zones[z]
                                .current
                                .get_mut(p)
                                .ok_or_else(|| invalid("twilight index out of range"))?
                                .clone_from(&base);
                            self.move_by(z, p, distance)?;
                            self.zones[z].original[p] = self.zones[z].current[p];
                        } else {
                            let d = self.distance((z, p), refp, false)?;
                            self.move_by(z, p, distance - d)?;
                        }
                        self.state.rp[1] = self.state.rp[0];
                        self.state.rp[2] = p;
                        if op & 1 != 0 {
                            self.state.rp[0] = p;
                        }
                    }
                    0x3c => {
                        let reference = (self.state.zp[0], self.state.rp[0]);
                        let count = self.take_loops();
                        for _ in 0..count {
                            let p = self.index()?;
                            let z = self.state.zp[1];
                            let d = self.distance((z, p), reference, false)?;
                            self.move_by(z, p, -d)?;
                        }
                    }
                    0x3d => self.state.round = (32, 0, 16),
                    0x3e | 0x3f => {
                        let c = self.index()?;
                        let p = self.index()?;
                        let z = self.state.zp[0];
                        let mut target = self.cvt(c)?;
                        if z == 0 {
                            self.point(z, p, false)?;
                            self.zones[z].current[p] = Point::default();
                            self.move_by(z, p, target)?;
                            self.zones[z].original[p] = self.zones[z].current[p];
                        }
                        let current = Self::project(self.point(z, p, false)?, self.state.pv);
                        if op & 1 != 0 {
                            if (target as i64 - current as i64).abs() > self.state.cutin as i64 {
                                target = current;
                            }
                            target = self.round(target);
                        }
                        self.move_by(z, p, target - current)?;
                        self.state.rp[0] = p;
                        self.state.rp[1] = p;
                    }
                    0x42 => {
                        let value = self.pop()?;
                        let index = self.index()?;
                        *self
                            .storage
                            .get_mut(index)
                            .ok_or_else(|| invalid("storage index out of range"))? = value;
                    }
                    0x43 => {
                        let index = self.index()?;
                        let v = *self
                            .storage
                            .get(index)
                            .ok_or_else(|| invalid("storage index out of range"))?;
                        self.push(v)?;
                    }
                    0x44 | 0x70 => {
                        let value = self.pop()?;
                        let index = self.index()?;
                        if op == 0x44 {
                            self.write_cvt(index, value)?;
                        } else {
                            *self
                                .cvt
                                .get_mut(index)
                                .ok_or_else(|| invalid("CVT index out of range"))? =
                                self.environment.scale(value, self.size.y, self.units)?;
                        }
                    }
                    0x45 => {
                        let index = self.index()?;
                        self.push(self.cvt(index)?)?;
                    }
                    0x46 | 0x47 => {
                        let p = self.index()?;
                        let original = op & 1 != 0;
                        let value = Self::project(
                            self.point(self.state.zp[2], p, original)?,
                            if original {
                                self.state.dual
                            } else {
                                self.state.pv
                            },
                        );
                        self.push(value)?;
                    }
                    0x48 => {
                        let target = self.pop()?;
                        let p = self.index()?;
                        let z = self.state.zp[2];
                        let current = Self::project(self.point(z, p, false)?, self.state.pv);
                        self.move_by(z, p, target - current)?;
                        if z == 0 {
                            self.zones[z].original[p] = self.zones[z].current[p];
                        }
                    }
                    0x49 | 0x4a => {
                        let p2 = self.index()?;
                        let p1 = self.index()?;
                        let d = self.distance(
                            (self.state.zp[0], p1),
                            (self.state.zp[1], p2),
                            op == 0x4a,
                        )?;
                        self.push(d)?;
                    }
                    0x4b => self.push(self.ppem())?,
                    0x4c => return Err(invalid("MPS requires an explicit point-size policy")),
                    0x4d => self.state.auto_flip = true,
                    0x4e => self.state.auto_flip = false,
                    0x50..=0x55 | 0x5a | 0x5b | 0x60..=0x63 | 0x8b | 0x8c => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        let value = match op {
                            0x50 => (a < b) as i32,
                            0x51 => (a <= b) as i32,
                            0x52 => (a > b) as i32,
                            0x53 => (a >= b) as i32,
                            0x54 => (a == b) as i32,
                            0x55 => (a != b) as i32,
                            0x5a => (a != 0 && b != 0) as i32,
                            0x5b => (a != 0 || b != 0) as i32,
                            0x60 => a.checked_add(b).ok_or_else(|| invalid("ADD overflow"))?,
                            0x61 => a.checked_sub(b).ok_or_else(|| invalid("SUB overflow"))?,
                            0x62 => {
                                if b == 0 {
                                    return Err(invalid("TrueType division by zero"));
                                }
                                i32::try_from(div_round(a as i64 * 64, b as i64))
                                    .map_err(|_| invalid("DIV overflow"))?
                            }
                            0x63 => i32::try_from(div_round(a as i64 * b as i64, 64))
                                .map_err(|_| invalid("MUL overflow"))?,
                            0x8b => a.max(b),
                            _ => a.min(b),
                        };
                        self.push(value)?;
                    }
                    0x56 | 0x57 => {
                        let a = self.pop()?;
                        let odd = self.round(a) & 127 == 64;
                        self.push((if op == 0x56 { odd } else { !odd }) as i32)?;
                    }
                    0x58 => {
                        if self.pop()? == 0 {
                            let next = skip_branch(code, pc, true)?;
                            self.charge(next - pc)?;
                            pc = next;
                        }
                    }
                    0x59 => {}
                    0x5c => {
                        let a = self.pop()?;
                        self.push((a == 0) as i32)?;
                    }
                    0x5d | 0x71..=0x75 => self.delta(op)?,
                    0x5e => {
                        let n = self.pop()?;
                        if !(0..=255).contains(&n) {
                            return Err(invalid("invalid DELTA base"));
                        }
                        self.state.delta_base = n;
                    }
                    0x5f => {
                        let n = self.index()?;
                        if n > 6 {
                            return Err(invalid("unsupported DELTA shift"));
                        }
                        self.state.delta_shift = n as u32;
                    }
                    0x64..=0x6f => {
                        let a = self.pop()?;
                        let value = match op {
                            0x64 => a.checked_abs().ok_or_else(|| invalid("ABS overflow"))?,
                            0x65 => a.checked_neg().ok_or_else(|| invalid("NEG overflow"))?,
                            0x66 => a & !63,
                            0x67 => {
                                a.checked_add(63)
                                    .ok_or_else(|| invalid("CEILING overflow"))?
                                    & !63
                            }
                            0x68..=0x6b => self.round(a),
                            _ => a,
                        };
                        self.push(value)?;
                    }
                    0x77 => return Err(invalid("S45ROUND is not supported")),
                    0x76 => {
                        let n = self.pop()?;
                        let base = 64;
                        let period = match (n >> 6) & 3 {
                            0 => base / 2,
                            1 => base,
                            2 => base * 2,
                            _ => return Err(invalid("reserved super-round period")),
                        };
                        let phase = ((n >> 4) & 3) * period / 4;
                        let threshold = if n & 15 == 0 {
                            period - 1
                        } else {
                            ((n & 15) - 4) * period / 8
                        };
                        self.state.round = (period, phase, threshold);
                    }
                    0x78 | 0x79 => {
                        let condition = self.pop()? != 0;
                        let delta = self.pop()?;
                        if condition == (op == 0x78) {
                            pc = jump(code, at, delta)?;
                        }
                    }
                    0x7a => self.state.round = (0, 0, 0),
                    0x7c => self.state.round = (64, 0, 63),
                    0x7d => self.state.round = (64, 0, 0),
                    0x7e | 0x7f => {
                        self.pop()?;
                    }
                    0x80 => {
                        let count = self.take_loops();
                        for _ in 0..count {
                            let p = self.index()?;
                            let z = self.state.zp[0];
                            self.point(z, p, false)?;
                            self.zones[z].current[p].on_curve ^= true;
                        }
                    }
                    0x81 | 0x82 => {
                        let high = self.index()?;
                        let low = self.index()?;
                        let z = self.state.zp[0];
                        self.point(z, high, false)?;
                        if low > high {
                            return Err(invalid("invalid FLIP range"));
                        }
                        self.charge(high - low + 1)?;
                        for p in low..=high {
                            self.zones[z].current[p].on_curve = op == 0x81;
                        }
                    }
                    0x85 => self.state.scan_control = self.pop()? as u16,
                    0x88 => {
                        let selector = self.pop()?;
                        let mut result = if selector & 1 != 0 { 35 } else { 0 };
                        if let Environment::Zebra203 { quarter_turns }
                        | Environment::Zd621V93 { quarter_turns } = self.environment
                        {
                            if selector & 4 != 0
                                && quarter_turns % 2 == 0
                                && self.size.x != self.size.y
                            {
                                result |= 1 << 9;
                            }
                        }
                        self.push(result)?;
                    }
                    0x8a => {
                        let c = self.pop()?;
                        let b = self.pop()?;
                        let a = self.pop()?;
                        self.push(b)?;
                        self.push(c)?;
                        self.push(a)?;
                    }
                    0x8d => {
                        let n = self.pop()?;
                        if !(0..=5).contains(&n) {
                            return Err(invalid("unsupported scan type"));
                        }
                        self.state.scan_type = n as u16;
                    }
                    0x8e => {
                        let selector = self.pop()?;
                        let value = self.pop()?;
                        match selector {
                            1 => self.state.inhibit = value & 1 != 0,
                            2 => self.state.reset = value & 2 != 0,
                            _ => return Err(invalid("unsupported INSTCTRL selector")),
                        }
                    }
                    0xc0..=0xff => self.relative(op)?,
                    _ => {
                        return Err(Error(format!(
                            "unsupported TrueType instruction 0x{op:02x}"
                        )))
                    }
                }
                Ok(())
            })();
            execute.map_err(|e| Error(format!("{} at byte {at} (opcode 0x{op:02x})", e.0)))?;
        }
        Ok(())
    }
    fn relative(&mut self, op: u8) -> Result<()> {
        let cvt = if op >= 0xe0 {
            Some(self.index()?)
        } else {
            None
        };
        let p = self.index()?;
        let z = self.state.zp[1];
        let reference = (self.state.zp[0], self.state.rp[0]);
        let mut distance = if let Some(c) = cvt {
            self.cvt(c)?
        } else {
            self.distance((z, p), reference, true)?
        };
        if cvt.is_some() && z == 0 {
            let base = self.point(reference.0, reference.1, true)?;
            self.point(z, p, false)?;
            self.zones[z].current[p] = base;
            self.move_by(z, p, distance)?;
            self.zones[z].original[p] = self.zones[z].current[p];
        }
        let original = self.distance((z, p), reference, true)?;
        if cvt.is_some() && self.state.auto_flip && (distance < 0) != (original < 0) {
            distance = -distance;
        }
        if (distance as i64)
            .abs()
            .abs_diff((self.state.single_width as i64).abs())
            < self.state.single_cutin.max(0) as u64
        {
            distance = if distance < 0 {
                -self.state.single_width.abs()
            } else {
                self.state.single_width.abs()
            };
        }
        if op & 4 != 0 {
            if cvt.is_some()
                && z == reference.0
                && (distance as i64 - original as i64).abs() > self.state.cutin as i64
            {
                distance = original;
            }
            distance = self.round(distance);
        }
        if op & 8 != 0 {
            distance = if original < 0 {
                distance.min(-self.state.minimum)
            } else {
                distance.max(self.state.minimum)
            };
        }
        let current = self.distance((z, p), reference, false)?;
        self.move_by(z, p, distance - current)?;
        self.state.rp[1] = self.state.rp[0];
        self.state.rp[2] = p;
        if op & 16 != 0 {
            self.state.rp[0] = p;
        }
        Ok(())
    }
    fn delta(&mut self, op: u8) -> Result<()> {
        let count = self.index()?;
        if count > MAX_STACK / 2 {
            return Err(invalid("DELTA count limit exceeded"));
        }
        let group = match op {
            0x5d | 0x73 => 0,
            0x71 | 0x74 => 16,
            _ => 32,
        };
        for _ in 0..count {
            let index = self.index()?;
            let argument = self.pop()?;
            if !(0..=255).contains(&argument) {
                return Err(invalid("invalid DELTA argument"));
            }
            if (argument >> 4) + self.state.delta_base + group == self.ppem() {
                let step = (argument & 15) - 8;
                let step = if step >= 0 { step + 1 } else { step };
                let delta = step * (64 >> self.state.delta_shift);
                if op >= 0x73 {
                    let value = self.cvt(index)?;
                    self.write_cvt(index, value + delta)?;
                } else {
                    self.move_by(self.state.zp[0], index, delta)?;
                }
            }
        }
        Ok(())
    }
    fn iup(&mut self, axis: usize) -> Result<()> {
        if self.state.zp[2] != 1 {
            return Err(invalid("IUP requires glyph zone"));
        }
        self.charge(self.zones[1].current.len())?;
        let z = &mut self.zones[1];
        let coordinate = |p: Point| if axis == 0 { p.x } else { p.y };
        // Interpolation ratios must retain the original outline's precision.
        // Rounding every input coordinate to 26.6 before taking the ratio can
        // move untouched points by another 1/64 dot at small sizes.
        let original = z.unscaled.as_deref().unwrap_or(&z.original);
        let mut start = 0;
        for &end in &self.ends {
            let touched = (start..=end)
                .filter(|&p| z.touched[p][axis])
                .collect::<Vec<_>>();
            if touched.len() == 1 {
                let p = touched[0];
                let delta = coordinate(z.current[p]) - coordinate(z.original[p]);
                for i in start..=end {
                    if i != p {
                        let value = bounded(coordinate(z.original[i]) as i64 + delta as i64)?;
                        let value = bounded(value as i64)?;
                        if axis == 0 {
                            z.current[i].x = value;
                        } else {
                            z.current[i].y = value;
                        }
                    }
                }
            } else if touched.len() > 1 {
                for n in 0..touched.len() {
                    let a = touched[n];
                    let b = touched[(n + 1) % touched.len()];
                    let (mut x1, mut x2) = (coordinate(original[a]), coordinate(original[b]));
                    let (mut y1, mut y2) = (coordinate(z.current[a]), coordinate(z.current[b]));
                    let (mut old1, mut old2) =
                        (coordinate(z.original[a]), coordinate(z.original[b]));
                    if x1 > x2 {
                        std::mem::swap(&mut x1, &mut x2);
                        std::mem::swap(&mut y1, &mut y2);
                        std::mem::swap(&mut old1, &mut old2);
                    }
                    let mut i = if a == end { start } else { a + 1 };
                    while i != b {
                        let x = coordinate(original[i]);
                        let old = coordinate(z.original[i]);
                        let value = if x1 == x2 {
                            if y1 == y2 {
                                old + y1 - old1
                            } else {
                                old
                            }
                        } else if x <= x1 {
                            old + y1 - old1
                        } else if x >= x2 {
                            old + y2 - old2
                        } else {
                            y1 + div_round((x - x1) as i64 * (y2 - y1) as i64, (x2 - x1) as i64)
                                as i32
                        };
                        let value = bounded(value as i64)?;
                        if axis == 0 {
                            z.current[i].x = value;
                        } else {
                            z.current[i].y = value;
                        }
                        i = if i == end { start } else { i + 1 };
                    }
                }
            }
            start = end + 1;
        }
        Ok(())
    }
}

/// Decode PUSH payload boundaries without interpreting their bytes as opcodes.
pub(super) fn next_instruction(code: &[u8], pc: usize) -> Result<usize> {
    let op = *code
        .get(pc)
        .ok_or_else(|| invalid("truncated TrueType instruction"))?;
    let mut next = pc + 1;
    let bytes = match op {
        0x40 | 0x41 => {
            let n = *code
                .get(next)
                .ok_or_else(|| invalid("truncated PUSH count"))? as usize;
            next += 1;
            n * if op == 0x41 { 2 } else { 1 }
        }
        0xb0..=0xb7 => (op - 0xb0 + 1) as usize,
        0xb8..=0xbf => (op - 0xb8 + 1) as usize * 2,
        _ => 0,
    };
    next += bytes;
    if next > code.len() {
        return Err(invalid("truncated PUSH payload"));
    }
    Ok(next)
}
fn skip_branch(code: &[u8], mut pc: usize, stop_else: bool) -> Result<usize> {
    let mut nesting = 0;
    while pc < code.len() {
        let op = code[pc];
        let next = next_instruction(code, pc)?;
        match op {
            0x58 => nesting += 1,
            0x59 => {
                if nesting == 0 {
                    return Ok(next);
                }
                nesting -= 1;
            }
            0x1b if nesting == 0 && stop_else => return Ok(next),
            _ => {}
        }
        pc = next;
    }
    Err(invalid("unterminated TrueType conditional"))
}
fn jump(code: &[u8], pc: usize, delta: i32) -> Result<usize> {
    let next = pc as i64 + delta as i64;
    if next < 0 || next > code.len() as i64 {
        return Err(invalid("TrueType jump outside program"));
    }
    Ok(next as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vm() -> Vm {
        Vm {
            size: Size { x: 32, y: 16 },
            units: 2048,
            environment: Environment::Standard,
            cvt: vec![64, 128, 256],
            storage: vec![0; 8],
            functions: BTreeMap::new(),
            state: State::default(),
            zones: [
                Zone::new(vec![Point::default(); 8]),
                Zone::new(vec![
                    Point {
                        x: 0,
                        y: 0,
                        on_curve: true,
                    },
                    Point {
                        x: 320,
                        y: 0,
                        on_curve: true,
                    },
                    Point {
                        x: 640,
                        y: 0,
                        on_curve: true,
                    },
                ]),
            ],
            stack: Vec::new(),
            ends: vec![2],
            steps: 0,
            definition_bytes: 0,
        }
    }
    #[test]
    fn functions_conditionals_and_push_payloads_are_distinct() {
        let mut vm = vm();
        // Function 7 adds two values. The skipped branch contains opcode-looking
        // bytes inside PUSH payloads, which must not close IF or define functions.
        vm.run(
            &[
                0xb0, 7, 0x2c, 0x60, 0x2d, 0xb0, 0, 0x58, 0xb2, 0x59, 0x2c, 0x1b, 0x1b, 0xb1, 20,
                22, 0xb0, 7, 0x2b, 0x59,
            ],
            0,
        )
        .unwrap();
        assert_eq!(vm.stack, vec![42]);
        vm.run(&[0xb8, 0xff, 0xc0, 0x63], 0).unwrap();
        assert_eq!(vm.stack, vec![-42]);
    }
    #[test]
    fn projected_cvt_and_device_sizes_follow_each_axis() {
        let mut vm = vm();
        vm.run(&[0x01, 0x4b, 0xb0, 0, 0x45, 0x00, 0x4b, 0xb0, 0, 0x45], 0)
            .unwrap();
        assert_eq!(vm.stack, vec![32, 128, 16, 64]);
    }
    #[test]
    fn iup_interpolates_untouched_points_and_keeps_endpoints() {
        let mut vm = vm();
        vm.run(&[0xb1, 0, 64, 0x48, 0xb0, 2, 0xb8, 3, 0, 0x48, 0x31], 0)
            .unwrap();
        assert_eq!(
            vm.zones[1].current.iter().map(|p| p.x).collect::<Vec<_>>(),
            vec![64, 416, 768]
        );
        assert_eq!(
            vm.zones[1].touched,
            vec![[true, false], [false, false], [true, false]]
        );
    }
    #[test]
    fn delta_only_changes_matching_size() {
        let mut vm = vm();
        vm.state.delta_base = 32;
        // DELTAP: argument 8 selects base ppem and +1/8 dot; point 1.
        vm.run(&[0xb2, 8, 1, 1, 0x5d], 0).unwrap();
        assert_eq!(vm.zones[1].current[1].x, 328);
        vm.state.pv = [0, 16384];
        vm.run(&[0xb2, 8, 1, 1, 0x5d], 0).unwrap();
        assert_eq!(vm.zones[1].current[1].x, 328);
    }
    #[test]
    fn glyph_work_and_numbers_are_bounded_and_state_is_isolated() {
        let base = vm();
        let points = base.zones[1].current.clone();
        // A glyph may change CVT without affecting the prepared size instance.
        let mut work = 0;
        base.glyph(points.clone(), &[2], &[0xb1, 0, 20, 0x44], None, &mut work)
            .unwrap();
        assert_eq!(base.cvt, vec![64, 128, 256]);
        assert!(work > 0);
        let mut work = MAX_STEPS;
        assert!(base
            .glyph(points, &[2], &[0x00], None, &mut work)
            .unwrap_err()
            .0
            .contains("budget"));
        let mut machine = vm();
        // Multiplication grows a valid stack value beyond the numeric limit.
        assert!(machine
            .run(&[0xb8, 0x7f, 0xff, 0x20, 0x63, 0x20, 0x63], 0)
            .is_err());
        // Force a nearly exhausted budget, then shift the whole zone.
        let mut machine = vm();
        machine.steps = MAX_STEPS - 3;
        assert!(machine
            .run(&[0xb0, 1, 0x36], 0)
            .unwrap_err()
            .0
            .contains("budget"));
    }

    #[test]
    fn arbitrary_instruction_bytes_and_operand_extremes_do_not_panic() {
        // Deterministic malformed-program coverage, not a substitute for fuzzing.
        let mut seed = 0x32d17a91_u32;
        for op in 0..=255 {
            for extreme in [-MAX_COORD, -1, 0, 1, MAX_COORD] {
                let mut machine = vm();
                machine.stack = vec![extreme; 16];
                let _ = machine.run(&[op], 0);
            }
        }
        for _ in 0..512 {
            let mut bytes = [0; 64];
            for byte in &mut bytes {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *byte = seed as u8;
            }
            let _ = vm().run(&bytes, 0);
        }
    }

    #[test]
    fn invalid_programs_fail_and_execution_is_bounded() {
        for code in [
            &[0x60][..],
            &[0xb0][..],
            &[0xb0, 9, 0x2b][..],
            &[0xb0, 9, 0x13][..],
            &[0x89][..],
            &[0xb0, 0, 0x58][..],
        ] {
            assert!(vm().run(code, 0).is_err(), "{code:?}");
        }
        // Jump back to PUSH, replenishing the offset indefinitely.
        assert!(vm()
            .run(&[0xb8, 0xff, 0xfd, 0x1c], 0)
            .unwrap_err()
            .0
            .contains("budget exceeded"));
    }
}
