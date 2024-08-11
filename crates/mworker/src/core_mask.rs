/// # CoreMask Module
///
/// This module provides the `CoreMask` struct, which represents a bitmap for managing CPU core allocations.
/// It allows for efficient allocation and deallocation of CPU cores, typically used in conjunction with
/// cgroups and cpusets in system resource management.
///
/// The `CoreMask` uses a 64-bit unsigned integer to represent up to 64 CPU cores, where each bit
/// corresponds to a core. A set bit (1) indicates an allocated core, while an unset bit (0) represents
/// an available core.
///
/// Why is this so complicated? I just felt like it...
///
/// ## Features
///
/// - Allocate a specified number of cores
/// - Free previously allocated cores
/// - Convert core masks to human-readable strings
/// - Query available and allocated cores
///
/// ## Examples
///
/// ### Creating a CoreMask and Allocating Cores
///
/// ```
/// use mworker::core_mask::CoreMask;
///
/// let mut mask = CoreMask::new(8);  // System with 8 cores
///
/// // Allocate 3 cores
/// let allocation = mask.allocate(3).unwrap();
/// assert_eq!(allocation, 0b1110_0000);
///
/// // Allocate 2 more cores
/// let another_allocation = mask.allocate(2).unwrap();
/// assert_eq!(another_allocation, 0b0001_1000);
///
/// // Current state of the mask
/// assert_eq!(mask.get_allocated_cores(), 0b1111_1000);
/// ```
///
/// ### Freeing Allocated Cores
///
/// ```
/// use mworker::core_mask::CoreMask;
///
/// let mut mask = CoreMask::new(8);
/// let allocation = mask.allocate(4).unwrap();
/// assert_eq!(allocation, 0b1111_0000);
///
/// // Free the allocated cores
/// mask.free(allocation);
/// assert_eq!(mask.get_allocated_cores(), 0);
/// ```
///
/// ### Converting Mask to String
///
/// ```
/// use mworker::core_mask::CoreMask;
///
/// let mask = 0b1010_1010;
/// assert_eq!(CoreMask::mask_to_string(mask), "1,3,5,7");
/// ```
///
/// ### Querying Available Cores
///
/// ```
/// use mworker::core_mask::CoreMask;
///
/// let mut mask = CoreMask::new(8);
/// mask.allocate(2).unwrap();  // 1100_0000
///
/// let available = mask.get_available_core_ids(3).unwrap();
/// assert_eq!(available, 0b0011_1000);
/// ```
///
/// ## Implementation Details
///
/// The `CoreMask` struct uses a greedy allocation strategy, always trying to allocate cores from
/// the highest available core ID. This can lead to fragmentation over time, but ensures that
/// lower-numbered cores are kept free for as long as possible, which can be beneficial in some
/// system configurations.
///
/// Note that this implementation is limited to systems with up to 64 cores due to the use of a
/// `u64` for the internal representation. For systems with more cores, the implementation would
/// need to be adapted, possibly using a vector of `u64` or a different data structure.

#[derive(Debug)]
pub struct CoreMask {
    mask: u64,
    total_cores: u32,
}

impl CoreMask {
    pub fn new(total_cores: u32) -> Self {
        Self {
            mask: 0,
            total_cores,
        }
    }

    pub fn allocate(&mut self, cores_needed: u32) -> Option<u64> {
        println!("Allocate {} cores", cores_needed);
        if cores_needed == 0 || cores_needed > self.total_cores {
            return None;
        }

        let mut allocated_mask = 0u64;
        let mut count = 0;

        // start from the leftmost bit (most significant bit)
        for i in (0..self.total_cores).rev() {
            if self.mask & (1u64 << i) == 0 {
                allocated_mask |= 1u64 << i; // set bit in the allocated mask
                self.mask |= 1u64 << i; // set bit in the overall mask
                count += 1;

                if count == cores_needed {
                    return Some(allocated_mask);
                }
            }
        }

        // roll back the allocation if not enough were found
        self.mask &= !allocated_mask;
        None
    }

    pub fn free(&mut self, mask_to_free: u64) {
        self.mask &= !mask_to_free;
    }

    pub fn get_allocated_cores(&self) -> u64 {
        self.mask
    }

    pub fn get_available_core_ids(&self, cores_needed: u32) -> Option<u64> {
        if cores_needed == 0 || cores_needed > self.total_cores {
            return None;
        }

        let mut available_mask = 0u64;
        let mut count = 0;

        for i in (0..self.total_cores).rev() {
            if self.mask & (1u64 << i) == 0 {
                available_mask |= 1u64 << i;
                count += 1;
                if count == cores_needed {
                    return Some(available_mask);
                }
            }
        }

        None
    }

    pub fn mask_to_string(mask: u64) -> String {
        (0..64)
            .filter(|&i| mask & (1 << i) != 0)
            .map(|i| i.to_string())
            .collect::<Vec<String>>()
            .join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_mask_allocation_and_freeing() {
        let mut core_mask = CoreMask::new(8);

        // Initial state
        assert_eq!(core_mask.mask, 0b0000_0000);

        // 1) allocate 4 cores
        let allocation1 = core_mask.allocate(4).unwrap();
        println!(
            "{:0>64}",
            format!("{:b}", allocation1)
                .chars()
                .rev()
                .collect::<Vec<char>>()
                .chunks(4)
                .map(|chunk| chunk.iter().collect::<String>())
                .collect::<Vec<String>>()
                .join("_")
                .chars()
                .rev()
                .collect::<String>()
        );
        assert_eq!(allocation1, 0b1111_0000);
        assert_eq!(core_mask.mask, 0b1111_0000);

        // 2) allocate 1 core
        let allocation2 = core_mask.allocate(1).unwrap();
        assert_eq!(allocation2, 0b0000_1000);
        assert_eq!(core_mask.mask, 0b1111_1000);

        // 3) allocate 1 core
        let allocation3 = core_mask.allocate(1).unwrap();
        assert_eq!(allocation3, 0b0000_0100);
        assert_eq!(core_mask.mask, 0b1111_1100);

        // 4) free up allocation2
        core_mask.free(allocation2);
        assert_eq!(core_mask.mask, 0b1111_0100);

        // 5) allocate 2 cores
        let allocation5 = core_mask.allocate(2).unwrap();
        assert_eq!(allocation5, 0b0000_1010);
        assert_eq!(core_mask.mask, 0b1111_1110);

        // 6) allocate 3 cores - should fail
        assert!(core_mask.allocate(3).is_none());
        assert_eq!(core_mask.mask, 0b1111_1110); // Mask should remain unchanged

        // 7) free up allocation1
        core_mask.free(allocation1);
        assert_eq!(core_mask.mask, 0b0000_1110);
    }

    #[test]
    fn test_allocate_returns_none() {
        let mut core_mask = CoreMask::new(8);
        assert!(core_mask.allocate(0).is_none());
        assert!(core_mask.allocate(9).is_none());
    }

    #[test]
    fn test_allocate_returns_some() {
        let mut core_mask = CoreMask::new(8);
        let result = core_mask.allocate(4);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), 0b1111_0000);
        let result = core_mask.allocate(2);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), 0b0000_1100);
    }

    #[test]
    fn test_mask_to_string_empty() {
        assert_eq!(CoreMask::mask_to_string(0), "");
    }

    #[test]
    fn test_mask_to_string_single_core() {
        assert_eq!(CoreMask::mask_to_string(1), "0");
        assert_eq!(CoreMask::mask_to_string(1 << 63), "63");
    }

    #[test]
    fn test_mask_to_string_consecutive_cores() {
        assert_eq!(CoreMask::mask_to_string(0b1111), "0,1,2,3");
        assert_eq!(CoreMask::mask_to_string(0b11110000), "4,5,6,7");
    }

    #[test]
    fn test_mask_to_string_scattered_cores() {
        assert_eq!(CoreMask::mask_to_string(0b10101010), "1,3,5,7");
        assert_eq!(CoreMask::mask_to_string(0b1000100010001), "0,4,8,12");
    }

    #[test]
    fn test_mask_to_string_all_cores() {
        assert_eq!(CoreMask::mask_to_string(u64::MAX), "0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63");
    }

    #[test]
    fn test_mask_to_string_high_cores() {
        assert_eq!(CoreMask::mask_to_string(0xF000000000000000), "60,61,62,63");
    }

    #[test]
    fn test_mask_to_string_mixed_range() {
        assert_eq!(
            CoreMask::mask_to_string(0b1010101010101010101010101010101),
            "0,2,4,6,8,10,12,14,16,18,20,22,24,26,28,30"
        );
    }

    #[test]
    fn test_get_available_core_ids_empty_mask() {
        let core_mask = CoreMask::new(8);
        let available = core_mask.get_available_core_ids(4).unwrap();
        assert_eq!(available, 0b1111_0000);
    }

    #[test]
    fn test_get_available_core_ids_partially_allocated() {
        let mut core_mask = CoreMask::new(8);
        core_mask.allocate(3).unwrap(); // 1110_0000
        let available = core_mask.get_available_core_ids(4).unwrap();
        assert_eq!(available, 0b0001_1110);
    }

    #[test]
    fn test_get_available_core_ids_fragmented() {
        let mut core_mask = CoreMask::new(8);
        core_mask.allocate(2).unwrap(); // 1100_0000
        core_mask.allocate(2).unwrap(); // 0011_0000
        let available = core_mask.get_available_core_ids(3).unwrap();
        assert_eq!(available, 0b0000_1110);
    }

    #[test]
    fn test_get_available_core_ids_not_enough() {
        let mut core_mask = CoreMask::new(8);
        core_mask.allocate(6).unwrap(); // 1111_1100
        assert!(core_mask.get_available_core_ids(3).is_none());
    }

    #[test]
    fn test_get_available_core_ids_exact_fit() {
        let mut core_mask = CoreMask::new(8);
        core_mask.allocate(5).unwrap(); // 1111_1000
        let available = core_mask.get_available_core_ids(3).unwrap();
        assert_eq!(available, 0b0000_0111);
    }

    #[test]
    fn test_get_available_core_ids_zero_cores() {
        let core_mask = CoreMask::new(8);
        assert!(core_mask.get_available_core_ids(0).is_none());
    }

    #[test]
    fn test_get_available_core_ids_more_than_total() {
        let core_mask = CoreMask::new(8);
        assert!(core_mask.get_available_core_ids(9).is_none());
    }

    #[test]
    fn test_get_available_core_ids_all_allocated() {
        let mut core_mask = CoreMask::new(8);
        core_mask.allocate(8).unwrap(); // 1111_1111
        assert!(core_mask.get_available_core_ids(1).is_none());
    }

    #[test]
    fn test_get_allocated_cores_empty() {
        let core_mask = CoreMask::new(8);
        assert_eq!(core_mask.get_allocated_cores(), 0);
    }

    #[test]
    fn test_get_allocated_cores_single_allocation() {
        let mut core_mask = CoreMask::new(8);
        core_mask.allocate(3).unwrap(); // 1110_0000
        assert_eq!(core_mask.get_allocated_cores(), 0b1110_0000);
    }

    #[test]
    fn test_get_allocated_cores_multiple_allocations() {
        let mut core_mask = CoreMask::new(8);
        core_mask.allocate(2).unwrap(); // 1100_0000
        core_mask.allocate(3).unwrap(); // 0011_1000
        assert_eq!(core_mask.get_allocated_cores(), 0b1111_1000);
    }

    #[test]
    fn test_get_allocated_cores_all_allocated() {
        let mut core_mask = CoreMask::new(8);
        core_mask.allocate(8).unwrap(); // 1111_1111
        assert_eq!(core_mask.get_allocated_cores(), 0b1111_1111);
    }

    #[test]
    fn test_get_allocated_cores_after_free() {
        let mut core_mask = CoreMask::new(8);
        let allocation = core_mask.allocate(4).unwrap(); // 1111_0000
        core_mask.allocate(2).unwrap(); // 0000_1100
        core_mask.free(allocation);
        assert_eq!(core_mask.get_allocated_cores(), 0b0000_1100);
    }

    #[test]
    fn test_get_allocated_cores_fragmented() {
        let mut core_mask = CoreMask::new(8);
        core_mask.allocate(2).unwrap(); // 1100_0000
        core_mask.allocate(2).unwrap(); // 0011_0000
        core_mask.allocate(1).unwrap(); // 0000_1000
        assert_eq!(core_mask.get_allocated_cores(), 0b1111_1000);
    }

    #[test]
    fn test_get_allocated_cores_single_core() {
        let mut core_mask = CoreMask::new(8);
        core_mask.allocate(1).unwrap(); // 1000_0000
        assert_eq!(core_mask.get_allocated_cores(), 0b1000_0000);
    }

    #[test]
    fn test_get_allocated_cores_allocate_free_allocate() {
        let mut core_mask = CoreMask::new(8);
        let allocation1 = core_mask.allocate(4).unwrap(); // 1111_0000
        core_mask.free(allocation1);
        core_mask.allocate(2).unwrap(); // 1100_0000
        assert_eq!(core_mask.get_allocated_cores(), 0b1100_0000);
    }
}
