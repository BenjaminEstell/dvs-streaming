use crate::dvs::{DvsRawEncoder, DVSRawEvent};
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B28, B4, B11, B6};
use std::io::{BufWriter, Write, Seek};

#[derive(Debug, Clone, Copy)]
enum EventTypes {
    CdOff = 0x0,
    CdOn = 0x1,
    EvtTimeHigh = 0x8,
    ExtTrigger = 0xA,
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

#[bitfield]
#[derive(Clone)]
struct RawEvent {
    r#type: B4,
    pad: B28,
}

pub type Timestamp = u64;

#[bitfield]
struct RawEventTime {
    r#type: B4,
    timestamp: B28,
}

#[bitfield]
struct RawEventCD {
    r#type: B4, // Event type : EventTypes::EVT_ADDR_X
    timestamp: B6,
    x: B11, // Pixel X coordinate
    y: B11, // Pixel Y coordinate
}

impl From<RawEventCD> for RawEvent {
    fn from(event: RawEventCD) -> Self {
        let event_cd = RawEvent::new()
            .with_pad((event.timestamp() as u32) << 22 | (event.x() as u32) << 11 | (event.y() as u32))
            .with_type(event.r#type());
        event_cd
    }
}

impl From<RawEventTime> for RawEvent {
    fn from(event: RawEventTime) -> Self {
        let event_time = RawEvent::new()
            .with_pad((event.timestamp() as u32))
            .with_type(event.r#type());
        event_time
    }
}

impl From<RawEvent> for [u8; 4] {
    fn from(event: RawEvent) -> Self {
        let mut value = [0u8; 4];
        value[0] = (event.pad() & 0xFF) as u8;
        value[1] = ((event.pad() >> 8) & 0xFF) as u8;
        value[2] = ((event.pad() >> 16) & 0xFF) as u8;
        value[3] = ((event.pad() >> 24) & 0x0F) as u8 | (event.r#type() << 4) as u8;
        value
    }
}


pub struct DVSRawEncoderEvt2<R: Write + Seek> {
    writer: BufWriter<R>,
}

impl<R: Write + Seek> DvsRawEncoder<R> for DVSRawEncoderEvt2<R> {
    fn new(writer: R) -> Self {
        let _buffer_write: Vec<u8> = vec![0; std::mem::size_of::<RawEvent>()];

        Self {
            writer: BufWriter::new(writer)
        }
    }

    fn write_header(&mut self, header: Vec<String>) -> anyhow::Result<()> {
        let writer = self.writer.get_mut();
        for line in header {
            let buf = line.as_bytes();
            let _res = writer.write_all(buf);
        }

        Ok(())
    }

    fn write_event(&mut self, event: DVSRawEvent) -> anyhow::Result<()> {
        match event {
            DVSRawEvent::CD(ev) => {
                // Determine event type and polarity
                let (event_type, _polarity) = match ev.polarity {
                    0 => (EventTypes::CdOff, 0),
                    1 => (EventTypes::CdOn, 1),
                    _ => (EventTypes::CdOff, 0), // Default or error handling
                };
    
                let timestamp_low = (ev.timestamp & 0x3F) as u8;
                let raw_event_cd = RawEventCD::new()
                    .with_x(ev.x)
                    .with_y(ev.y)
                    .with_timestamp(timestamp_low)
                    .with_type(event_type as u8);
    
                // Convert to RawEvent
                let raw_event = RawEvent::from(raw_event_cd);
                // Convert to bytes and write
                self.writer.write_all(&<[u8; 4]>::from(raw_event))?;
            }
            DVSRawEvent::TimeHigh { timestamp } => {
                let raw_time_event = RawEventTime::new()
                    .with_timestamp(timestamp as u32)
                    .with_type(EventTypes::EvtTimeHigh as u8);
                // Convert to RawEvent
                let raw_event = RawEvent::from(raw_time_event);
                // Convert to bytes and write
                self.writer.write_all(&<[u8; 4]>::from(raw_event))?;
            }
        }
        Ok(())
    }
}
