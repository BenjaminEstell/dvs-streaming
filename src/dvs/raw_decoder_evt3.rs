use anyhow::Result;
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B1, B11, B12, B4, B7, B8};
use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};

use crate::dvs::DvsRawDecoder;
use compression::compression::DVSEvent;


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
    Continued4 = 0x7,
    Continued12 = 0xF,
    Others = 0xE,
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
            0x7 => EventTypes::Continued4,
            0xF => EventTypes::Continued12,
            0xE => EventTypes::Others,
            _ => EventTypes::ExtTrigger,
        }
    }
}

#[bitfield]
struct RawEvent {
    pad: B12,
    r#type: B4,
}

// Conversion from bytes to RawEvent
impl From<[u8; 2]> for RawEvent {
    fn from(value: [u8; 2]) -> Self {
        let mut event = RawEvent::new();
        let two = value[0] & 0xF;
        let one = value[0] >> 4;
        let four = value[1] & 0xF;
        let three = value[1] >> 4;
        event.set_type(value[1] >> 4);
        event.set_pad(((value[1] & 0xF) as u16) << 8 | (value[0] as u16));
        event
    }
}



#[bitfield]
struct RawEventEvtAddrY {
    y: B11,     // Pixel Y coordinate
    system_type: B1,    // Event polarity
    r#type: B4, // Event type : EventTypes::EVT_ADDR_Y
}

// Conversion from Raw event to EvtAddry
impl From<RawEvent> for RawEventEvtAddrY {
    fn from(raw_event: RawEvent) -> Self {
        let event = RawEventEvtAddrY::new()
            .with_system_type((raw_event.pad() >> 11) as u8)
            .with_y(raw_event.pad() & 0x7FF)
            .with_type(raw_event.r#type());
        event
    }
}


#[bitfield]
struct RawEventEvtAddrX {
    x: B11,     // Pixel X coordinate
    pol: B1,    // Event polarity
    r#type: B4, // Event type : EventTypes::EVT_ADDR_X
}

// Conversion from Raw event to EvtAddrX
impl From<RawEvent> for RawEventEvtAddrX {
    fn from(raw_event: RawEvent) -> Self {
        let event = RawEventEvtAddrX::new()
            .with_pol((raw_event.pad() >> 11) as u8)
            .with_x(raw_event.pad() & 0x7FF)
            .with_type(raw_event.r#type());
        event
    }
}

#[bitfield]
struct RawEventVectBaseX {
    x: B11,     // Pixel X coordinate
    pol: B1,    // Event polarity
    r#type: B4, // Event type : EventTypes::VECT_BASE_X
}

// Conversion from Raw event to VectBaseX
impl From<RawEvent> for RawEventVectBaseX {
    fn from(raw_event: RawEvent) -> Self {
        let event = RawEventVectBaseX::new()
            .with_pol((raw_event.pad() >> 11) as u8)
            .with_x(raw_event.pad() & 0x7FF)
            .with_type(raw_event.r#type());
        event
    }
}

#[bitfield]
struct RawEventVect12 {
    valid: B12, // Encodes the validity of the events in the vector
    r#type: B4, // Event type : EventTypes::VECT_12
}

// Conversion from Raw event to Vect12
impl From<RawEvent> for RawEventVect12 {
    fn from(raw_event: RawEvent) -> Self {
        let event = RawEventVect12::new()
            .with_valid(raw_event.pad())
            .with_type(raw_event.r#type());
        event
    }
}

#[bitfield]
struct RawEventVect8 {
    valid: B8, // Encodes the validity of the events in the vector
    unused: B4,
    r#type: B4, // Event type : EventTypes::VECT_8
}

// Conversion from Raw event to Vect8
impl From<RawEvent> for RawEventVect8 {
    fn from(raw_event: RawEvent) -> Self {
        let event = RawEventVect8::new()
            .with_valid((raw_event.pad() & 0xFF) as u8)
            .with_type(raw_event.r#type());
        event
    }
}

#[bitfield]
struct RawEventEvtTimeLow {
    time: B12,
    r#type: B4,
}

// Conversion from Raw event to EvtTimeLow
impl From<RawEvent> for RawEventEvtTimeLow {
    fn from(raw_event: RawEvent) -> Self {
        let event = RawEventEvtTimeLow::new()
            .with_time(raw_event.pad())
            .with_type(raw_event.r#type());
        event
    }
}

#[bitfield]
struct RawEventContinued4 {
    field: B4,
    r#type: B4,
}

// Conversion from Raw event to Continued4
impl From<RawEvent> for RawEventContinued4 {
    fn from(raw_event: RawEvent) -> Self {
        let event = RawEventContinued4::new()
            .with_field((raw_event.pad() & 0x0F) as u8)
            .with_type(raw_event.r#type());
        event
    }
}


#[bitfield]
struct RawEventEvtTimeHigh {
    time: B12,
    r#type: B4,
}

// Conversion from Raw event to EvtTimeHigh
impl From<RawEvent> for RawEventEvtTimeHigh {
    fn from(raw_event: RawEvent) -> Self {
        let event = RawEventEvtTimeHigh::new()
            .with_time(raw_event.pad())
            .with_type(raw_event.r#type());
        event
    }
}

#[bitfield]
struct RawEventExtTrigger {
    value: B1, // Trigger current value (edge polarity)
    unused: B7,
    id: B4,     // Trigger channel ID
    r#type: B4, // Event type : EventTypes::EXT_TRIGGER
}


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
    pub current_time_low: u32,
    pub current_time: u64,
    pub current_ev_addr_y: u16,
    pub current_base_x: u16,
    pub current_polarity: u8,
    pub n_time_high_loop: u64,
    buffer_read: Vec<[u8; 2]>,
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
            buffer_read: vec![unsafe { std::mem::zeroed() }],
            event_queue: VecDeque::new(),
        }
    }

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
                let mut line = String::new();
                self.reader.read_line(&mut line)?;
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
                self.reader.seek(SeekFrom::Current(-1))?;
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

        // First, skip any events until we get one of the type EVT_TIME_HIGH
        loop {
            self.reader.read_exact(unsafe {
                std::slice::from_raw_parts_mut(
                    self.buffer_read.as_mut_ptr() as *mut u8,
                    std::mem::size_of::<RawEvent>(),
                )
            })?;

            // Get two bytes slice
            let raw_event = RawEvent::from(self.buffer_read[0]);
            let event_type = EventTypes::from(raw_event.r#type());
            match event_type {
                EventTypes::EvtTimeHigh => {
                    // record the EvtTimeHigh event
                    let ev_time_high = RawEventEvtTimeHigh::from(raw_event);
                    // Read next 2 bytes for EVT_Time_Low
                    let mut time_buf = [0u8; 2];
                    self.reader.read_exact(&mut time_buf)?;
                    let ev_time_low = RawEventEvtTimeLow::from(RawEvent::from(time_buf));
                    self.current_time_base = (ev_time_high.time() as u64) << 12 | ev_time_low.time() as u64;
                    self.current_time = self.current_time_base;
                    self.first_time_base_set = true;
                    break;
                }
                _ => {}
            }
        }

        Ok(header)
    }

    fn read_event(&mut self) -> Result<Option<DVSEvent>> {
        if let Some(event) = self.event_queue.pop_front() {
            return Ok(Some(event));
        }

        loop {
            // Read event
            self.reader.read_exact(unsafe {
                std::slice::from_raw_parts_mut(
                    self.buffer_read.as_mut_ptr() as *mut u8,
                    std::mem::size_of::<RawEvent>(),
                )
            })?;

            let raw_event = RawEvent::from(self.buffer_read[0]);
            let event_type = EventTypes::from(raw_event.r#type());
            match event_type {
                EventTypes::EvtAddrX => {
                    let ev_addr_x = RawEventEvtAddrX::from(raw_event);

                    // eprintln!(
                    //     "EvtAddrX X: {}, Y: {}, Timestamp: {}, Pol: {}",
                    //     ev_addr_x.x(),
                    //     self.current_ev_addr_y,
                    //     self.current_time,
                    //     ev_addr_x.pol()
                    // );

                    return Ok(Some(DVSEvent {
                        timestamp: self.current_time,
                        x: ev_addr_x.x(),
                        y: self.current_ev_addr_y,
                        polarity: ev_addr_x.pol(),
                    }));
                }
                EventTypes::Vect12 => {
                    let ev_vect12 = RawEventVect12::from(raw_event);
                    let end = self.current_base_x + 12;
                    let mut valid = ev_vect12.valid();
                    for i in self.current_base_x..end {
                        if valid & 0x1 != 0 {
                            self.event_queue.push_back(DVSEvent {
                                timestamp: self.current_time,
                                x: i,
                                y: self.current_ev_addr_y,
                                polarity: self.current_polarity,
                            });
                        }
                        valid >>= 1;
                    }
                    // eprintln!(
                    //     "Vect12 X: {}, Y: {}, Timestamp: {}, Pol: {}",
                    //     self.current_base_x,
                    //     self.current_ev_addr_y,
                    //     self.current_time,
                    //     self.current_polarity,
                    // );
                    self.current_base_x = end;
                    if let Some(event) = self.event_queue.pop_front() {
                        return Ok(Some(event));
                    }
                }
                EventTypes::Vect8 => {
                    let ev_vect8 = RawEventVect8::from(raw_event);
                    let end = self.current_base_x + 8;
                    let mut valid = ev_vect8.valid();
                    for i in self.current_base_x..end {
                        if valid & 0x1 != 0 {
                            self.event_queue.push_back(DVSEvent {
                                timestamp: self.current_time,
                                x: i,
                                y: self.current_ev_addr_y,
                                polarity: self.current_polarity,
                            });
                        }
                        valid >>= 1;
                    }

                    // eprintln!(
                    //     "Vect8 X: {}, Y: {}, Timestamp: {}, Pol: {}",
                    //     self.current_base_x,
                    //     self.current_ev_addr_y,
                    //     self.current_time,
                    //     self.current_polarity,
                    // );
                    self.current_base_x = end;
                    if let Some(event) = self.event_queue.pop_front() {
                        return Ok(Some(event));
                    }
                }
                EventTypes::EvtAddrY => {
                    let ev_addr_y = RawEventEvtAddrY::from(raw_event);
                    self.current_ev_addr_y = ev_addr_y.y();
                    // eprintln!(
                    //     "EvtAddrY X: {}, Y: {}, Timestamp: {}, Pol: {}",
                    //     self.current_base_x,
                    //     self.current_ev_addr_y,
                    //     self.current_time,
                    //     self.current_polarity,
                    // );
                }
                EventTypes::VectBaseX => {
                    let ev_xbase = RawEventVectBaseX::from(raw_event);
                    self.current_polarity = ev_xbase.pol();
                    self.current_base_x = ev_xbase.x();
                    // eprintln!(
                    //     "VectBaseX X: {}, Y: {}, Timestamp: {}, Pol: {}",
                    //     self.current_base_x,
                    //     self.current_ev_addr_y,
                    //     self.current_time,
                    //     self.current_polarity,
                    // );
                }
                EventTypes::EvtTimeHigh => {
                    static MAX_TIMESTAMP_BASE: u64 = ((1u64 << 12) - 1) << 12;
                    static TIME_LOOP: u64 = MAX_TIMESTAMP_BASE + (1 << 12);
                    static LOOP_THRESHOLD: u64 = 10 << 12;
                    let ev_time_high = RawEventEvtTimeHigh::from(raw_event);
                    let mut new_time_base = (ev_time_high.time() as u64) << 12;
                    new_time_base += self.n_time_high_loop * TIME_LOOP;

                    if (self.current_time_base > new_time_base)
                        && (self.current_time_base - new_time_base
                            >= MAX_TIMESTAMP_BASE - LOOP_THRESHOLD)
                    {
                        self.n_time_high_loop += 1;
                        dbg!(self.n_time_high_loop);
                        new_time_base += TIME_LOOP;
                    }

                    self.current_time_base = new_time_base;
                    self.current_time = self.current_time_base;

                    // eprintln!(
                    //     "EvtTimeHigh X: {}, Y: {}, Timestamp: {}, Pol: {}",
                    //     self.current_base_x,
                    //     self.current_ev_addr_y,
                    //     self.current_time,
                    //     self.current_polarity,
                    // );
                }
                EventTypes::EvtTimeLow => {
                    let ev_time_low = RawEventEvtTimeLow::from(raw_event);
                    self.current_time_low = ev_time_low.time() as u32;
                    self.current_time = self.current_time_base + self.current_time_low as u64;
                    // eprintln!(
                    //     "EvtTimeLow X: {}, Y: {}, Timestamp: {}, Pol: {}",
                    //     self.current_base_x,
                    //     self.current_ev_addr_y,
                    //     self.current_time,
                    //     self.current_polarity,
                    // );
                }
                EventTypes::Continued4 => {

                }
                EventTypes::Continued12 => {

                }
                _ => {
                    //eprintln!("Error: Invalid event type {} from raw event {:?}", raw_event.r#type(), self.buffer_read[0]);
                }
            }   
        }

     }

}

