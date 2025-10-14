use crate::dvs::DVSEvent;
use crate::dvs::DvsRawEncoder;
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B28, B4, B11, B6};
use std::io::{BufWriter, Write, Seek};

/* 
This file implements an EVT2 raw event encoder for Dynamic Vision Sensor (DVS) data streams.
It provides types and logic to parse a vector of DVSRaWEvents into an EVT2-formatted event file, extract sensor metadata, and decode individual events.
*/

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
#[derive(Clone)]
struct RawEvent {
    r#type: B4,
    pad: B28,
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

// Conversion from RawEventCD to RawEvent
impl From<RawEventCD> for RawEvent {
    fn from(event: RawEventCD) -> Self {
        let event_cd = RawEvent::new()
            .with_pad((event.timestamp() as u32) << 22 | (event.x() as u32) << 11 | (event.y() as u32))
            .with_type(event.r#type());
        event_cd
    }
}

// Conversion from RawEventTime to RawEvent
impl From<RawEventTime> for RawEvent {
    fn from(event: RawEventTime) -> Self {
        let event_time = RawEvent::new()
            .with_pad(event.timestamp() as u32)
            .with_type(event.r#type());
        event_time
    }
}

// Conversion from RawEvent to a byte array for writing to the EVT2 file
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
    first_timehigh_written: bool,
    ts_last_timehigh: i64,
}

impl<R: Write + Seek> DvsRawEncoder<R> for DVSRawEncoderEvt2<R> {
    fn new(writer: R) -> Self {
        let _buffer_write: Vec<u8> = vec![0; std::mem::size_of::<RawEvent>()];

        Self {
            writer: BufWriter::new(writer),
            first_timehigh_written: false,
            ts_last_timehigh: 0,
        }
    }

    // Writes the header to the EVT2 file, including sensor metadata and initial timestamp
    fn write_header(&mut self, header: Vec<String>) -> anyhow::Result<()> {
        let writer = self.writer.get_mut();
        for line in header {
            let buf = line.as_bytes();
            let _res = writer.write_all(buf);
        }

        Ok(())
    }

    // Writes a DVSRawEvent to the EVT2 file, converting it to the appropriate RawEvent format
    fn write_event(&mut self, event: DVSEvent) -> anyhow::Result<u8> {
        let mut events_written: u8 = 0;
        // If necessary, write a Time High event
        // if we haven't generated any time high events yet 
        if !self.first_timehigh_written {
            self.first_timehigh_written = true;
            self.ts_last_timehigh = event.timestamp & !0x3F; // Get the upper 28 bits of the event's timestamp
            // Generate a Time High Event with the same timestamp as the first CD event in the stream
            let raw_time_event = RawEventTime::new()
                .with_timestamp((self.ts_last_timehigh >> 6) as u32)
                .with_type(EventTypes::EvtTimeHigh as u8);
            // Convert to RawEvent
            let raw_event = RawEvent::from(raw_time_event);
            // Convert to bytes and write
            self.writer.write_all(&<[u8; 4]>::from(raw_event))?;
            events_written+=1;
        } else {
            // Find the timestamp of a time high event just before the CD event we are trying to write
            while (self.ts_last_timehigh) < (event.timestamp & !0x3F) {
                // Increment the Time High Timestamp
                self.ts_last_timehigh = self.ts_last_timehigh + 0x40;
            }
            // Generate a Time High Event
            let raw_time_event = RawEventTime::new()
                .with_timestamp((self.ts_last_timehigh >> 6) as u32)
                .with_type(EventTypes::EvtTimeHigh as u8);
            // Convert to RawEvent
            let raw_event = RawEvent::from(raw_time_event);
            // Convert to bytes and write
            self.writer.write_all(&<[u8; 4]>::from(raw_event))?;
            events_written+=1;
        }

        // Then, write the CD Event
        // Determine event type and polarity
        let event_type = match event.polarity {
            0 => EventTypes::CdOff,
            1 => EventTypes::CdOn,
            _ => EventTypes::CdOff, // Default or error handling
        };
        // Write just the lower 6 bits of the timestamp as part of the CD Event
        let timestamp_low = (event.timestamp & 0x3F) as u8;
        let raw_event_cd = RawEventCD::new()
            .with_x(event.x as u16)
            .with_y(event.y as u16)
            .with_timestamp(timestamp_low)
            .with_type(event_type as u8);

        // Convert to RawEvent
        let raw_event = RawEvent::from(raw_event_cd);
        // Convert to bytes and write
        self.writer.write_all(&<[u8; 4]>::from(raw_event))?;
        events_written+=1;

        Ok(events_written)
    }
}
