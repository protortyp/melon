#[cfg(target_os = "linux")]
use cgroups_rs::cgroup_builder::*;
#[cfg(target_os = "linux")]
use cgroups_rs::hierarchies::auto;
#[cfg(target_os = "linux")]
use cgroups_rs::*;
#[cfg(target_os = "linux")]
use sysinfo::System;

use melon_common::proto;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct CGroups {
    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    job_id: u64,
    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    cgroup: Option<Cgroup>,
    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)]
    cgroup: Option<()>, // dummy field for macos/win
}

impl Drop for CGroups {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(cg) = self.cgroup.take() {
            cg.delete().expect("Could not remove cgroup");
        }
    }
}

impl CGroups {
    #[cfg(target_os = "linux")]
    pub fn create_group_guard(
        job_id: u64,
        pid: u32,
        resources: proto::RequestedResources,
    ) -> Result<Self, Box<dyn Error>> {
        let cgroup = Self::create_group(job_id, resources)?;
        Self::assign_pid_to_group(&cgroup, pid)?;
        Ok(Self {
            job_id,
            cgroup: Some(cgroup),
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn create_group_guard(
        _job_id: u64,
        _pid: u32,
        _resources: proto::RequestedResources,
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Self { cgroup: Some(()) })
    }

    #[cfg(target_os = "linux")]
    fn create_group(
        job_id: u64,
        resources: proto::RequestedResources,
    ) -> Result<Cgroup, Box<dyn Error>> {
        let cpu_count = resources.cpu_count as u64;
        let memory_in_bytes = resources.memory;

        let mut system = System::new_all();
        system.refresh_all();
        let n_cpus = system.cpus().len() as u64;

        let cpu_shares = if n_cpus > 0 {
            cpu_count * 100 / n_cpus
        } else {
            100
        };

        let hier = auto();
        let cgroup = CgroupBuilder::new(&format!("melon-{}", job_id))
            .cpu()
            .shares(cpu_shares)
            .done()
            .memory()
            .memory_hard_limit(memory_in_bytes as i64)
            .done()
            .build(hier)?;

        Ok(cgroup)
    }

    #[cfg(target_os = "linux")]
    fn assign_pid_to_group(cgroup: &Cgroup, pid: u32) -> Result<(), Box<dyn Error>> {
        let cpu_controller = cgroup
            .controller_of::<cgroups_rs::cpu::CpuController>()
            .ok_or("CPU controller not found")?;
        cpu_controller.add_task_by_tgid(&CgroupPid::from(pid as u64))?;

        let memory_controller = cgroup
            .controller_of::<cgroups_rs::memory::MemController>()
            .ok_or("Memory controller not found")?;
        memory_controller.add_task_by_tgid(&CgroupPid::from(pid as u64))?;
        Ok(())
    }
}
