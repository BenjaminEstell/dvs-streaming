use std::vec;
use std::io::BufReader;
use ebod::dvs::{prep_file_decoder, DvsRawDecoder, DVSRawEvent};
use clap::Parser;

pub type Timestamp = u64;
// Struct to help with parsing command line args
#[derive(Parser, Default, Debug)]
struct Cli {
    // Input event stream file path
    #[arg(short = 'f', long = "file")]
    file_path: String,
    // Type of loss. (Optional. Default: 1)
    #[arg(short = 'l', long = "loss", default_value_t=1)]
    loss_type: i32,
    // Desired bandwidth (in Mbps) used to simulate loss
    #[arg(short = 'b', long = "bandwidth", default_value_t=25.0)]
    bandwidth: f64,
    // Chunksize (in ms) used to group events temporally (Optional. Default: 50)
    #[arg(short = 'c', long = "chunksize", default_value_t=50.0)]
    chunk_size: f64,
    // Output file path (Optional. Default: <input_file>_loss.bin)
    #[arg(short = 'o', long = "output")]
    output_path: String,
}


fn decode_events(path: &str) -> Result<(Vec<ebod::dvs::DVSRawEvent>, Vec<String>), Box<dyn std::error::Error>> {
    println!("Creating decoder using path {}", path);
    // Open file
    let mut decoder = prep_file_decoder::<BufReader<std::fs::File>>(path)?;

    let header = decoder.read_header()?;

    // Create a vector to hold events
    let mut events: Vec<ebod::dvs::DVSRawEvent> = Vec::new();

    // while events can be read from the file
    while let Ok(Some(event)) = decoder.read_event() {
        events.push(event);
    }

    // Print the number of events collected
    println!("Collected {} events", events.len());
    Ok((events, header))
}


fn encode_events(path: &str, events: Vec<DVSRawEvent>, header: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Open or create file
    let mut encoder = ebod::dvs::prep_file_encoder::<std::io::BufWriter<std::fs::File>>(path).unwrap();
    // Write header to the file
    let _ = ebod::dvs::DvsRawEncoder::write_header(&mut encoder, header);
    // Write all events to the file
    println!("Writing {} events to file {}", events.len(), path);
    let mut write_ctr = 0;
    for event in events {
        let _ = ebod::dvs::DvsRawEncoder::write_event(&mut encoder, event);
        write_ctr += 1;
    }
    println!("Wrote {} events to file {}", write_ctr, path);
    Ok(())
}


fn apply_loss(events: Vec<DVSRawEvent>, chunk_size: f64, bandwidth: f64, loss_type: i64) -> Result<Vec<DVSRawEvent>, Box<dyn std::error::Error>> {
    // Apply loss
    // Calculate the maximum number of events that are permitted in each window
    let bits_per_event = 32 as f64;
    let max_events_per_chunk = (bandwidth * 1000.0 * chunk_size) / bits_per_event;

    let mut events_loss: Vec<DVSRawEvent> = vec![];

    // Collect max_events_per_window into a group, and copy into output buffer
    if loss_type == 1 {
        // if loss_type is 1: remove events at the end of the window
        let mut zero_time = events.first().unwrap().timestamp() as f64; // time in microseconds
        let mut events_in_current_chunk = 0;
        for index in 0..events.len() {
            let event = events[index];
            // if the event is an EVT TIME HIGH event, copy to output buffer
            if let DVSRawEvent::TimeHigh { .. } = event {
                events_loss.push(event);
                continue;
            }

            // if the event is located within the current chunk
            if (event.timestamp() as f64) < (zero_time + (chunk_size * 1000.0) as f64) {
                // if the chunk is not full, add the event to the output buffer
                if events_in_current_chunk < max_events_per_chunk as i64 {
                    events_in_current_chunk += 1;
                    events_loss.push(event);
                }
            } else {
                // if the event is located in a different chunk
                // Add the event to the output buffer
                events_loss.push(event);
                // update the zero time to be the beginning of the chunk that this event is located in
                zero_time = event.timestamp() as f64 - (event.timestamp() as f64 % (chunk_size * 1000.0) as f64);
                // Reset the number of events per chunk
                events_in_current_chunk = 1;
            } 
        }
    } else if loss_type == 2 {
        // if loss_type is 2: remove events at equal intervals throughout the window
        let mut zero_time = events.first().unwrap().timestamp() as f64;
        let mut events_in_current_chunk = 0;
        let mut current_chunk: Vec<DVSRawEvent> = vec![];
        for index in 0..events.len() {
            let event = events[index];
            // if the event is an EVT TIME HIGH event, copy to output buffer
            if let DVSRawEvent::TimeHigh { .. } = event {
                current_chunk.push(event);
            } else {
                // if the event is located within the current chunk
                if (event.timestamp() as f64) < zero_time + (chunk_size * 1000.0) as f64 {
                    // Add the event to the chunk
                    events_in_current_chunk += 1;
                    current_chunk.push(event);
                } else {
                    // if the event is located in a different chunk
                    // Apply loss to the current chunk
                    let mut num_removed = 1.0;
                    let num_to_remove = f64::max(0.0, events_in_current_chunk as f64 - max_events_per_chunk);
                    let mut idx = 0;
                    for i in 1..current_chunk.len() + 1 {
                        // if the event is an EVT TIME HIGH event, copy to output buffer
                        let event2 = current_chunk[i - 1];
                        if let DVSRawEvent::TimeHigh { .. } = event2 {
                            events_loss.push(event2);
                            continue;
                        }
                        idx += 1;
                        // If we are not on pace to remove enough elements
                        if (num_removed / idx as f64) < (num_to_remove / events_in_current_chunk as f64) {
                            // Remove the element
                            num_removed += 1.0;
                        } else {
                            // We do not need to remove this element, so copy it into the output buffer
                            events_loss.push(event2);
                        }
                    }

                    // Advance to the next chunk. Update the zero time to be the beginning of the chunk that this event is located in
                    current_chunk.clear();
                    zero_time = event.timestamp() as f64 - (event.timestamp() as f64 % (chunk_size * 1000.0) as f64);
                    // Reset the number of events per chunk
                    events_in_current_chunk = 1;

                    // Add the event to the current chunk
                    current_chunk.push(event);
                }
            }
        }
        // Apply loss to the last chunk
        let mut num_removed = 1.0;
        let num_to_remove = f64::max(0.0, current_chunk.len() as f64 - max_events_per_chunk);
        for idx in 1..current_chunk.len() + 1 {
            let event = current_chunk[idx - 1];
            // if the event is an EVT TIME HIGH event, copy to output buffer
            if let DVSRawEvent::TimeHigh { .. } = event {
                events_loss.push(event);
                continue;
            }
            // If we are not on pace to remove enough elements
            if (num_removed / idx as f64) < (num_to_remove / current_chunk.len() as f64) {
                // Remove the element
                num_removed += 1.0;
            } else {
                // We do not need to remove this element, so copy it into the output buffer
                events_loss.push(event);
            }
        }
    }
    Ok(events_loss)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line args
    let args = Cli::parse();
    let file_path = args.file_path;
    let loss_type: i64 = args.loss_type as i64;
    let bandwidth: f64 = args.bandwidth as f64;
    let chunk_size: f64 = args.chunk_size as f64;
    let output_path: String = args.output_path;

    // Decode events from file
    let events_ = decode_events(file_path.as_str());

    let (events, header): (Vec<DVSRawEvent>, Vec<String>);
    match events_ {
        Ok((ev, hdr)) => {
            events = ev;
            header = hdr;
        },
        Err(e) =>  {
            println!("Error decoding events");
            return Err(e)
        },
    }

    // Apply loss
    let events_loss = apply_loss(events.clone(), chunk_size, bandwidth, loss_type)?;
    //let events_loss = events.clone(); // For testing purposes, we will not apply loss

    let bits_per_event = 32 as f64;

    // Find the first and last timestamps
    let first_cd_timestamp = events.iter()
        .find(|event| matches!(event, DVSRawEvent::CD { .. }))
        .map_or(0, |event| event.timestamp());

    let last_cd_timestamp = events.iter()
        .rev()
        .find(|event| matches!(event, DVSRawEvent::CD { .. }))
        .map_or(0, |event| event.timestamp());

    println!("First timestamp: {}", first_cd_timestamp);
    println!("Last timestamp: {}", last_cd_timestamp);
    let seconds: f64 = (last_cd_timestamp - first_cd_timestamp) as f64 / 1000000.0;
    let average_mbps = (events_loss.len() as f64 * bits_per_event / 1000000.0) / seconds as f64;
    let average_og_mbps = (events.len() as f64 * bits_per_event / 1000000.0) / seconds as f64;
    println!("The file lasts for {} seconds", seconds);
    println!("Size of original file in Mbits: {}", (events.len() as f64  / 1000000.0) * bits_per_event);
    println!("Size of loss file in Mbits: {}", (events_loss.len() as f64 / 1000000.0) * bits_per_event);
    println!("The selected maximum bandwidth was {0} Mbps, the original average bitrate was {1} Mbps, and the lossy average bitrate was {2} Mbps", bandwidth, average_og_mbps, average_mbps);

    // Write events out to .raw file
    let _ = encode_events(&output_path, events_loss, header);

    Ok(())
}
