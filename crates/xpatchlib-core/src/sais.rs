//! SA-IS suffix array construction (Nong, Zhang, Chan 2009), the algorithm
//! family behind bsdiff's suffix sorting, over an integer alphabet so the
//! recursion can reuse it directly. Input values are `>= 1`; value `0` is
//! reserved for the implicit trailing sentinel, which guarantees the empty
//! suffix sorts first and terminates the recursion.

/// Computes the suffix array of `data`. The result has `data.len() + 1`
/// entries and always starts with `data.len()` (the empty suffix), followed
/// by the suffix start positions in lexicographic order.
pub(crate) fn suffix_array(data: &[u8]) -> Vec<i32> {
    if data.is_empty() {
        return vec![0];
    }
    let mut text = vec![0i32; data.len()];
    let mut max_value = 0i32;
    for (i, &c) in data.iter().enumerate() {
        text[i] = c as i32 + 1;
        if text[i] > max_value {
            max_value = text[i];
        }
    }
    let mut sa = vec![0i32; data.len() + 1];
    sais(&text, &mut sa, max_value);
    sa
}

fn sais(text: &[i32], sa: &mut [i32], max_value: i32) {
    let n = text.len();
    if n == 0 {
        sa[0] = 0;
        return;
    }
    if n == 1 {
        sa[0] = 1;
        sa[1] = 0;
        return;
    }

    // S-type classification: t[i] is true when suffix i is S-type (strictly
    // smaller than its successor), with the sentinel position always S-type.
    let mut t = vec![false; n + 1];
    t[n] = true;
    for i in (0..n).rev() {
        let next = if i + 1 < n { text[i + 1] } else { 0 };
        t[i] = text[i] < next || (text[i] == next && t[i + 1]);
    }
    // The sentinel position maps to character 0.
    let ch = |i: usize| -> i32 {
        if i >= n {
            0
        } else {
            text[i]
        }
    };
    // Position n is always treated as an LMS position: the empty suffix must
    // be seeded into its bucket for the induction to cover every slot.
    let is_lms = |i: usize| i == n || (i > 0 && t[i] && !t[i - 1]);

    let mut bucket = vec![0i32; max_value as usize + 2];
    count_buckets(text, &mut bucket);
    buckets_to_end(&mut bucket);

    // LMS positions in ascending text order; the sentinel position closes
    // the list and becomes the tail of the reduced problem.
    let lms: Vec<i32> = (1..=n).filter(|&i| is_lms(i)).map(|i| i as i32).collect();
    let m = lms.len();

    // Round 1: seed the LMS positions at their bucket ends in reverse text
    // order, induce, and read the LMS substrings back in sorted order.
    sa.fill(-1);
    for &p in lms.iter().rev() {
        let c = ch(p as usize) as usize;
        bucket[c] -= 1;
        sa[bucket[c] as usize] = p;
    }
    induce(sa, &t, text, max_value, n);

    let sorted_lms: Vec<i32> = sa
        .iter()
        .copied()
        .filter(|&p| p >= 0 && is_lms(p as usize))
        .collect();

    // Name the LMS substrings by their sorted order; equal substrings share
    // a name. name_at and end_of are indexed by text position.
    let mut end_of = vec![0i32; n + 1];
    for k in 0..m {
        end_of[lms[k] as usize] = if k + 1 < m { lms[k + 1] } else { n as i32 };
    }
    let mut name_at = vec![0i32; n + 1];
    let mut names = 1i32;
    name_at[n] = 1; // the sentinel substring always sorts first
    for w in 1..sorted_lms.len() {
        let (a, b) = (sorted_lms[w - 1] as usize, sorted_lms[w] as usize);
        if !lms_substrings_equal(text, a, b, &end_of) {
            names += 1;
        }
        name_at[b] = names;
    }
    let reduced: Vec<i32> = lms.iter().map(|&p| name_at[p as usize]).collect();

    // Solve the reduced problem. With all names distinct the suffix order is
    // the counting sort of the names themselves; otherwise recurse.
    let mut sa1 = vec![0i32; m + 1];
    if names as usize == m {
        sa1[0] = m as i32;
        for (k, &v) in reduced.iter().enumerate() {
            sa1[v as usize] = k as i32;
        }
    } else {
        let reduced_max = reduced.iter().copied().max().unwrap_or(0);
        sais(&reduced, &mut sa1, reduced_max);
    }

    // Round 2: seed the LMS suffixes in their true sorted order and induce
    // the final suffix array.
    sa.fill(-1);
    count_buckets(text, &mut bucket);
    buckets_to_end(&mut bucket);
    for &w in sa1[1..].iter().rev() {
        let p = lms[w as usize];
        let c = ch(p as usize) as usize;
        bucket[c] -= 1;
        sa[bucket[c] as usize] = p;
    }
    induce(sa, &t, text, max_value, n);
}

fn count_buckets(text: &[i32], bucket: &mut [i32]) {
    bucket.fill(0);
    bucket[0] = 1; // the sentinel itself
    for &v in text {
        bucket[v as usize] += 1;
    }
}

fn buckets_to_start(bucket: &mut [i32]) {
    let mut sum = 0;
    for v in bucket.iter_mut() {
        let c = *v;
        *v = sum;
        sum += c;
    }
}

fn buckets_to_end(bucket: &mut [i32]) {
    let mut sum = 0;
    for v in bucket.iter_mut() {
        sum += *v;
        *v = sum;
    }
}

/// Completes a full suffix order from the seeded suffixes using the classic
/// left-to-right L-type pass followed by the right-to-left S-type pass. Each
/// pass resets its bucket pointers, so the caller's (possibly decremented)
/// seeding state never leaks into the induction.
fn induce(sa: &mut [i32], t: &[bool], text: &[i32], max_value: i32, n: usize) {
    let mut bucket = vec![0i32; max_value as usize + 2];
    count_buckets(text, &mut bucket);
    buckets_to_start(&mut bucket);
    for i in 0..=n {
        let p = sa[i];
        if p < 1 {
            continue;
        }
        let q = (p - 1) as usize;
        if !t[q] {
            let c = text[q] as usize;
            sa[bucket[c] as usize] = p - 1;
            bucket[c] += 1;
        }
    }
    count_buckets(text, &mut bucket);
    buckets_to_end(&mut bucket);
    for i in (0..=n).rev() {
        let p = sa[i];
        if p < 1 {
            continue;
        }
        let q = (p - 1) as usize;
        if t[q] {
            let c = text[q] as usize;
            bucket[c] -= 1;
            sa[bucket[c] as usize] = p - 1;
        }
    }
}

/// Reports whether the LMS substrings starting at a and b are identical.
/// `end_of` maps an LMS position to the end of its substring.
fn lms_substrings_equal(text: &[i32], a: usize, b: usize, end_of: &[i32]) -> bool {
    let (len_a, len_b) = (
        end_of[a] as usize - a + 1,
        end_of[b] as usize - b + 1,
    );
    if len_a != len_b {
        return false;
    }
    (0..len_a).all(|i| ch_at(text, a + i) == ch_at(text, b + i))
}

fn ch_at(text: &[i32], i: usize) -> i32 {
    if i >= text.len() {
        0
    } else {
        text[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive_suffix_array(data: &[u8]) -> Vec<i32> {
        let mut sa: Vec<i32> = (0..data.len() as i32).collect();
        sa.sort_by(|&a, &b| data[a as usize..].cmp(&data[b as usize..]));
        sa
    }

    fn check(data: &[u8]) {
        let sa = suffix_array(data);
        let want = naive_suffix_array(data);
        assert_eq!(
            sa.len(),
            want.len() + 1,
            "len={} length mismatch",
            data.len()
        );
        assert_eq!(sa[0], data.len() as i32, "len={} empty suffix first", data.len());
        for i in 0..want.len() {
            assert_eq!(
                sa[i + 1],
                want[i],
                "len={} entry {} mismatch",
                data.len(),
                i
            );
        }
    }

    #[test]
    fn against_naive_sorted_inputs() {
        for s in [
            &b""[..],
            b"a",
            b"aa",
            b"ab",
            b"ba",
            b"banana",
            b"mississippi",
            b"aaaaaaaaaa",
            b"abcabcabcabc",
        ] {
            check(s);
        }
    }

    #[test]
    fn against_naive_random_inputs() {
        let mut state = 0x9E3779B97F4A7C15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for &alphabet in &[2usize, 3, 4, 16, 256] {
            for &size in &[1usize, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610] {
                let mut data = vec![0u8; size];
                for b in data.iter_mut() {
                    *b = (next() % alphabet as u64) as u8;
                }
                check(&data);
            }
            let mut data = vec![0u8; 5000];
            for b in data.iter_mut() {
                *b = (next() % alphabet as u64) as u8;
            }
            check(&data);
        }
    }
}
