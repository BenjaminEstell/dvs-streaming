use std::io::BufReader;
use dvs::dvs::{prep_file_decoder, prep_file_encoder, DvsRawDecoder, DvsRawEncoder};
use clap::Parser;
use compression::compression::encoder::compress_events;
use compression::compression::decoder::decompress_events;
use compression::compression::DVSEvent;

pub type Timestamp = u64;
// Struct to help with parsing command line args
#[derive(Parser, Default, Debug)]
struct Cli {
    // Input event stream file path
    #[arg(short = 'f', long = "file")]
    file_path: String,
    // Output file path (Optional. Default: <input_file>_loss.bin)
    #[arg(short = 'o', long = "output")]
    output_path: String,
}


fn decode_events(path: &str) -> Result<(Vec<DVSEvent>, Vec<String>, i64), Box<dyn std::error::Error>> {
    // Open file
    let mut decoder = prep_file_decoder::<BufReader<std::fs::File>>(path)?;

    let header = decoder.read_header()?;

    // Create a vector to hold events
    let mut events: Vec<DVSEvent> = Vec::new();

    // while events can be read from the file
    let mut num_events: i64 = 0;
    while let Ok(event_option) = decoder.read_event() {
        match event_option {
            Some(event) =>  {
                events.push(event);
                num_events+=1;
            }
            None => num_events+=1,
        }
    }

    Ok((events, header, num_events))
}


fn encode_events(path: &str, events: Vec<DVSEvent>, header: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Open or create file
    let mut encoder = prep_file_encoder::<std::io::BufWriter<std::fs::File>>(path).unwrap();
    // Write header to the file
    let _ = DvsRawEncoder::write_header(&mut encoder, header);
    // Write all events to the file
    for event in events {
        let _ = DvsRawEncoder::write_event(&mut encoder, event);
    }
    Ok(())
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line args
    let args = Cli::parse();
    let file_path = args.file_path;
    let output_path: String = args.output_path;

    // Decode events from file
    let events_ = decode_events(file_path.as_str());

    let (mut events, header, num_events): (Vec<DVSEvent>, Vec<String>, i64);
    match events_ {
        Ok((ev, hdr, ne)) => {
            events = ev;
            header = hdr;
            num_events = ne;
        },
        Err(e) =>  {
            println!("Error decoding events");
            return Err(e)
        },
    }
    // print the number of events read
    println!("Decoded {} events", num_events);

    // Compress events into intermediate representation
    let time_step_ms = 50;
    let intermediate_representation = compress_events(&mut events, time_step_ms)?;


    let reconstructed_events = decompress_events(&intermediate_representation)?;

    // Verify reconstructed events are the same as the original events
    if events.len() != reconstructed_events.len() {
        println!(
            "Verification failed: event count mismatch (original: {}, reconstructed: {})",
            events.len(),
            reconstructed_events.len()
        );
    } else {
        let mut all_match = true;
        for (i, (e1, e2)) in events.iter().zip(reconstructed_events.iter()).enumerate() {
            if e1.timestamp != e2.timestamp || e1.x != e2.x || e1.y != e2.y || e1.polarity != e2.polarity {
                println!(
                    "Verification failed at event {}: original = {:?}, reconstructed = {:?}",
                    i, e1, e2
                );
                all_match = false;
                break;
            }
        }
        if all_match {
            println!("Verification passed: all reconstructed events match original events.");
        }
    }

    // Write events out to .raw file
    let _ = encode_events(&output_path, reconstructed_events, header);

    Ok(())
}
