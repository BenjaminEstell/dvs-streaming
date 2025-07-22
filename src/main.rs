use std::io::BufReader;
use dvs::dvs::{prep_file_decoder, DvsRawDecoder, DVSRawEvent};
use clap::Parser;

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


fn decode_events(path: &str) -> Result<(Vec<dvs::dvs::DVSRawEvent>, Vec<String>), Box<dyn std::error::Error>> {
    println!("Creating decoder using path {}", path);
    // Open file
    let mut decoder = prep_file_decoder::<BufReader<std::fs::File>>(path)?;

    let header = decoder.read_header()?;

    // Create a vector to hold events
    let mut events: Vec<dvs::dvs::DVSRawEvent> = Vec::new();

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
    let mut encoder = dvs::dvs::prep_file_encoder::<std::io::BufWriter<std::fs::File>>(path).unwrap();
    // Write header to the file
    let _ = dvs::dvs::DvsRawEncoder::write_header(&mut encoder, header);
    // Write all events to the file
    println!("Writing {} events to file {}", events.len(), path);
    let mut write_ctr = 0;
    for event in events {
        let _ = dvs::dvs::DvsRawEncoder::write_event(&mut encoder, event);
        write_ctr += 1;
    }
    println!("Wrote {} events to file {}", write_ctr, path);
    Ok(())
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line args
    let args = Cli::parse();
    let file_path = args.file_path;
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
    // Write events out to .raw file
    let _ = encode_events(&output_path, events, header);

    Ok(())
}
