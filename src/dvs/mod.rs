use crate::dvs::raw_decoder_evt2::DVSRawDecoderEvt2;
use crate::dvs::raw_decoder_evt3::DVSRawDecoderEvt3;
use crate::dvs::raw_encoder_evt2::DVSRawEncoderEvt2;
use crate::dvs::raw_decoder_dat::DVSRawDecoderDat;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, Write};
use compression::compression::DVSEvent;

pub mod raw_decoder_evt2;
pub mod raw_decoder_evt3;
pub mod raw_decoder_dat;
pub mod raw_encoder_evt2;


pub trait DvsRawDecoder<R: Read + BufRead + Seek>: Sized {
    fn new(reader: R) -> Self;
    fn read_header(&mut self) -> anyhow::Result<Vec<String>>;
    fn read_event(&mut self) -> anyhow::Result<Option<DVSEvent>>;
}

pub trait DvsRawEncoder<R: Write + Seek>: Sized {
    fn new(reader: R) -> Self;
    fn write_header(&mut self, header: Vec<String>) -> anyhow::Result<()>;
    fn write_event(&mut self, event: DVSEvent) -> anyhow::Result<u8>;

}

pub enum DvsRawDecoderEnum<R: Read + BufRead + Seek> {
    Evt2(DVSRawDecoderEvt2<R>),
    Evt3(DVSRawDecoderEvt3<R>),
    Dat(DVSRawDecoderDat<R>),
}

pub enum DvsRawEncoderEnum<R: Write + Seek> {
    Evt2(DVSRawEncoderEvt2<R>),
}

// Implement the DvsRawDecoder trait for the enum, using enum dispatch (to avoid heap allocation and boxing)
impl<R: Read + BufRead + Seek> DvsRawDecoder<R> for DvsRawDecoderEnum<R> {
    fn new(reader: R) -> Self {
        let _ = reader;
        // This method is not used in the enum implementation
        unimplemented!()
    }

    fn read_header(&mut self) -> anyhow::Result<Vec<String>> {
        match self {
            DvsRawDecoderEnum::Evt2(decoder) => decoder.read_header(),
            DvsRawDecoderEnum::Evt3(decoder) => decoder.read_header(),
            DvsRawDecoderEnum::Dat(decoder) => decoder.read_header(),
        }
    }

    fn read_event(&mut self) -> anyhow::Result<Option<DVSEvent>> {
        match self {
            DvsRawDecoderEnum::Evt2(decoder) => decoder.read_event(),
            DvsRawDecoderEnum::Evt3(decoder) => decoder.read_event(),
            DvsRawDecoderEnum::Dat(decoder) => decoder.read_event(),
        }
    }
}

// Implementations for DVSRawEncoder traits
impl<R: Write + Seek> DvsRawEncoder<R> for DvsRawEncoderEnum<R> {
    // Constructor
    fn new(reader: R) -> Self {
        let _ = reader;
        unimplemented!()
    }

    // Delegates work to specific implementations
    fn write_header(&mut self, header: Vec<String>) -> anyhow::Result<()> {
        match self {
            DvsRawEncoderEnum::Evt2(encoder) => encoder.write_header(header),
        }
    }

    fn write_event(&mut self, event: DVSEvent) -> anyhow::Result<u8> {
        match self {
            DvsRawEncoderEnum::Evt2(encoder) => encoder.write_event(event),
        }
    }

}

pub fn prep_file_decoder<R: std::io::BufRead + std::io::Seek>(file_path: &str) -> anyhow::Result<DvsRawDecoderEnum<BufReader<File>>> {
    // If file extension is .dat, try reading as DAT file
    if file_path.ends_with(".dat") {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let mut decoder = raw_decoder_dat::DVSRawDecoderDat::new(reader);
        decoder.read_header()?;
        return Ok(DvsRawDecoderEnum::Dat(decoder));
    } else if file_path.ends_with(".raw") {
        // If file extension is .raw, try reading as RAW file
        // Try reading it as an EVT2 file
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let mut decoder = DVSRawDecoderEvt2::new(reader);
        match decoder.read_header() {
            Ok(_) => Ok(DvsRawDecoderEnum::Evt2(decoder)),
            Err(_) => {
                // Try reading as an EVT3 file
                let file = File::open(file_path)?;
                let reader = BufReader::new(file);
                let mut decoder = DVSRawDecoderEvt3::new(reader);
                decoder.read_header().expect("Error parsing file header. Invalid file type");
                Ok(DvsRawDecoderEnum::Evt3(decoder))
            }
        }
    } else {
        // If file extension is not .dat or .raw, return an error
        anyhow::bail!("Unsupported file format. Please provide a .dat or .raw file.");
    }
}

pub fn prep_file_encoder<R: std::io::Seek>(file_path: &str) -> anyhow::Result<DvsRawEncoderEnum<BufWriter<File>>> {
    // Delete the file if it exists
    let file_ = File::open(file_path);
    if file_.is_ok() {
        let _ = fs::remove_file(file_path);
    }
    let file =  File::create(file_path).unwrap();
    let writer = BufWriter::new(file);
    Ok(DvsRawEncoderEnum::Evt2(DVSRawEncoderEvt2::new(writer)))
}
