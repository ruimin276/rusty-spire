use crate::state::RngStreamState;

const MBIG: i32 = i32::MAX;
const MSEED: i32 = 161_803_398;

#[derive(Clone, Debug)]
pub struct DotNetSeededRandom {
    seed_array: [i32; 56],
    inext: usize,
    inextp: usize,
}

#[derive(Clone, Debug)]
pub struct Xoshiro256StarStar {
    state: [u64; 4],
}

impl Xoshiro256StarStar {
    pub fn new(seed: u32) -> Self {
        let mut seed = seed as u64;
        let mut state = [0; 4];
        for value in &mut state {
            *value = splitmix64(&mut seed);
        }
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let shifted = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= shifted;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }

    pub fn next_int(&mut self, max_exclusive: u32) -> u32 {
        assert!(max_exclusive > 0 && max_exclusive <= i32::MAX as u32);
        let sample = (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0);
        (sample * max_exclusive as f64) as u32
    }
}

fn splitmix64(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(11_400_714_819_323_198_485);
    let mut value = *seed;
    value = (value ^ (value >> 30)).wrapping_mul(13_787_848_793_156_543_929);
    value = (value ^ (value >> 27)).wrapping_mul(10_723_151_780_598_845_931);
    value ^ (value >> 31)
}

impl DotNetSeededRandom {
    pub fn new(seed: u32) -> Self {
        let signed_seed = seed as i32;
        let subtraction = if signed_seed == i32::MIN {
            i32::MAX
        } else {
            signed_seed.abs()
        };
        let mut seed_array = [0; 56];
        let mut mj = MSEED - subtraction;
        if mj < 0 {
            mj += MBIG;
        }
        seed_array[55] = mj;
        let mut mk = 1;
        for i in 1..55 {
            let ii = (21 * i) % 55;
            seed_array[ii] = mk;
            mk = mj - mk;
            if mk < 0 {
                mk += MBIG;
            }
            mj = seed_array[ii];
        }
        for _ in 0..4 {
            for i in 1..56 {
                seed_array[i] -= seed_array[1 + (i + 30) % 55];
                if seed_array[i] < 0 {
                    seed_array[i] += MBIG;
                }
            }
        }
        Self {
            seed_array,
            inext: 0,
            inextp: 21,
        }
    }

    fn internal_sample(&mut self) -> i32 {
        let mut loc_inext = self.inext + 1;
        if loc_inext >= 56 {
            loc_inext = 1;
        }
        let mut loc_inextp = self.inextp + 1;
        if loc_inextp >= 56 {
            loc_inextp = 1;
        }
        let mut ret = self.seed_array[loc_inext] - self.seed_array[loc_inextp];
        if ret == MBIG {
            ret -= 1;
        }
        if ret < 0 {
            ret += MBIG;
        }
        self.seed_array[loc_inext] = ret;
        self.inext = loc_inext;
        self.inextp = loc_inextp;
        ret
    }

    pub fn next_int(&mut self, max_exclusive: u32) -> u32 {
        assert!(max_exclusive > 0 && max_exclusive <= i32::MAX as u32);
        ((self.internal_sample() as f64 / MBIG as f64) * max_exclusive as f64) as u32
    }
}

pub fn next_int(algorithm: &str, stream: &mut RngStreamState, max_exclusive: u32) -> u32 {
    let result = match algorithm {
        "dotnet_system_random_v1" => {
            let mut rng = DotNetSeededRandom::new(stream.seed);
            for _ in 0..stream.counter {
                rng.internal_sample();
            }
            rng.next_int(max_exclusive)
        }
        "xoshiro256_star_star_v1" => {
            let mut rng = Xoshiro256StarStar::new(stream.seed);
            for _ in 0..stream.counter {
                rng.next_u64();
            }
            rng.next_int(max_exclusive)
        }
        other => panic!("unsupported RNG algorithm {other}"),
    };
    stream.counter += 1;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_seeded_system_random_known_vector() {
        let mut rng = DotNetSeededRandom::new(1);
        assert_eq!(rng.internal_sample(), 534_011_718);
        assert_eq!(rng.internal_sample(), 237_820_880);
        assert_eq!(rng.internal_sample(), 1_002_897_798);
    }

    #[test]
    fn counter_reconstruction_is_branchable() {
        let mut stream = RngStreamState {
            seed: 1,
            counter: 2,
        };
        assert_eq!(
            next_int("dotnet_system_random_v1", &mut stream, i32::MAX as u32),
            1_002_897_798
        );
        assert_eq!(stream.counter, 3);
    }

    #[test]
    fn xoshiro_counter_reconstruction_is_branchable() {
        let mut direct = RngStreamState {
            seed: 1,
            counter: 0,
        };
        let values = (0..4)
            .map(|_| next_int("xoshiro256_star_star_v1", &mut direct, 1000))
            .collect::<Vec<_>>();
        let mut restored = RngStreamState {
            seed: 1,
            counter: 3,
        };
        assert_eq!(
            next_int("xoshiro256_star_star_v1", &mut restored, 1000),
            values[3]
        );
        assert_eq!(values, vec![702, 520, 574, 391]);
    }
}
