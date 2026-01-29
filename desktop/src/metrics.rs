// Project Kyber: metrics.rs
// Copyright © 2022-2026 Kyber SAS
// SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0
//
// This file is both under dual license: AGPLv3 and a Commercial one.
//
// ----
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

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
