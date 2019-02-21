extern crate bytes;
extern crate futures;
extern crate getopts;
extern crate pbr;
extern crate tokio;

use bytes::BytesMut;
use futures::future::{loop_fn, Loop};
use getopts::Options;
use pbr::ProgressBar;
use std::{
    env,
    str::FromStr,
    time::Instant,
    fs,
    cmp,
};

use tokio::{
    fs::File,
    prelude::*,
};

fn usage(opts: Options) {
    let brief = ("Usage: dd [options] <INFILE> <OUTFILE>").to_string();
    print!("{}", opts.usage(&brief));
}

fn main() {
    let mut opts = Options::new();
    opts.optopt("b", "blocksize", "Block size in bytes", "BS");
    opts.optopt("c", "count", "Number of blocks to copy", "COUNT");
    opts.optflag("h", "help", "print this help menu");

    let matches = match opts.parse(env::args().skip(1)) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };

    if matches.opt_present("h") {
        usage(opts);
        return;
    }

    let bs = match matches.opt_str("b") {
        Some(v) => usize::from_str(v.as_str()).unwrap(),
        None => 512,
    };

    let mut dbs = BytesMut::with_capacity(bs);
    let count = match matches.opt_str("c") {
        Some(v) => usize::from_str(v.as_str()).unwrap(),
        None => 1,
    };
    let infile = &matches.free[0];
    let outfile = &matches.free[1];

    let metadata = std::fs::metadata(infile.as_str()).unwrap();
    let real_count = (metadata.len() as usize + bs) / bs;
    let status_count = cmp::min(count, real_count);
    let mut pb = ProgressBar::new(status_count as u64);
    let std_file = fs::File::create(outfile.as_str()).unwrap();
    let mut w_file = File::from_std(std_file);

    let now = Instant::now();
    let task = File::open(String::from(infile.as_str()))
        .and_then(move |mut file| {
            let mut eof = false;
            loop_fn((0, 0), move |(mut n, mut readed)| {
                file.read_buf(&mut dbs)
                    .and_then(|num| {
                        let n = match num {
                            Async::Ready(n) => n,
                            _ => panic!(),
                        };
                        readed += n;
                        if n == 0 {
                            eof = true;
                        }
                        Ok(n)
                    })
                    .and_then(|num| {
                        if readed == bs || eof {
                            dbs.truncate(readed);
                            w_file
                                .poll_write(&dbs)
                                .map(|res|
                                    match res {
                                    Async::Ready(n) => {
                                        if n != readed {
                                            panic!()
                                        } else {
                                            dbs.clear();
                                        }
                                    }
                                    _ => panic!(),
                                })
                                .map_err(|err| eprintln!("IO error: {:?}", err))
                                .unwrap();
                        }
                        Ok(num)
                    })
                    .and_then(|num| {
                        if readed < bs && !eof {
                            return Ok(Loop::Continue((n, num)));
                        }

                        if readed == bs {
                            n += 1;
                            pb.inc();
                        }
                        if n == count || eof {
                            pb.finish();
                            return w_file.poll_flush().and_then(|_| Ok(Loop::Break((n, num))));
                        }
                        Ok(Loop::Continue((n, 0)))
                    })
            })
            .and_then(move |_| {
                let delta = now.elapsed().as_millis();
                println!("Done! use {} msec", delta);
                Ok(())
            })
        })
        .map_err(|err| eprintln!("IO error: {:?}", err));

    tokio::run(task);
}
