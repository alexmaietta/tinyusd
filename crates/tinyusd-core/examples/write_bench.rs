use std::time::Instant;

use tinyusd_core::{BaseType, OpenOptions, Prim, Stage, Value};

fn setup(bodies: usize) -> (Stage, Vec<Prim>) {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let prims = stage
        .edit(|s| {
            let mut prims = Vec::new();
            for i in 0..bodies {
                let p = s.define_prim(&format!("/World/Body{i}"), "Xform")?;
                s.create_attribute(p, "xformOp:translate", "double3", false)?;
                prims.push(p);
            }
            Ok(prims)
        })
        .unwrap();
    (stage, prims)
}

fn frame(stage: &mut Stage, prims: &[Prim], f: usize) {
    for (i, &p) in prims.iter().enumerate() {
        let v = Value::from_slice(BaseType::Double3, false, &[[i as f64, 0.0, f as f64]]).unwrap();
        stage.set(p, "xformOp:translate", &v, Some(f as f64)).unwrap();
    }
}

fn main() {
    let mut args = std::env::args().skip(1).map(|a| a.parse::<usize>().unwrap());
    let bodies = args.next().unwrap_or(1000);
    let frames = args.next().unwrap_or(10);

    let (mut stage, prims) = setup(bodies);
    let start = Instant::now();
    for f in 0..frames {
        frame(&mut stage, &prims, f);
    }
    let immediate = start.elapsed();

    let (mut stage, prims) = setup(bodies);
    let start = Instant::now();
    for f in 0..frames {
        stage.begin_edit().unwrap();
        frame(&mut stage, &prims, f);
        stage.commit_edit().unwrap();
    }
    let per_frame = start.elapsed();

    let (mut stage, prims) = setup(bodies);
    let start = Instant::now();
    stage.begin_edit().unwrap();
    for f in 0..frames {
        frame(&mut stage, &prims, f);
    }
    stage.commit_edit().unwrap();
    let whole = start.elapsed();

    let writes = (bodies * frames) as f64;
    for (name, t) in [
        ("immediate", immediate),
        ("batch per frame", per_frame),
        ("one batch", whole),
    ] {
        println!(
            "{name:>16}: {:>9.1} ms, {:>7.2} us per value",
            t.as_secs_f64() * 1e3,
            t.as_secs_f64() * 1e6 / writes
        );
    }
}
