# dvs-streaming

## Overview

**dvs-streaming** provides an encoder and decoder for parsing event streams from Prophesee's EVT2 format into a simple Rust struct, `DVSEvent`.  
This library is designed for efficient reading and writing of Dynamic Vision Sensor (DVS) event data, enabling easy integration with Rust-based event processing pipelines.

---

## Getting Started

- Run `cargo build` to build the module.
- To run the example, use the command `cargo run -- --file test_day_001.raw --output output_day_001.raw`, replacing the name of the 
input file with a .raw file.
- To incorporate the decoder and encoder into your streaming applications, see the example in 'main.rs'. 
- The decoder and encoder are initialized by `prep_file_decoder()` and `prep_file_encoder()`, respectively.
- Events are read from the file using `decode_events()`, and the output file is written using `encode_events()`

## Prophesee EVT 2.0 Format

The EVT 2.0 format is a 32-bit data format intended for use in applications with low event rates. Data is transmitted from the cameras in little-endian by default.

For more information, see the [Prophesee EVT2 documentation](https://docs.prophesee.ai/stable/data/encoding_formats/evt2.html).

### EVT2 Event Types

- **CD_OFF**: Change detection event with negative polarity.
- **CD_ON**: Change detection event with positive polarity.
- **EVT_TIME_HIGH**: Used for pulling out the upper 28 bits of the event timestamps for better compression.
- **EXT_TRIGGER**: Used for synchronizing events between multiple cameras.  
  _Note: EXT_TRIGGER is not currently supported by this module._

---

## Decoder

- Parses and returns the file header.
- Moves the decoder read head to the first CD event in the file.
- Parses and returns all events in the file.
- For CD events, computes the absolute timestamp of the event.
- For EVT_TIME_HIGH events, computes the absolute timestamp as well.
- Both event types are stored using the `DVSRawEvent` enum as the common datatype.

---

## Encoder

- Writes the file header.
- Writes each event in the event structure to the file.
- Converts each event into bytes as expected by the EVT2 format, splitting timestamps appropriately between the two event types.

---

## Credits

Developed by the Baylor University Multimedia Lab in Waco, TX.  
_Last updated: July 2025_

