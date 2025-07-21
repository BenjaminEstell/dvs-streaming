use anyhow::Result;
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B1, B11, B12, B4, B7, B8, B28};
use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Seek};

use crate::dvs::{DVSEvent, DvsRawDecoder, DVSRawEvent};

type Timestamp = u64;

#[derive(Debug, Clone, Copy)]
enum EventTypes {
    EvtAddrY = 0x0,
    EvtAddrX = 0x2,
    VectBaseX = 0x3,
    Vect12= 0x4,
    Vect8 = 0x5,
    EvtTimeLow = 0x6,
    EvtTimeHigh = 0x8,
    ExtTrigger = 0xA,
}

impl From<u8> for EventTypes {
    fn from(value: u8) -> Self {
        match value {
            0x0 => EventTypes::EvtAddrY,
            0x2 => EventTypes::EvtAddrX,
            0x3 => EventTypes::VectBaseX,
            0x4 => EventTypes::Vect12,
            0x5 => EventTypes::Vect8,
            0x6 => EventTypes::EvtTimeLow,
            0x8 => EventTypes::EvtTimeHigh,
            0xA => EventTypes::ExtTrigger,
            _ => EventTypes::ExtTrigger,
        }
    }
}

#[bitfield]
struct RawEvent {
    pad: B12,
    r#type: B4,
}

#[bitfield]
struct RawEventTime {
    timestamp: B28,
    r#type: B4,
}


#[bitfield]
struct RawEventXAddr {
    x: B11,     // Pixel X coordinate
    pol: B1,    // Event polarity
    r#type: B4, // Event type : EventTypes::EVT_ADDR_X
}

#[bitfield]
struct RawEventVect12 {
    valid: B12, // Encodes the validity of the events in the vector
    r#type: B4, // Event type : EventTypes::VECT_12
}

#[bitfield]
struct RawEventVect8 {
    valid: B8, // Encodes the validity of the events in the vector
    unused: B4,
    r#type: B4, // Event type : EventTypes::VECT_8
}

#[bitfield]
struct RawEventY {
    y: B11,     // Pixel Y coordinate
    orig: B1,   // Identifies the System Type
    r#type: B4, // Event type : EventTypes::EVT_ADDR_Y
}

#[bitfield]
struct RawEventXBase {
    x: B11,     // Pixel X coordinate
    pol: B1,    // Event polarity
    r#type: B4, // Event type : EventTypes::VECT_BASE_X
}

#[bitfield]
struct RawEventExtTrigger {
    value: B1, // Trigger current value (edge polarity)
    unused: B7,
    id: B4,     // Trigger channel ID
    r#type: B4, // Event type : EventTypes::EXT_TRIGGER
}

const WORDS_TO_READ: usize = 1_000_000;

struct Metadata {
    sensor_width: usize,
    sensor_height: usize,
}

impl Default for Metadata {
    fn default() -> Self {
        Metadata {
            sensor_width: 1280,
            sensor_height: 720,
        }
    }
}



pub struct DVSRawDecoderEvt3<R: Read + BufRead + Seek> {
    reader: BufReader<R>,
    pub first_time_base_set: bool,
    pub current_time_base: u64,
    pub current_time_low: u16,
    pub current_time: u64,
    pub current_ev_addr_y: u16,
    pub current_base_x: u16,
    pub current_polarity: u8,
    pub n_time_high_loop: u64,
    buffer_read: Vec<u8>,
    event_queue: VecDeque<DVSEvent>,
}

impl<R: Read + BufRead + Seek> DvsRawDecoder<R> for DVSRawDecoderEvt3<R> {
    fn new(reader: R) -> Self {
        let _buffer_read: Vec<u8> = vec![0; std::mem::size_of::<RawEvent>()];

        Self {
            reader: BufReader::new(reader),
            first_time_base_set: false,
            current_time_base: 0,
            current_time_low: 0,
            current_time: 0,
            current_ev_addr_y: 0,
            current_base_x: 0,
            current_polarity: 0,
            n_time_high_loop: 0,
            buffer_read: vec![0; std::mem::size_of::<RawEvent>()],
            event_queue: VecDeque::new(),
        }
    }

    fn read_header(&mut self) -> anyhow::Result<Vec<String>> {
        // Copy header
        let mut header: Vec<String> = Vec::new();
        // Reset the reader to the beginning
        self.reader.seek(io::SeekFrom::Start(0))?;
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
        let reader = self.reader.get_mut();
        let mut first_char = [0; 1];

        // Reset the reader to the beginning
        reader.seek(io::SeekFrom::Start(0))?;

        loop {
            reader.read_exact(&mut first_char)?;
            if first_char == ['%' as u8] {
                // read the rest of the line
                let mut line = String::new();
                reader.read_line(&mut line)?;
                eprintln!("line: {}", line);
                if line == " end\n" {
                    eprintln!("breaking");

                    break;
                } else if line.starts_with(" format ") {
                    let format_str = &line[8..];
                    let mut parts = format_str.split(';');
                    if parts.next().unwrap() != "EVT3" {
                        eprintln!("Error: detected non-EVT3 input file");
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
                    if &line[5..] != "3.0\n" {
                        dbg!(line[5..].to_string());
                        eprintln!("Error: detected non-EVT3 input file");
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid file format").into());
                    }
                }
            } else {
                // Move the reader back one byte if we didn't have the "% end\n" line
                reader.seek(io::SeekFrom::Current(-1))?;
                eprintln!("breaking2");
                break;
            }
        }

        if metadata.sensor_width > 0 && metadata.sensor_height > 0 {
            eprintln!(
                "%geometry:{}x{}",
                metadata.sensor_width, metadata.sensor_height
            );
        }

        // let mut buffer_read: Vec<u8> = vec![0; WORDS_TO_READ * std::mem::size_of::<RawEvent>()];
        let mut buffer_read: Vec<u8> = vec![0; std::mem::size_of::<RawEvent>()];


        // First, skip any events until we get one of the type EVT_TIME_HIGH
        let mut bytes_read = 0;
        loop {
            bytes_read = reader.read(&mut buffer_read)?;
            if bytes_read == 0 {
                break;
            }

            // let raw_events: &[RawEvent] = bytemuck::cast_slice(&buffer_read[..bytes_read]);

            let mut idx = 0;
            // loop {
            // Get two bytes slice
            let raw_event = &buffer_read[idx..idx + 2];
            idx += 2;
            let raw_event = RawEvent::from_bytes([raw_event[0], raw_event[1]]);
            let event_type = EventTypes::from(raw_event.r#type());
            match event_type {
                EventTypes::EvtTimeHigh => {
                    // Read 4 bytes for RawEventTime
                    let mut time_buf = [0u8; 4];
                    reader.read_exact(&mut time_buf)?;
                    let ev_time_high = RawEventTime::from_bytes(time_buf);
                    self.current_time_base = (ev_time_high.timestamp() as u64) << 12;
                    self.current_time = self.current_time_base;
                    self.first_time_base_set = true;
                    break;
                }
                _ => {}
            }
            // if idx >= bytes_read {
            //     break;
            // }
            // panic!("stop");
            // }
            if self.first_time_base_set {
                eprintln!("Breaking skipper...");
                break;
            }
        }

        Ok(header)
    }

    // fn read_event(&mut self) -> Result<Option<DVSEvent>> {
    //     if let Some(event) = self.event_queue.pop_front() {
    //         return Ok(Some(event));
    //     }

    //     loop {
    //         let bytes_read = self.reader.read(&mut self.buffer_read)?;

    //         if bytes_read < 2 {
    //             return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF").into());
    //         }
    //         let mut idx = 0;
    //         // Get two bytes slice
    //         let raw_event = &self.buffer_read[idx..idx + 2];
    //         idx += 2;
    //         let raw_event = RawEvent::from_bytes([raw_event[0], raw_event[1]]);
    //         let event_type = EventTypes::from(raw_event.r#type());
    //         match event_type {
    //             EventTypes::EvtAddrX => {
    //                 let ev_addr_x = unsafe { &*(&raw_event as *const _ as *const RawEventXAddr) };

    //                 // eprintln!(
    //                 //     "X: {}, Y: {}, Pol: {}",
    //                 //     ev_addr_x.x(),
    //                 //     self.current_ev_addr_y,
    //                 //     ev_addr_x.pol()
    //                 // );

    //                 return Ok(Some(DVSEvent {
    //                     timestamp: self.current_time,
    //                     x: ev_addr_x.x(),
    //                     y: self.current_ev_addr_y,
    //                     polarity: ev_addr_x.pol(),
    //                 }));
    //             }
    //             EventTypes::Vect12 => {
    //                 let ev_vect12 = unsafe { &*(&raw_event as *const _ as *const RawEventVect12) };
    //                 let end = self.current_base_x + 12;
    //                 let mut valid = ev_vect12.valid();
    //                 for i in self.current_base_x..end {
    //                     if valid & 0x1 != 0 {
    //                         self.event_queue.push_back(DVSEvent {
    //                             timestamp: self.current_time,
    //                             x: i,
    //                             y: self.current_ev_addr_y,
    //                             polarity: self.current_polarity,
    //                         });
    //                     }
    //                     valid >>= 1;
    //                 }
    //                 self.current_base_x = end;
    //                 if let Some(event) = self.event_queue.pop_front() {
    //                     return Ok(Some(event));
    //                 }
    //             }
    //             EventTypes::Vect8 => {
    //                 let ev_vect8 = unsafe { &*(&raw_event as *const _ as *const RawEventVect8) };
    //                 let end = self.current_base_x + 8;
    //                 let mut valid = ev_vect8.valid();
    //                 for i in self.current_base_x..end {
    //                     if valid & 0x1 != 0 {
    //                         self.event_queue.push_back(DVSEvent {
    //                             timestamp: self.current_time,
    //                             x: i,
    //                             y: self.current_ev_addr_y,
    //                             polarity: self.current_polarity,
    //                         });
    //                     }
    //                     valid >>= 1;
    //                 }
    //                 self.current_base_x = end;
    //                 if let Some(event) = self.event_queue.pop_front() {
    //                     return Ok(Some(event));
    //                 }
    //             }
    //             EventTypes::EvtAddrY => {
    //                 let ev_addr_y = unsafe { &*(&raw_event as *const _ as *const RawEventY) };
    //                 self.current_ev_addr_y = ev_addr_y.y();
    //             }
    //             EventTypes::VectBaseX => {
    //                 let ev_xbase = unsafe { &*(&raw_event as *const _ as *const RawEventXBase) };
    //                 self.current_polarity = ev_xbase.pol();
    //                 self.current_base_x = ev_xbase.x();
    //             }
    //             EventTypes::EvtTimeHigh => {
    //                 static MAX_TIMESTAMP_BASE: u64 = ((1u64 << 12) - 1) << 12;
    //                 static TIME_LOOP: u64 = MAX_TIMESTAMP_BASE + (1 << 12);
    //                 static LOOP_THRESHOLD: u64 = 10 << 12;

    //                 let ev_time_high = unsafe { &*(&raw_event as *const _ as *const RawEventTime) };
    //                 let mut new_time_base = (ev_time_high.time() as u64) << 12;
    //                 new_time_base += self.n_time_high_loop * TIME_LOOP;

    //                 if (self.current_time_base > new_time_base)
    //                     && (self.current_time_base - new_time_base
    //                         >= MAX_TIMESTAMP_BASE - LOOP_THRESHOLD)
    //                 {
    //                     self.n_time_high_loop += 1;
    //                     dbg!(self.n_time_high_loop);
    //                     new_time_base += TIME_LOOP;
    //                 }

    //                 self.current_time_base = new_time_base;
    //                 self.current_time = self.current_time_base;
    //             }
    //             EventTypes::EvtTimeLow => {
    //                 let ev_time_low = unsafe { &*(&raw_event as *const _ as *const RawEventTime) };
    //                 self.current_time_low = ev_time_low.time();
    //                 self.current_time = self.current_time_base + self.current_time_low as u64;
    //             }
    //             EventTypes::ExtTrigger => {
    //                 // if let Some(ref mut trig_file) = trigger_output_file {
    //                 //     let ev_ext_trigger =
    //                 //         unsafe { &*(&raw_event as *const _ as *const RawEventExtTrigger) };
    //                 //     let timestamp = current_time;
    //                 //
    //                 //     let value = ev_ext_trigger.value();
    //                 //     let id = ev_ext_trigger.id();
    //                 //     trigg_str.push_str(&format!("{},{},{}\n", value, id, timestamp));
    //                 // }
    //             }
    //         }
    //     }

    // }

    fn read_event(&mut self) -> anyhow::Result<Option<DVSRawEvent>> {
        Ok(None)
    }
}

