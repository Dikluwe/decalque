//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/cli-scan-observation-validate.md
//! @layer L3
//! @updated 2026-09-18

use decalque_infra::probe_raster_bytes;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};

fn valid_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&[0, 32, 64, 128, 192, 255], 3, 2, ExtendedColorType::L8)
        .expect("fixture PNG deve ser codificada");
    bytes
}

fn valid_jpeg() -> Vec<u8> {
    let mut bytes = Vec::new();
    let pixels: Vec<u8> = (0..64 * 64)
        .map(|index| ((index * 37 + index / 7) % 256) as u8)
        .collect();
    JpegEncoder::new(&mut bytes)
        .encode(&pixels, 64, 64, ExtendedColorType::L8)
        .expect("fixture JPEG deve ser codificada");
    bytes
}

fn png_with_truncated_idat_and_valid_container(bytes: &[u8]) -> Vec<u8> {
    let mut index = 8usize;
    while index + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[index..index + 4].try_into().unwrap()) as usize;
        let end = index + 12 + length;
        assert!(end <= bytes.len(), "fixture PNG deve ter chunks completos");
        if &bytes[index + 4..index + 8] == b"IDAT" {
            let retained = (length / 2).max(1);
            assert!(retained < length, "IDAT precisa permitir truncamento");
            let mut result = bytes[..index].to_vec();
            result.extend_from_slice(&(retained as u32).to_be_bytes());
            result.extend_from_slice(b"IDAT");
            result.extend_from_slice(&bytes[index + 8..index + 8 + retained]);
            let mut crc = crc32fast::Hasher::new();
            crc.update(b"IDAT");
            crc.update(&bytes[index + 8..index + 8 + retained]);
            result.extend_from_slice(&crc.finalize().to_be_bytes());
            result.extend_from_slice(&bytes[end..]);
            return result;
        }
        index = end;
    }
    panic!("fixture PNG deve conter IDAT");
}

fn jpeg_with_truncated_scan_and_eoi(bytes: &[u8]) -> Vec<u8> {
    let start = bytes
        .windows(2)
        .position(|pair| pair == b"\xff\xda")
        .expect("fixture JPEG deve conter SOS");
    let length = usize::from(u16::from_be_bytes([bytes[start + 2], bytes[start + 3]]));
    let entropy_start = start + 2 + length;
    assert!(
        entropy_start + 2 < bytes.len(),
        "scan JPEG deve ter payload"
    );
    let mut result = bytes[..entropy_start].to_vec();
    result.push(bytes[entropy_start]);
    result.extend_from_slice(b"\xff\xd9");
    result
}

#[test]
fn reconhece_png_jpeg_e_pgm_por_bytes_nao_por_extensao() {
    let png = probe_raster_bytes(&valid_png()).expect("PNG decodificável deve ser reconhecido");
    assert_eq!(png.media_type, "image/png");
    assert_eq!((png.width_px, png.height_px), (3, 2));

    let jpeg = probe_raster_bytes(&valid_jpeg()).expect("JPEG decodificável deve ser reconhecido");
    assert_eq!(jpeg.media_type, "image/jpeg");
    assert_eq!((jpeg.width_px, jpeg.height_px), (64, 64));

    let pgm = probe_raster_bytes(b"P5\n# comentario\n3 2\n255\n\x00\x01\x02\x03\x04\x05")
        .expect("PGM completo deve ser reconhecido");
    assert_eq!(pgm.media_type, "image/x-portable-graymap");
    assert_eq!((pgm.width_px, pgm.height_px), (3, 2));
}

#[test]
fn rejeita_png_com_idat_truncado_mesmo_com_comprimento_e_crc_recalculados() {
    let attacked = png_with_truncated_idat_and_valid_container(&valid_png());
    assert!(probe_raster_bytes(&attacked).is_err());
}

#[test]
fn rejeita_jpeg_com_scan_entropico_truncado_mesmo_com_eoi() {
    let attacked = jpeg_with_truncated_scan_and_eoi(&valid_jpeg());
    assert!(probe_raster_bytes(&attacked).is_err());
}

#[test]
fn rejeita_formato_desconhecido_e_rasters_truncados() {
    for bytes in [
        b"GIF89a".as_slice(),
        b"\x89PNG\r\n\x1a\n".as_slice(),
        b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\0\0\0\0\0\0\0\0IEND\0\0\0\0".as_slice(),
        b"\xff\xd8\xff\xc0\x00".as_slice(),
        b"P5\n3 2\n255\n\x00".as_slice(),
    ] {
        assert!(probe_raster_bytes(bytes).is_err(), "bytes {bytes:?}");
    }
}
