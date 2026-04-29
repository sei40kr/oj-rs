/// Output comparison policy. Mirrors upstream `oj test --compare-mode`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CompareMode {
    ExactMatch,
    CrlfInsensitiveExactMatch,
    IgnoreSpaces,
    IgnoreSpacesAndNewlines,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    Summary,
    All,
    Diff,
    DiffAll,
}

pub fn compare(actual: &[u8], expected: &[u8], mode: CompareMode, error: Option<f64>) -> bool {
    let (a, e) = match mode {
        CompareMode::ExactMatch => (actual.to_vec(), expected.to_vec()),
        _ => (normalize_crlf(actual), normalize_crlf(expected)),
    };

    if error.is_none()
        && matches!(
            mode,
            CompareMode::ExactMatch | CompareMode::CrlfInsensitiveExactMatch
        )
    {
        return a == e;
    }

    match mode {
        CompareMode::IgnoreSpacesAndNewlines => compare_tokens_flat(&a, &e, error),
        _ => compare_tokens_lines(&a, &e, error),
    }
}

fn normalize_crlf(buf: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len());
    let mut i = 0;
    while i < buf.len() {
        if buf[i] == b'\r' && i + 1 < buf.len() && buf[i + 1] == b'\n' {
            out.push(b'\n');
            i += 2;
        } else {
            out.push(buf[i]);
            i += 1;
        }
    }
    out
}

fn compare_tokens_flat(a: &[u8], e: &[u8], error: Option<f64>) -> bool {
    let ta: Vec<&[u8]> = a
        .split(|c| (*c as char).is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .collect();
    let te: Vec<&[u8]> = e
        .split(|c| (*c as char).is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .collect();
    if ta.len() != te.len() {
        return false;
    }
    ta.iter().zip(te.iter()).all(|(x, y)| token_eq(x, y, error))
}

fn compare_tokens_lines(a: &[u8], e: &[u8], error: Option<f64>) -> bool {
    let la: Vec<&[u8]> = strip_trailing_lf(a).split(|c| *c == b'\n').collect();
    let le: Vec<&[u8]> = strip_trailing_lf(e).split(|c| *c == b'\n').collect();
    if la.len() != le.len() {
        return false;
    }
    for (la_i, le_i) in la.iter().zip(le.iter()) {
        let ta: Vec<&[u8]> = la_i
            .split(|c| (*c as char).is_ascii_whitespace())
            .filter(|t| !t.is_empty())
            .collect();
        let te: Vec<&[u8]> = le_i
            .split(|c| (*c as char).is_ascii_whitespace())
            .filter(|t| !t.is_empty())
            .collect();
        if ta.len() != te.len() {
            return false;
        }
        for (x, y) in ta.iter().zip(te.iter()) {
            if !token_eq(x, y, error) {
                return false;
            }
        }
    }
    true
}

fn strip_trailing_lf(buf: &[u8]) -> &[u8] {
    if let Some(last) = buf.last() {
        if *last == b'\n' {
            return &buf[..buf.len() - 1];
        }
    }
    buf
}

fn token_eq(a: &[u8], b: &[u8], error: Option<f64>) -> bool {
    if let Some(eps) = error {
        if let (Ok(sa), Ok(sb)) = (std::str::from_utf8(a), std::str::from_utf8(b)) {
            if let (Ok(fa), Ok(fb)) = (sa.parse::<f64>(), sb.parse::<f64>()) {
                return is_close(fa, fb, eps, eps);
            }
        }
    }
    a == b
}

fn is_close(a: f64, b: f64, rel_tol: f64, abs_tol: f64) -> bool {
    if a == b {
        return true;
    }
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let diff = (a - b).abs();
    diff <= abs_tol || diff <= rel_tol * a.abs().max(b.abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_default_passes() {
        assert!(compare(
            b"abc\r\n",
            b"abc\n",
            CompareMode::CrlfInsensitiveExactMatch,
            None
        ));
        assert!(!compare(
            b"abc\r\n",
            b"abc\n",
            CompareMode::ExactMatch,
            None
        ));
    }

    #[test]
    fn ignore_spaces_within_line() {
        assert!(compare(
            b"1 2  3\n",
            b"1  2 3\n",
            CompareMode::IgnoreSpaces,
            None
        ));
        assert!(!compare(
            b"1\n2 3\n",
            b"1 2 3\n",
            CompareMode::IgnoreSpaces,
            None
        ));
    }

    #[test]
    fn ignore_spaces_and_newlines_flat() {
        assert!(compare(
            b"1\n2 3\n",
            b"1 2 3\n",
            CompareMode::IgnoreSpacesAndNewlines,
            None
        ));
    }

    #[test]
    fn float_error_tolerance() {
        assert!(compare(
            b"1.0000001\n",
            b"1.0\n",
            CompareMode::IgnoreSpacesAndNewlines,
            Some(1e-6)
        ));
        assert!(!compare(
            b"1.01\n",
            b"1.0\n",
            CompareMode::IgnoreSpacesAndNewlines,
            Some(1e-6)
        ));
    }
}
