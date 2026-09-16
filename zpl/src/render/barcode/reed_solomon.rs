//! Polynomial division over binary extension fields. Coefficients are MSB first.
pub(super) fn multiply(mut a: usize, mut b: usize, polynomial: usize) -> usize {
    let top = 1 << (usize::BITS - 1 - polynomial.leading_zeros());
    let mut product = 0;
    while b != 0 {
        if b & 1 != 0 {
            product ^= a;
        }
        b >>= 1;
        a <<= 1;
        if a & top != 0 {
            a ^= polynomial;
        }
    }
    product
}
pub(super) fn parity(
    data: &[usize],
    count: usize,
    polynomial: usize,
    first_root: usize,
) -> Vec<usize> {
    let mut generator = vec![1];
    let mut root = 1;
    for _ in 0..first_root {
        root = multiply(root, 2, polynomial);
    }
    for _ in 0..count {
        let mut next = vec![0; generator.len() + 1];
        for (i, &v) in generator.iter().enumerate() {
            next[i] ^= v;
            next[i + 1] ^= multiply(v, root, polynomial);
        }
        generator = next;
        root = multiply(root, 2, polynomial);
    }
    let mut work = data.to_vec();
    work.resize(data.len() + count, 0);
    for i in 0..data.len() {
        let coefficient = work[i];
        for j in 1..generator.len() {
            work[i + j] ^= multiply(coefficient, generator[j], polynomial);
        }
    }
    work[data.len()..].to_vec()
}
