use crate::dvs::raw_decoder_evt2::DVSRawDecoderEvt2;
use crate::dvs::raw_decoder_evt3::DVSRawDecoderEvt3;
use crate::dvs::raw_encoder_evt2::DVSRawEncoderEvt2;
use crate::dvs::raw_decoder_dat::DVSRawDecoderDat;
use bytes::Bytes;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, Write};

pub mod raw_decoder_evt2;
pub mod raw_decoder_evt3;
pub mod raw_decoder_dat;
pub mod raw_encoder_evt2;


pub enum CompressionFormat {
    EVT2,
    EVT3,
    DAT
}

// This struct represents a captured event from the sensor
#[derive(Debug, Copy, Clone, Default)]
pub struct DVSEvent {
    pub timestamp: u64,
    pub x: u16,
    pub y: u16,
    pub polarity: u8,
}


//unsafe impl Send for DVSEvent {}
//unsafe impl Sync for DVSEvent {}

// test to see if we need this
impl DVSEvent{
    pub fn slice_to_i64(events: &[DVSEvent]) -> bytes::Bytes {
        let mut array: Vec<u8> = vec![0; events.len() * 4 * std::mem::size_of::<i64>()];
        let mut offset = 0;
        for i in 0..events.len(){
            let curr = &events[i];
            array[offset..(offset+8)].clone_from_slice(curr.timestamp.to_le_bytes().as_slice());
            array[(offset + 8)..(offset+10)].clone_from_slice(curr.x.to_le_bytes().as_slice());
            array[(offset + 16)..(offset+18)].clone_from_slice(curr.y.to_le_bytes().as_slice());
            array[offset + 24] = curr.polarity;
            offset += 4 * std::mem::size_of::<i64>();
        }

        let data: Box<[u8]> = array.into_boxed_slice();
        Bytes::from(data)
    }
}

impl From<DVSEvent> for bytes::Bytes {
    fn from(event: DVSEvent) -> Self {
        let event_ptr = &event as *const DVSEvent as *const u8;
        let event_slice =
            unsafe { std::slice::from_raw_parts(event_ptr, std::mem::size_of::<DVSEvent>()) };
        let boxed_slice: Box<[u8]> = Box::from(event_slice);
        bytes::Bytes::from(boxed_slice)
    }
}

impl From<bytes::Bytes> for DVSEvent {
    fn from(value: Bytes) -> Self {
        unsafe { std::ptr::read(value.as_ptr() as *const DVSEvent) }
    }
}

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

pub fn prep_file_encoder<R: std::io::Seek>(file_path: &str, fmt: CompressionFormat) -> anyhow::Result<DvsRawEncoderEnum<BufWriter<File>>> {
    // Delete the file if it exists
    let file_ = File::open(file_path);
    if file_.is_ok() {
        let _ = fs::remove_file(file_path);
    }
    let file =  File::create(file_path).unwrap();
    let writer = BufWriter::new(file);
    match fmt {
        CompressionFormat::EVT2 => Ok(DvsRawEncoderEnum::Evt2(DVSRawEncoderEvt2::new(writer))),
        CompressionFormat::EVT3 => Ok(DvsRawEncoderEnum::Evt2(DVSRawEncoderEvt2::new(writer))),
        CompressionFormat::DAT => Ok(DvsRawEncoderEnum::Evt2(DVSRawEncoderEvt2::new(writer))),
    }
}
