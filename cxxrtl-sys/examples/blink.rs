use cxxrtl_sys::cxxrtl;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::process::Command;
use std::{env, ptr, slice};
use c_str_macro::c_str;
use std::io::prelude::*;

pub fn build(sources: &[PathBuf], dest: &Path) {
    let output = Command::new("yosys-config")
        .args(&["--datdir/include"])
        .output()
        .expect("failed to get yosys include dir");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let include = stdout.trim();

    let tmp = env::temp_dir().join("cxxrtl.cpp");

    let capi = Path::new(include).join("backends/cxxrtl/cxxrtl_capi.cc");
    let cvcd = Path::new(include).join("backends/cxxrtl/cxxrtl_vcd_capi.cc");

    Command::new("yosys")
        .args(&["-p", &format!("write_cxxrtl {}", tmp.to_string_lossy())])
        .args(sources)
        .status()
        .expect("failed generate cxxrtl code");

    Command::new("clang++")
        .args(&["-g", "-O3", "-fPIC", "-shared", "-std=c++14"])
        .arg(format!("-I{}", include))
        .arg(capi).arg(cvcd).arg(tmp).arg("-o").arg(dest)
        .status()
        .expect("failed generate cxxrtl code");
}

fn main() {
    let sources: Vec<PathBuf> = env::args().skip(1).map(PathBuf::from).collect();
    let lib = env::temp_dir().join("blink.so");
    build(sources.as_slice(), &lib);
    unsafe {
        let sim = cxxrtl::new(lib).expect("failed to load");
        let top = sim.cxxrtl_design_create();
        let blink = sim.cxxrtl_create(top);
        let vcd = sim.cxxrtl_vcd_create();
        sim.cxxrtl_vcd_timescale(vcd, 10, c_str!("ns").as_ptr());
        sim.cxxrtl_vcd_add_from(vcd, blink);
        let mut trace_file = File::create("trace.vcd").expect("unable to open trace file");

        let clk = sim.cxxrtl_get(blink, c_str!("clk").as_ptr());
        let led = sim.cxxrtl_get(blink, c_str!("led").as_ptr());

        sim.cxxrtl_step(blink);
        sim.cxxrtl_vcd_sample(vcd, 0);
        let mut prev_led = 0;
        for cycle in 0..1000 {
            *(*clk).next = 0;
            sim.cxxrtl_step(blink);
            sim.cxxrtl_vcd_sample(vcd, cycle*2 + 0);
            *(*clk).next = 1;
            sim.cxxrtl_step(blink);
            sim.cxxrtl_vcd_sample(vcd, cycle*2 + 1);

            let curr_led = *(*led).curr;
            if prev_led != curr_led {
                println!("cycle {}, led {}", cycle, curr_led);
                prev_led = curr_led;
            }
            {
                let mut len = 0;
                let mut buf = ptr::null();
                sim.cxxrtl_vcd_read(vcd, &mut buf, &mut len);
                let data = slice::from_raw_parts(buf as *const u8, len as usize);
                trace_file.write_all(data);
            }
        }
    }
}
