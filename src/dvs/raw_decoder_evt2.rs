#![allow(dead_code)]

use crate::dvs::{DVSEvent, DvsRawDecoder, DVSRawEvent};
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B11, B28, B4};
use modular_bitfield::specifiers::B6;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

/* 
This file implements an EVT2 raw event decoder for Dynamic Vision Sensor (DVS) data streams.
It provides types and logic to parse EVT2-formatted event files, extract sensor metadata, and decode individual events.
*/

type Timestamp = u64;
// An enum representing the possible event types in EVT2 streams:
#[derive(Debug, Clone, Copy)]
enum EventTypes {
    CdOff = 0x0,        // Change Detection event, polarity off.
    CdOn = 0x1,         // Change Detection event, polarity on.
    EvtTimeHigh = 0x8,  // EVT_TIME_HIGH event, used for timestamp synchronization.
    ExtTrigger = 0xA,   // External trigger event
}

impl From<u8> for EventTypes {
    fn from(value: u8) -> Self {
        match value {
            0x0 => EventTypes::CdOff,
            0x1 => EventTypes::CdOn,
            0x8 => EventTypes::EvtTimeHigh,
            0xA => EventTypes::ExtTrigger,
            _ => EventTypes::ExtTrigger,
        }
    }
}

// A bitfield struct representing the raw 32 bits of an event in EVT2 format
#[bitfield]
#[derive(Clone, Debug)]
struct RawEvent {
    r#type: B4,
    pad: B28
}

// A bitfield struct for EVT_TIME_HIGH events, which contain a timestamp
#[bitfield]
struct RawEventTime {
    r#type: B4,     // Event type
    timestamp: B28  // Event timestamp
}

// A bitfield struct for Change Detection events, which contain pixel coordinates, polarity, and timestamp
#[bitfield]
struct RawEventCD {
    r#type: B4,     // Event type
    timestamp: B6,  // Event timestamp
    x: B11,         // Pixel X coordinate
    y: B11,         // Pixel Y coordinate
}

// Conversion from bytes to RawEvent
impl From<[u8; 4]> for RawEvent {
    fn from(value: [u8; 4]) -> Self {
        let mut event = RawEvent::new();
        event.set_type(value[3] >> 4);
        event.set_pad(
            ((value[3] & 0x0F) as u32) << 24
            | (value[2] as u32) << 16
            | (value[1] as u32) << 8
            | value[0] as u32,
        );
        event
    }
}

// Conversion from Raw event to RawEventTime
impl From<RawEvent> for RawEventTime {
    fn from(event: RawEvent) -> Self {
        let event_time = RawEventTime::new()
            .with_timestamp(event.pad())
            .with_type(event.r#type());
        event_time
    }
}

// Conversion from RawEvent to RawEventCD
impl From<RawEvent> for RawEventCD {
    fn from(event: RawEvent) -> Self {
        let event_cd = RawEventCD::new()
            .with_y((event.pad()& 0x7FF) as u16)
            .with_x(((event.pad() >> 11) & 0x7FF) as u16)
            .with_timestamp((event.pad() >> 22) as u8) 
            .with_type(event.r#type());
        event_cd
    }
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

// The main decoder struct. Wraps a buffered reader and maintains state for timestamp base and event parsing.
pub struct DVSRawDecoderEvt2<R: Read + BufRead + Seek> {
    reader: BufReader<R>,
    first_time_base_set: bool,
    current_time_base: u64,
    n_time_high_loop: u64,
    buffer_read: Vec<[u8; 4]>,
}

impl<R: Read + BufRead + Seek> DvsRawDecoder<R> for DVSRawDecoderEvt2<R> {
    // Creates a new DVSRawDecoderEvt2 instance with a buffered reader
    fn new(reader: R) -> Self {
        let _buffer_read: Vec<u8> = vec![0; std::mem::size_of::<[u8; 4]>()];

        Self {
            reader: BufReader::new(reader),
            first_time_base_set: false,
            current_time_base: 0,
            n_time_high_loop: 0,
            buffer_read: vec![unsafe { std::mem::zeroed() }],
        }
    }

    // Reads the header of the EVT2 file, extracting metadata and setting the initial time base
    // Returns the header as a vector of strings
    fn read_header(&mut self) -> anyhow::Result<Vec<String>> {
        // Copy header
        let mut header: Vec<String> = Vec::new();
        // Reset the reader to the beginning
        self.reader.seek(SeekFrom::Start(0))?;
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line)?;
            // Add line to header
            header.push(line.clone());
            if line.contains("% end") {
                break;
            }
        }

        let mut metadata = Metadata::default();
        let mut first_char = [0; 1];
        // Reset the reader to the beginning
        self.reader.seek(SeekFrom::Start(0))?;

        loop {
            self.reader.read_exact(&mut first_char)?;
            if first_char == ['%' as u8] {
                // read the rest of the line
                let mut line: String = String::new();
                self.reader.read_line(&mut line)?;
                //eprintln!("line: {}", line);
                if line == " end\n" {
                    break;
                } else if line.starts_with(" format ") {
                    let format_str = &line[8..];
                    let mut parts = format_str.split(';');
                    if parts.next().unwrap() != "EVT2" {
                        eprintln!("Error: detected non-EVT2 input file");
                        return Ok(header);
                    }
                    for option in parts {
                        let mut kv = option.split('=');
                        let name = kv.next().unwrap();
                        let value = kv.next().unwrap();
                        if name == "width" {
                            metadata.sensor_width = value[..value.len() - 1].parse().unwrap();
                        } else if name == "height" {
                            metadata.sensor_height = value.parse().unwrap();
                        }
                    }
                } else if line.starts_with(" geometry ") {
                    let geometry_str = &line[10..line.len() - 1];
                    let mut parts = geometry_str.split('x');
                    metadata.sensor_width = parts.next().unwrap().parse().unwrap();
                    metadata.sensor_height = parts.next().unwrap().parse().unwrap();
                } else if line.starts_with(" evt ") {
                    if &line[5..] != "2.0\n" {
                        dbg!(line[5..].to_string());
                        eprintln!("Error: detected non-EVT2 input file");
                        return Ok(header);
                    }
                }
            } else {
                // Move the reader back one byte if we didn't have the "% end\n" line
                self.reader.seek_relative(-1)?;
                break;
            }
        }

        if metadata.sensor_width > 0 && metadata.sensor_height > 0 {
            println!(
                "%geometry:{},{}",
                metadata.sensor_width, metadata.sensor_height
            );
        }


        loop {
            // First, skip any events until we get one of the type EVT_TIME_HIGH
            self.reader.read_exact(unsafe {
                std::slice::from_raw_parts_mut(
                    self.buffer_read.as_mut_ptr() as *mut u8,
                    std::mem::size_of::<RawEvent>(),
                )
            })?;
            
            let raw_event = RawEvent::from(self.buffer_read[0]);
            match raw_event.r#type() {
                x if x == EventTypes::EvtTimeHigh as u8 => {
                    let ev_time_high = RawEventTime::from(raw_event);
                    self.current_time_base = (ev_time_high.timestamp() as Timestamp) << 6;
                    self.first_time_base_set = true;
                    break;
                }
                _ => {}
            }
        }
        Ok(header)
    }

    
    // Reads the next event from the EVT2 file, returning it as a DVSRawEvent
    fn read_event(&mut self) -> anyhow::Result<Option<DVSRawEvent>> {
        loop {
            // Read event
            self.reader.read_exact(unsafe {
                std::slice::from_raw_parts_mut(
                    self.buffer_read.as_mut_ptr() as *mut u8,
                    std::mem::size_of::<RawEvent>(),
                )
            })?;

            let raw_event = RawEvent::from(self.buffer_read[0]);
            match raw_event.r#type() {
                x if x == EventTypes::CdOff as u8 => {
                    let ev_cd = RawEventCD::from(raw_event);
                    let t = self.current_time_base + ev_cd.timestamp() as Timestamp;
                    return Ok(Some(DVSRawEvent::CD(DVSEvent {
                        timestamp: t,
                        x: ev_cd.x(),
                        y: ev_cd.y(),
                        polarity: 0,
                    })));
                }
                x if x == EventTypes::CdOn as u8 => {
                    let ev_cd = RawEventCD::from(raw_event);
                    let t = self.current_time_base + ev_cd.timestamp() as Timestamp;
                    return Ok(Some(DVSRawEvent::CD(DVSEvent {
                        timestamp: t,
                        x: ev_cd.x(),
                        y: ev_cd.y(),
                        polarity: 1,
                    })));
                }
                x if x == EventTypes::EvtTimeHigh as u8 => {
                    const MAX_TIMESTAMP_BASE: Timestamp = ((1 << 28) - 1) << 6;
                    const TIME_LOOP: Timestamp = MAX_TIMESTAMP_BASE + (1 << 6);
                    const LOOP_THRESHOLD: Timestamp = 10 << 6;

                    let ev_time_high = RawEventTime::from(raw_event);
                    let mut new_time_base = (ev_time_high.timestamp() as Timestamp) << 6;
                    new_time_base += self.n_time_high_loop * TIME_LOOP;

                    if self.current_time_base > new_time_base
                        && self.current_time_base - new_time_base
                            >= MAX_TIMESTAMP_BASE - LOOP_THRESHOLD
                    {
                        new_time_base += TIME_LOOP;
                        self.n_time_high_loop += 1;
                    }

                    self.current_time_base = new_time_base;
                    return Ok(Some(DVSRawEvent::TimeHigh { timestamp: ev_time_high.timestamp() as Timestamp }));

                }
                x if x == EventTypes::ExtTrigger as u8 => {
                    // Ignore for now--we're not doing anything with triggers.
                }
                _ => {
                    //println!("Unknown event type: {}", unsafe { (*raw_event).r#type() });
                }
            }
        }
    }
}
