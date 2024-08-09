#[derive(Debug, Clone)]
pub struct CGroups {
    job_id: u64,
}

// a helper method to turn the number of requested cores into digits
fn cores_to_ids(cpu_count: u32) -> String {
    (0..cpu_count)
        .map(|i| i.to_string())
        .collect::<Vec<String>>()
        .join(",")
}
