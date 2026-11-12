//! Step 6 measurements: median of 200 runs, in microseconds.

use rquickjs::{Context, Module, Runtime, WriteOptions};
use std::time::{Duration, Instant};

const PRELUDE: &str = include_str!("../js/dist/prelude.js");
const RUNS: usize = 200;

fn median(mut samples: Vec<Duration>) -> u128 {
    samples.sort();
    samples[samples.len() / 2].as_micros()
}

fn main() {
    let create: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let t = Instant::now();
            let rt = Runtime::new().unwrap();
            let ctx = Context::full(&rt).unwrap();
            let elapsed = t.elapsed();
            drop((ctx, rt));
            elapsed
        })
        .collect();

    let from_source: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let rt = Runtime::new().unwrap();
            let ctx = Context::full(&rt).unwrap();
            ctx.with(|ctx| {
                let t = Instant::now();
                ctx.eval::<(), _>(PRELUDE).unwrap();
                let elapsed = t.elapsed();
                assert!(ctx.eval::<bool, _>("typeof chai.expect === 'function' && typeof _.sum === 'function'").unwrap());
                elapsed
            })
        })
        .collect();

    // rquickjs 0.14 exposes bytecode only for modules (Module::write / Module::load);
    // the IIFE prelude is valid module source, so it is compiled as a module.
    let bytecode = {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| Module::declare(ctx, "prelude", PRELUDE).unwrap().write(WriteOptions::default()).unwrap())
    };

    let from_bytecode: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let rt = Runtime::new().unwrap();
            let ctx = Context::full(&rt).unwrap();
            ctx.with(|ctx| {
                let t = Instant::now();
                // SAFETY: bytes were produced by Module::write of the same engine build above.
                let module = unsafe { Module::load(ctx.clone(), &bytecode) }.unwrap();
                let (_module, done) = module.eval().unwrap();
                done.finish::<()>().unwrap();
                let elapsed = t.elapsed();
                assert!(ctx.eval::<bool, _>("typeof chai.expect === 'function' && typeof _.sum === 'function'").unwrap());
                elapsed
            })
        })
        .collect();

    println!("prelude source: {} bytes, bytecode: {} bytes", PRELUDE.len(), bytecode.len());
    println!("runtime+context create (no prelude): {} us", median(create));
    println!("prelude load from source:           {} us", median(from_source));
    println!("prelude load from bytecode:         {} us", median(from_bytecode));
}
