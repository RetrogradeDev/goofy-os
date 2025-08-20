use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use crate::allocator::{HEAP_SIZE, HEAP_START};

pub static mut STACK_BASE: usize = 0;

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub architecture: String,
    pub processor_vendor: String,
    pub processor_model: String,
    pub base_frequency: Option<u16>,
    pub max_frequency: Option<u16>,
    pub heap_size: usize,
    pub heap_start: usize,
    pub heap_used: usize,
    pub stack_size: usize,
    pub cpu_features: Vec<String>,
}

impl SystemInfo {
    pub fn gather() -> Self {
        let heap_info = get_heap_info();
        let cpu_info = get_cpu_info();

        SystemInfo {
            os_name: "goofy-os".to_string(),
            os_version: "v0.1.0".to_string(), // This could be read from a config file
            architecture: "x86_64".to_string(),
            processor_vendor: cpu_info.vendor_id,
            processor_model: cpu_info.model,
            base_frequency: cpu_info.base_frequency,
            max_frequency: cpu_info.max_frequency,

            heap_size: HEAP_SIZE,
            heap_start: HEAP_START,
            heap_used: heap_info.used_bytes,

            stack_size: 4096 * 5, // From gdt.rs STACK_SIZE
            cpu_features: cpu_info.features,
        }
    }
}

pub struct HeapInfo {
    pub used_bytes: usize,
    pub free_bytes: usize,
    pub total_bytes: usize,
}

pub struct CpuInfo {
    pub features: Vec<String>,
    pub vendor_id: String,
    pub model: String,
    pub base_frequency: Option<u16>,
    pub max_frequency: Option<u16>,
}

fn get_heap_info() -> HeapInfo {
    // For now, we'll return estimated values
    // The linked_list_allocator doesn't provide detailed statistics
    // In a real implementation, you'd need to track allocations manually

    // We can estimate usage based on typical allocation patterns
    let estimated_used = estimate_heap_usage();

    HeapInfo {
        used_bytes: estimated_used,
        free_bytes: HEAP_SIZE.saturating_sub(estimated_used),
        total_bytes: HEAP_SIZE,
    }
}

fn estimate_heap_usage() -> usize {
    // Very rough estimation - in a real system you'd track this properly
    // We'll assume some usage based on our applications
    HEAP_SIZE / 4 // Assume 25% usage as a rough estimate
}

fn get_cpu_info() -> CpuInfo {
    use raw_cpuid::CpuId;

    let cpuid = CpuId::new();

    let vendor_id = if let Some(info) = cpuid.get_vendor_info() {
        info.as_str().to_string()
    } else {
        "Unknown".to_string()
    };

    let model = if let Some(info) = cpuid.get_processor_brand_string() {
        info.as_str().to_string()
    } else {
        "Unknown".to_string()
    };

    let (base_frequency, max_frequency) = if let Some(info) = cpuid.get_processor_frequency_info() {
        (
            Some(info.processor_base_frequency()),
            Some(info.processor_max_frequency()),
        )
    } else {
        (None, None)
    };

    let mut features = Vec::new();

    if let Some(finfo) = cpuid.get_feature_info() {
        if finfo.has_sse() {
            features.push("SSE".to_string());
        }
        if finfo.has_sse2() {
            features.push("SSE2".to_string());
        }
        if finfo.has_sse3() {
            features.push("SSE3".to_string());
        }
        if finfo.has_fma() {
            features.push("FMA".to_string());
        }
        if finfo.has_mmx() {
            features.push("MMX".to_string());
        }
        if finfo.has_fpu() {
            features.push("FPU".to_string());
        }
    }

    if let Some(extended) = cpuid.get_extended_feature_info() {
        if extended.has_avx2() {
            features.push("AVX2".to_string());
        }
        if extended.has_bmi1() {
            features.push("BMI1".to_string());
        }
        if extended.has_bmi2() {
            features.push("BMI2".to_string());
        }
    }

    CpuInfo {
        features,
        vendor_id,
        model,
        base_frequency,
        max_frequency,
    }
}

pub fn format_memory_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} bytes", bytes)
    }
}

// Function to get stack pointer (for stack usage estimation)
#[inline(always)]
pub fn get_stack_pointer() -> u64 {
    let sp: u64;
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) sp, options(nomem, nostack, preserves_flags));
    }
    sp
}

pub fn estimate_stack_usage() -> usize {
    // This is a very rough estimation
    // In a real kernel, you'd track stack bounds more precisely
    let current_sp = get_stack_pointer();

    // Estimate based on how far we are from a "typical" stack start
    // This is not precise but gives a rough idea
    let estimated_used = (unsafe { STACK_BASE } - current_sp as usize);

    // Clamp to reasonable values
    estimated_used.min(4096 * 5) // Max our known stack size
}
