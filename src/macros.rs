macro_rules! concat_sstr {
    ($a:expr, $b:expr) => {{
        const _: &str = $a;
        const _: &str = $b;

        const LEN: usize = $a.len() + $b.len();
        const SLICE: [u8; LEN] = $crate::macros::concat_bytes::<LEN>($a.as_bytes(), $b.as_bytes());
        unsafe { std::str::from_utf8_unchecked(&SLICE) }
    }};
}
pub(crate) use concat_sstr;

pub const fn concat_bytes<const LEN: usize>(a: &[u8], b: &[u8]) -> [u8; LEN] {
    let mut bytes = [0; LEN];
    let mut i = 0;
    while i < a.len() {
        bytes[i] = a[i];
        i += 1;
    }
    let mut j = 0;
    while j < b.len() {
        bytes[a.len() + j] = b[j];
        j += 1;
    }

    return bytes;
}
