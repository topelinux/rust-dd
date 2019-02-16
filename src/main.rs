extern crate futures;
extern crate bytes;
extern crate getopts;
extern crate tokio;

use bytes::BytesMut;
use futures::future::{loop_fn, Loop};
use getopts::Options;
use std::env;
use std::str::FromStr;
use tokio::fs::File;
use tokio::prelude::*;
use std::time::{Instant};

fn usage(opts: Options) {
    let brief = format!("Usage: dd [options] <INFILE> <OUTFILE>");
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

    let std_file = std::fs::File::create(outfile.as_str()).unwrap();
    let mut w_file = File::from_std(std_file);

    let now = Instant::now();
    let task = File::open(String::from(infile.as_str()))
                .and_then(move |mut file| {
                    loop_fn((0, 0), move |(mut n, mut readed)| {
                            file.read_buf(&mut dbs)
                            .and_then(|num| {
                                match num {
                                    Async::Ready(n) => readed += n,
                                    _ => panic!(),
                                };
                                Ok(readed)
                            })
                            .and_then(|num| {
                                if num == bs {
                                    dbs.truncate(bs);
                                    w_file.poll_write(&dbs).map(|res| {
                                        match res {
                                                Async::Ready(n) => {
                                                    if n != bs {
                                                        panic!()
                                                    }
                                                },
                                                _ => panic!()
                                            }
                                    })
                                    .map_err(|err| eprintln!("IO error: {:?}", err)).unwrap();
                                }
                                Ok(num)
                            })
                            .and_then(|num| {
                                if num == bs {
                                    n += 1;
                                }
                                if n == count {
                                    return w_file.poll_flush().and_then(|_| Ok(Loop::Break((n, num))))
                                }
                                Ok(Loop::Continue((n, num)))
                            })
                    }).and_then(move |_| { let delta = now.elapsed().as_millis(); println!("Done! use {} msec", delta); Ok(())})
                })
                .map_err(|err| eprintln!("IO error: {:?}", err));

    tokio::run(task);
}
