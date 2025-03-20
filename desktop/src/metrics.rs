use crate::error::Result;

use std::fs::File;
use std::io::{BufWriter, Write};

use libclient_rs::{Metric, MetricsJSONProducer};

pub struct MetricsProcessor {
    json_producer: MetricsJSONProducer,
    out: BufWriter<File>,
}

impl MetricsProcessor {
    pub fn new() -> Result<Self> {
        let json_producer = MetricsJSONProducer::new();
        let file = File::create("metrics.json")?;
        let out = BufWriter::new(file);
        Ok(Self { json_producer, out })
    }

    pub fn on_metric(&mut self, metric: Metric) {
        let lines = self.json_producer.push(&metric);
        for mut data in lines {
            data.push(b'\n');
            if let Err(err) = self.out.write_all(&data) {
                log::warn!("Could not write metric: {err:?}");
            }
        }
    }
}
