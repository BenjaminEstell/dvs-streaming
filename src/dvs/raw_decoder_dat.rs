#![allow(dead_code)]

use crate::dvs::{DvsRawDecoder, DVSRawEvent};
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B4, B32, B14};
use std::io::{self, BufRead, BufReader, Read, Seek};


#[bitfield]
#[derive(Clone)]
struct RawEvent {
    timestamp: B32,
    polarity: B4,
    x: B14,
    y: B14,
}

struct Metadata {
    sensor_width: i32,
    sensor_height: i32,
}

impl Default for Metadata {
    fn default() -> Self {
        Metadata {
            sensor_width: -1,
            sensor_height: -1,
        }
    }
}

type Timestamp = u64;

pub struct DVSRawDecoderDat<R: Read + BufRead + Seek> {
    reader: BufReader<R>,
    buffer_read: Vec<RawEvent>,
}

impl<R: Read + BufRead + Seek> DvsRawDecoder<R> for DVSRawDecoderDat<R> {
    fn new(reader: R) -> Self {
        let _buffer_read: Vec<u8> = vec![0; std::mem::size_of::<RawEvent>()];

        Self {
            reader: BufReader::new(reader),
            buffer_read: vec![RawEvent::new()],
        }
    }

    fn read_header(&mut self) -> anyhow::Result<Vec<String>> {
        // Copy header
        let mut header: Vec<String> = Vec::new();
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line)?;
            // Add line to header
            header.push(line.clone());
            if !line.contains("%") {
                break;
            }
        }

        let mut metadata = Metadata::default();
        let mut first_char = [0; 1];
        let reader = self.reader.get_mut();

        loop {
            reader.read_exact(&mut first_char)?;
            // if the first character ist a %, read the rest of the line
            if first_char == ['%' as u8] {
                // read the rest of the line
                let mut line: String = String::new();
                reader.read_line(&mut line)?;
                eprintln!("line: {}", line);
                // if this is the end of the header, break
                if !line.contains("%"){
                    break;
                } else if line.starts_with("% width ") {
                    println!("width: {}", line[8..].trim());
                    metadata.sensor_width = line[8..].trim().parse().unwrap();
                } else if line.starts_with("% height ") {
                    print!("height: {}", line[9..].trim());
                    metadata.sensor_height = line[9..].trim().parse().unwrap();
                }
            } else {
                // Move the reader back one byte if we didn't have a "%" line
                reader.seek(io::SeekFrom::Current(-1))?;
                break;
            }
        }

        if metadata.sensor_width > 0 && metadata.sensor_height > 0 {
            println!(
                "%geometry:{},{}",
                metadata.sensor_width, metadata.sensor_height
            );
        }

        // skip the event type and size details
        let mut line: String = String::new();
        let _ = reader.read_line(&mut line)?;

        Ok(header)
    }


    // fn read_event(&mut self) -> anyhow::Result<Option<DVSEvent>> {
    //     loop {
    //         self.reader.read_exact(unsafe {
    //             std::slice::from_raw_parts_mut(self.buffer_read.as_mut_ptr() as *mut u8, 
    //             std::mem::size_of::<RawEvent>())})?;

    //         let raw_event = self.buffer_read.as_ptr();
    //         return Ok(Some(DVSEvent {
    //             timestamp: unsafe { (*raw_event).timestamp() as u64 },
    //             x: unsafe { (*raw_event).x() as u16 },
    //             y: unsafe { (*raw_event).y() as u16 },
    //             polarity: unsafe { (*raw_event).polarity() as u8 },
    //         }));
    //     }

    // }

    fn read_event(&mut self) -> anyhow::Result<Option<DVSRawEvent>> {
        Ok(None)
    }
}
