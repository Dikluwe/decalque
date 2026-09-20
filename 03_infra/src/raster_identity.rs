use decalque_core::ScanObservation;
use image::{ImageFormat, ImageReader, Limits};
use sha2::{Digest, Sha256};
use std::ffi::CStr;
use std::fmt;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::Path;
use turbojpeg_sys as tj;

const MAX_DECODED_RASTER_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterMetadata {
    pub media_type: &'static str,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundRasterIdentity {
    pub sha256: String,
    pub metadata: RasterMetadata,
}

#[derive(Debug)]
pub enum RasterIdentityError {
    Io(io::Error),
    LimitExceeded {
        maximum: u64,
        actual: u64,
    },
    UnsupportedFormat,
    Malformed(String),
    Sha256Mismatch {
        declared: String,
        actual: String,
    },
    MediaTypeMismatch {
        declared: String,
        actual: String,
    },
    DimensionsMismatch {
        declared: (u32, u32),
        actual: (u32, u32),
    },
}

impl fmt::Display for RasterIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "erro de leitura do raster: {error}"),
            Self::LimitExceeded { maximum, actual } => write!(
                formatter,
                "raster excede o limite de {maximum} bytes: {actual} bytes"
            ),
            Self::UnsupportedFormat => {
                formatter.write_str("formato de raster não suportado (esperado PNG, JPEG ou PGM)")
            }
            Self::Malformed(detail) => write!(formatter, "raster malformado: {detail}"),
            Self::Sha256Mismatch { declared, actual } => write!(
                formatter,
                "SHA-256 do raster diverge: declarado {declared}, observado {actual}"
            ),
            Self::MediaTypeMismatch { declared, actual } => write!(
                formatter,
                "media type do raster diverge: declarado {declared}, observado {actual}"
            ),
            Self::DimensionsMismatch { declared, actual } => write!(
                formatter,
                "dimensões do raster divergem: declaradas {}x{}, observadas {}x{}",
                declared.0, declared.1, actual.0, actual.1
            ),
        }
    }
}

impl std::error::Error for RasterIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// Inspeciona o conteúdo sem confiar em extensão de arquivo.
pub fn probe_raster_bytes(bytes: &[u8]) -> Result<RasterMetadata, RasterIdentityError> {
    let (format, media_type) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        (ImageFormat::Png, "image/png")
    } else if bytes.starts_with(b"\xff\xd8") {
        (ImageFormat::Jpeg, "image/jpeg")
    } else if (bytes.starts_with(b"P2") || bytes.starts_with(b"P5"))
        && bytes.get(2).is_some_and(u8::is_ascii_whitespace)
    {
        (ImageFormat::Pnm, "image/x-portable-graymap")
    } else {
        return Err(RasterIdentityError::UnsupportedFormat);
    };

    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODED_RASTER_BYTES);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|error| RasterIdentityError::Malformed(error.to_string()))?;
    let width_px = decoded.width();
    let height_px = decoded.height();
    positive_dimensions(format_name(format), width_px, height_px)?;
    drop(decoded);
    if format == ImageFormat::Jpeg {
        decode_jpeg_strict(bytes, width_px, height_px)?;
    }
    Ok(RasterMetadata {
        media_type,
        width_px,
        height_px,
    })
}

fn decode_jpeg_strict(
    bytes: &[u8],
    expected_width: u32,
    expected_height: u32,
) -> Result<(), RasterIdentityError> {
    let mut decoder = TurboJpegDecoder::new()?;
    decoder.set(tj::TJPARAM_TJPARAM_STOPONWARNING, 1)?;
    decoder.set(
        tj::TJPARAM_TJPARAM_MAXMEMORY,
        i32::try_from(MAX_DECODED_RASTER_BYTES / (1024 * 1024)).expect("limite em MiB cabe em i32"),
    )?;
    decoder.set(
        tj::TJPARAM_TJPARAM_MAXPIXELS,
        i32::try_from(MAX_DECODED_RASTER_BYTES / 4).expect("limite de pixels cabe em i32"),
    )?;
    decoder.read_header(bytes)?;
    let width = decoder.get(tj::TJPARAM_TJPARAM_JPEGWIDTH)?;
    let height = decoder.get(tj::TJPARAM_TJPARAM_JPEGHEIGHT)?;
    if (width, height) != (expected_width, expected_height) {
        return malformed("decodificação JPEG estrita diverge nas dimensões");
    }
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .filter(|count| *count <= MAX_DECODED_RASTER_BYTES / 4)
        .ok_or_else(|| {
            RasterIdentityError::Malformed("pixels JPEG excedem o limite de memória".into())
        })?;
    let mut pixels = vec![0_u8; pixel_count as usize];
    decoder.decompress_gray(bytes, &mut pixels)?;
    Ok(())
}

struct TurboJpegDecoder {
    handle: tj::tjhandle,
}

impl TurboJpegDecoder {
    fn new() -> Result<Self, RasterIdentityError> {
        // SAFETY: `tj3Init` has no preconditions. The returned opaque handle is owned by Self.
        let handle = unsafe { tj::tj3Init(tj::TJINIT_TJINIT_DECOMPRESS as i32) };
        if handle.is_null() {
            return malformed("TurboJPEG não conseguiu iniciar o decodificador");
        }
        Ok(Self { handle })
    }

    fn set(&mut self, parameter: tj::TJPARAM, value: i32) -> Result<(), RasterIdentityError> {
        // SAFETY: `self.handle` is live and owned; parameter/value are scalar API arguments.
        let result = unsafe { tj::tj3Set(self.handle, parameter as i32, value) };
        self.check(result)
    }

    fn get(&mut self, parameter: tj::TJPARAM) -> Result<u32, RasterIdentityError> {
        // SAFETY: `self.handle` is live and owned and the parameter is a documented enum value.
        let value = unsafe { tj::tj3Get(self.handle, parameter as i32) };
        u32::try_from(value).map_err(|_| self.error())
    }

    fn read_header(&mut self, bytes: &[u8]) -> Result<(), RasterIdentityError> {
        let length = bytes
            .len()
            .try_into()
            .map_err(|_| RasterIdentityError::Malformed("raster JPEG longo demais".into()))?;
        // SAFETY: the immutable slice remains alive for the duration of the synchronous call.
        let result = unsafe { tj::tj3DecompressHeader(self.handle, bytes.as_ptr(), length) };
        self.check(result)
    }

    fn decompress_gray(
        &mut self,
        bytes: &[u8],
        output: &mut [u8],
    ) -> Result<(), RasterIdentityError> {
        let length = bytes
            .len()
            .try_into()
            .map_err(|_| RasterIdentityError::Malformed("raster JPEG longo demais".into()))?;
        // SAFETY: both slices remain alive and non-overlapping for the synchronous call. The
        // output length was derived from the header dimensions and TJPF_GRAY has one byte/pixel.
        let result = unsafe {
            tj::tj3Decompress8(
                self.handle,
                bytes.as_ptr(),
                length,
                output.as_mut_ptr(),
                0,
                tj::TJPF_TJPF_GRAY,
            )
        };
        self.check(result)
    }

    fn check(&mut self, result: i32) -> Result<(), RasterIdentityError> {
        if result == 0 {
            Ok(())
        } else {
            Err(self.error())
        }
    }

    fn error(&mut self) -> RasterIdentityError {
        // SAFETY: the handle is live and TurboJPEG owns a NUL-terminated error string.
        let pointer = unsafe { tj::tj3GetErrorStr(self.handle) };
        let detail = if pointer.is_null() {
            "erro TurboJPEG sem detalhe".to_string()
        } else {
            // SAFETY: TurboJPEG guarantees a NUL-terminated pointer valid until its next API call.
            unsafe { CStr::from_ptr(pointer) }
                .to_string_lossy()
                .into_owned()
        };
        RasterIdentityError::Malformed(detail)
    }
}

impl Drop for TurboJpegDecoder {
    fn drop(&mut self) {
        // SAFETY: this is the unique live handle and Drop runs exactly once.
        unsafe { tj::tj3Destroy(self.handle) };
    }
}

/// Lê no máximo `maximum_bytes + 1`, calcula o digest e liga a observação ao raster real.
pub fn bind_scan_observation_raster(
    observation: &ScanObservation,
    path: &Path,
    maximum_bytes: u64,
) -> Result<BoundRasterIdentity, RasterIdentityError> {
    let file = File::open(path).map_err(RasterIdentityError::Io)?;
    let metadata = file.metadata().map_err(RasterIdentityError::Io)?;
    if metadata.len() > maximum_bytes {
        return Err(RasterIdentityError::LimitExceeded {
            maximum: maximum_bytes,
            actual: metadata.len(),
        });
    }

    let mut bytes = Vec::with_capacity(metadata.len().min(maximum_bytes) as usize);
    file.take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(RasterIdentityError::Io)?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(RasterIdentityError::LimitExceeded {
            maximum: maximum_bytes,
            actual: bytes.len() as u64,
        });
    }

    let actual_metadata = probe_raster_bytes(&bytes)?;
    let actual_sha256 = lowercase_hex(&Sha256::digest(&bytes));
    let declared = &observation.source.raster;
    if declared.sha256 != actual_sha256 {
        return Err(RasterIdentityError::Sha256Mismatch {
            declared: declared.sha256.clone(),
            actual: actual_sha256,
        });
    }
    if declared.media_type != actual_metadata.media_type {
        return Err(RasterIdentityError::MediaTypeMismatch {
            declared: declared.media_type.clone(),
            actual: actual_metadata.media_type.to_string(),
        });
    }
    let declared_dimensions = (declared.width_px, declared.height_px);
    let actual_dimensions = (actual_metadata.width_px, actual_metadata.height_px);
    if declared_dimensions != actual_dimensions {
        return Err(RasterIdentityError::DimensionsMismatch {
            declared: declared_dimensions,
            actual: actual_dimensions,
        });
    }

    Ok(BoundRasterIdentity {
        sha256: declared.sha256.clone(),
        metadata: actual_metadata,
    })
}

fn format_name(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "PNG",
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Pnm => "PGM",
        _ => "raster",
    }
}

fn positive_dimensions(
    format: &str,
    width_px: u32,
    height_px: u32,
) -> Result<(), RasterIdentityError> {
    if width_px == 0 || height_px == 0 {
        malformed(format!("dimensões {format} devem ser positivas"))
    } else {
        Ok(())
    }
}

fn malformed<T>(detail: impl Into<String>) -> Result<T, RasterIdentityError> {
    Err(RasterIdentityError::Malformed(detail.into()))
}

fn lowercase_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
