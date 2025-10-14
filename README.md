# dvs-streaming

## Overview

**dvs-streaming** provides an encoder and decoder for parsing event streams from Prophesee's EVT2 or EVT3 format into a simple Rust struct, `DVSEvent`.  
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


## Prophesee EVT 3.0 Format

The EVT 3.0 format is a 16-bit data format intended for use in applications with high event rates. It adds vectorization and many more event types to reduce the amount of redundant data transmitted, at the cost of encoder and decoder complexity. Data is transmitted from the cameras in little-endian by default. To recover the raw events from this format, the decoder must record some state from previous events, and buffer some events to be processed once the necessary information from subsequent events is processed.

For more information, see the [Prophesee EVT2 documentation](https://docs.prophesee.ai/stable/data/encoding_formats/evt3.html).

### EVT3 Event Types

- **EVT_ADDR_Y**: Records an event, the camera system type, and its y-coordinate
- **EVT_ADDR_X**: Records an event, its polarity, and its x-coordinate
- **VECT_BASE_X**: Records the x-coordinate and polarity to be used for processing subsequent events.
- **VECT_12**: Records a vector of up to 12 events having the same timestamp and y-coordinate, which are saved in the decoder state. The x-coordinate for these events is taken from the existing decoder state, and is incremented for each event in the vector.
- **VEC_8**: Records a vector of up to 8 events having the same timestamp and y-coordinate, which are saved in the decoder state. The x-coordinate for these events is taken from the existing decoder state, and is incremented for each event in the vector.
- **EVT_TIME_LOW**: Records the lower 12 bits of an event timestamp. Updates the decoder state.
- **CONTINUED_4**: Records additional data for previously processed events.
- **EVT_TIME_HIGH**: Records the upper 12 bits of an event timestamp. Updates the decoder state.
- **OTHERS**: Currently unused
- **CONTINUED_12**: Records additional data for previously processed events.
- **EXT_TRIGGER**: Used for synchronizing events between multiple cameras.  
  _Note: EXT_TRIGGER, CONTINUED_4, CONTINUED_12, and OTHERS are not currently supported by this module._

---

## Decoder

- Parses and returns the file header.
- Moves the decoder read head to the first event in the file.
- Parses and returns all events in the file.
- Events are stored using the `DVSEvent` struct

---

## Encoder

- Writes the file header.
- Writes each event in the event structure to the file.
- Converts each event into bytes as expected by the EVT2 format, splitting timestamps appropriately between the two event types.

---

## Credits

Developed by the Baylor University Multimedia Lab in Waco, TX.  
_Last updated: October 2025_

