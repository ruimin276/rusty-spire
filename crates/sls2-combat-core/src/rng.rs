use crate::state::RngStreamState;
use blake3::Hasher;

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

pub fn next_int(algorithm: &str, stream: &mut RngStreamState, max_exclusive: u32) -> u32 {
    let result = match algorithm {
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

pub fn domain_seed(base_seed: u32, stream_name: &str) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(b"sls2-combat-rng-domain-v1\0");
    hasher.update(&base_seed.to_le_bytes());
    hasher.update(stream_name.as_bytes());
    let digest = hasher.finalize();
    u32::from_le_bytes(digest.as_bytes()[0..4].try_into().expect("four bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn named_stream_derivation_is_stable_and_separated() {
        assert_eq!(domain_seed(1, "shuffle"), 3_114_687_082);
        assert_eq!(domain_seed(1, "monster_ai"), 1_831_361_556);
        assert_ne!(domain_seed(1, "shuffle"), domain_seed(2, "shuffle"));
    }
}
