//! Suite documental interna.
//!
//! Esta capa NO abre OnlyOffice/LibreOffice ni delega en suites externas. Solo
//! lee contenedores ZIP de Office/OpenDocument y extrae contenido textual
//! básico para previews internas del chat.

use anyhow::{bail, Context, Result};
use flate2::read::DeflateDecoder;
use std::io::Read;
use std::path::Path;

const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENTRY_BYTES: usize = 16 * 1024 * 1024;
const MAX_PREVIEW_LINES: usize = 42;
const MAX_PREVIEW_CHARS: usize = 110;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentPreview {
    pub title: String,
    pub kind: String,
    pub lines: Vec<String>,
    pub note: String,
}

pub fn extract_preview(path: &Path) -> Result<DocumentPreview> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let title = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("documento")
        .to_string();
    let (kind, lines) = match ext.as_str() {
        "docx" => ("DOCX", extract_docx_lines(path)?),
        "odt" => ("ODT", extract_odf_lines(path)?),
        "xlsx" => ("XLSX", extract_xlsx_lines(path)?),
        "ods" => ("ODS", extract_odf_lines(path)?),
        other => bail!("Formato no soportado por la suite interna: {other}"),
    };
    Ok(DocumentPreview {
        title,
        kind: kind.to_string(),
        lines: cap_lines(lines),
        note: "Preview interna: no se abrió ninguna suite externa.".to_string(),
    })
}

fn extract_docx_lines(path: &Path) -> Result<Vec<String>> {
    let xml =
        read_zip_entry_text(path, "word/document.xml")?.context("DOCX sin word/document.xml")?;
    Ok(xml_text_stream(&xml, &["t"], &["p"]))
}

fn extract_odf_lines(path: &Path) -> Result<Vec<String>> {
    let xml = read_zip_entry_text(path, "content.xml")?.context("ODF sin content.xml")?;
    let lines = xml_elements_as_lines(&xml, &["h", "p"]);
    if lines.is_empty() {
        Ok(xml_text_stream(&xml, &["p"], &["p"]))
    } else {
        Ok(lines)
    }
}

fn extract_xlsx_lines(path: &Path) -> Result<Vec<String>> {
    let shared = read_zip_entry_text(path, "xl/sharedStrings.xml")?
        .map(|xml| xml_text_stream(&xml, &["t"], &["si"]))
        .unwrap_or_default();
    let entries = list_zip_entries(path)?;
    let mut sheets = entries
        .into_iter()
        .filter(|name| name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml"))
        .collect::<Vec<_>>();
    sheets.sort();
    let Some(first_sheet) = sheets.first() else {
        return Ok(shared);
    };
    let xml = read_zip_entry_text(path, first_sheet)?.unwrap_or_default();
    let mut rows = Vec::new();
    for row_xml in xml_elements_raw(&xml, &["row"])
        .into_iter()
        .take(MAX_PREVIEW_LINES)
    {
        let mut cells = Vec::new();
        for cell_xml in xml_elements_raw(&row_xml, &["c"]).into_iter().take(12) {
            if let Some(value_xml) = first_element_raw(&cell_xml, "v") {
                let raw = strip_tags(&value_xml);
                let value = if cell_xml.contains(r#"t="s""#) || cell_xml.contains(r#"t='s'"#) {
                    raw.trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|idx| shared.get(idx))
                        .cloned()
                        .unwrap_or(raw)
                } else {
                    raw
                };
                let value = normalize_space(&xml_unescape(&value));
                if !value.is_empty() {
                    cells.push(value);
                }
            }
        }
        if !cells.is_empty() {
            rows.push(cells.join("  ·  "));
        }
    }
    if rows.is_empty() {
        Ok(shared)
    } else {
        Ok(rows)
    }
}

fn read_zip_entry_text(path: &Path, entry: &str) -> Result<Option<String>> {
    Ok(read_zip_entry(path, entry)?.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
}

fn list_zip_entries(path: &Path) -> Result<Vec<String>> {
    let data = std::fs::read(path).with_context(|| format!("leyendo {}", path.display()))?;
    let entries = central_directory(&data)?;
    Ok(entries.into_iter().map(|entry| entry.name).collect())
}

fn read_zip_entry(path: &Path, wanted: &str) -> Result<Option<Vec<u8>>> {
    let data = std::fs::read(path).with_context(|| format!("leyendo {}", path.display()))?;
    if data.len() > MAX_ARCHIVE_BYTES {
        bail!("Documento demasiado grande para preview interna");
    }
    let entries = central_directory(&data)?;
    let Some(entry) = entries.into_iter().find(|entry| entry.name == wanted) else {
        return Ok(None);
    };
    if entry.compressed_size > MAX_ENTRY_BYTES || entry.uncompressed_size > MAX_ENTRY_BYTES {
        bail!("Entrada ZIP demasiado grande para preview interna");
    }
    let offset = entry.local_header_offset;
    ensure_range(&data, offset, 30)?;
    if le_u32(&data[offset..offset + 4]) != 0x0403_4b50 {
        bail!("Cabecera local ZIP inválida");
    }
    let name_len = le_u16(&data[offset + 26..offset + 28]) as usize;
    let extra_len = le_u16(&data[offset + 28..offset + 30]) as usize;
    let data_start = offset
        .checked_add(30)
        .and_then(|v| v.checked_add(name_len))
        .and_then(|v| v.checked_add(extra_len))
        .context("offset ZIP inválido")?;
    ensure_range(&data, data_start, entry.compressed_size)?;
    let compressed = &data[data_start..data_start + entry.compressed_size];
    match entry.method {
        0 => Ok(Some(compressed.to_vec())),
        8 => {
            let mut decoder = DeflateDecoder::new(compressed);
            let mut out = Vec::with_capacity(entry.uncompressed_size.min(MAX_ENTRY_BYTES));
            decoder.read_to_end(&mut out)?;
            if out.len() > MAX_ENTRY_BYTES {
                bail!("Entrada descomprimida demasiado grande");
            }
            Ok(Some(out))
        }
        method => bail!("Compresión ZIP no soportada: {method}"),
    }
}

#[derive(Debug)]
struct ZipEntry {
    name: String,
    method: u16,
    compressed_size: usize,
    uncompressed_size: usize,
    local_header_offset: usize,
}

fn central_directory(data: &[u8]) -> Result<Vec<ZipEntry>> {
    let eocd = find_eocd(data).context("ZIP inválido: EOCD no encontrado")?;
    ensure_range(data, eocd, 22)?;
    let total = le_u16(&data[eocd + 10..eocd + 12]) as usize;
    let cd_size = le_u32(&data[eocd + 12..eocd + 16]) as usize;
    let cd_offset = le_u32(&data[eocd + 16..eocd + 20]) as usize;
    ensure_range(data, cd_offset, cd_size)?;
    let mut pos = cd_offset;
    let end = cd_offset + cd_size;
    let mut entries = Vec::with_capacity(total.min(4096));
    while pos + 46 <= end && entries.len() < total {
        if le_u32(&data[pos..pos + 4]) != 0x0201_4b50 {
            break;
        }
        let method = le_u16(&data[pos + 10..pos + 12]);
        let compressed_size = le_u32(&data[pos + 20..pos + 24]) as usize;
        let uncompressed_size = le_u32(&data[pos + 24..pos + 28]) as usize;
        let name_len = le_u16(&data[pos + 28..pos + 30]) as usize;
        let extra_len = le_u16(&data[pos + 30..pos + 32]) as usize;
        let comment_len = le_u16(&data[pos + 32..pos + 34]) as usize;
        let local_header_offset = le_u32(&data[pos + 42..pos + 46]) as usize;
        let name_start = pos + 46;
        ensure_range(data, name_start, name_len)?;
        let name = String::from_utf8_lossy(&data[name_start..name_start + name_len]).into_owned();
        entries.push(ZipEntry {
            name,
            method,
            compressed_size,
            uncompressed_size,
            local_header_offset,
        });
        pos = name_start
            .checked_add(name_len)
            .and_then(|v| v.checked_add(extra_len))
            .and_then(|v| v.checked_add(comment_len))
            .context("offset central ZIP inválido")?;
    }
    Ok(entries)
}

fn find_eocd(data: &[u8]) -> Option<usize> {
    if data.len() < 22 {
        return None;
    }
    let min = data.len().saturating_sub(22 + 65_535);
    (min..=data.len() - 22)
        .rev()
        .find(|&pos| le_u32(&data[pos..pos + 4]) == 0x0605_4b50)
}

fn ensure_range(data: &[u8], offset: usize, len: usize) -> Result<()> {
    if offset.checked_add(len).is_some_and(|end| end <= data.len()) {
        Ok(())
    } else {
        bail!("ZIP truncado o con offsets inválidos")
    }
}

fn xml_text_stream(xml: &str, text_tags: &[&str], break_tags: &[&str]) -> Vec<String> {
    let mut out = String::new();
    let mut pos = 0;
    while let Some(rel) = xml[pos..].find('<') {
        let start = pos + rel;
        let Some(end_rel) = xml[start..].find('>') else {
            break;
        };
        let end = start + end_rel;
        let tag = &xml[start + 1..end];
        let local = tag_local_name(tag).unwrap_or_default();
        if tag.trim_start().starts_with('/') {
            if break_tags.contains(&local.as_str()) {
                out.push('\n');
            }
            pos = end + 1;
            continue;
        }
        if text_tags.contains(&local.as_str()) {
            let full_name = tag
                .trim()
                .trim_end_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or("");
            let close = format!("</{full_name}>");
            if let Some(close_rel) = xml[end + 1..].find(&close) {
                let text = &xml[end + 1..end + 1 + close_rel];
                out.push_str(&xml_unescape(text));
                out.push(' ');
                pos = end + 1 + close_rel + close.len();
                continue;
            }
        }
        pos = end + 1;
    }
    normalize_lines(&out)
}

fn xml_elements_as_lines(xml: &str, local_names: &[&str]) -> Vec<String> {
    xml_elements_raw(xml, local_names)
        .into_iter()
        .map(|raw| normalize_space(&xml_unescape(&strip_tags(&raw))))
        .filter(|line| !line.is_empty())
        .collect()
}

fn xml_elements_raw(xml: &str, local_names: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(rel) = xml[pos..].find('<') {
        let start = pos + rel;
        let Some(end_rel) = xml[start..].find('>') else {
            break;
        };
        let end = start + end_rel;
        let tag = &xml[start + 1..end];
        if tag.trim_start().starts_with('/') {
            pos = end + 1;
            continue;
        }
        let Some(local) = tag_local_name(tag) else {
            pos = end + 1;
            continue;
        };
        if local_names.contains(&local.as_str()) {
            let full_name = tag
                .trim()
                .trim_end_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or("");
            let close = format!("</{full_name}>");
            if let Some(close_rel) = xml[end + 1..].find(&close) {
                let raw = xml[end + 1..end + 1 + close_rel].to_string();
                out.push(raw);
                pos = end + 1 + close_rel + close.len();
                continue;
            }
        }
        pos = end + 1;
    }
    out
}

fn first_element_raw(xml: &str, local_name: &str) -> Option<String> {
    xml_elements_raw(xml, &[local_name]).into_iter().next()
}

fn strip_tags(xml: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn tag_local_name(tag: &str) -> Option<String> {
    let name = tag
        .trim()
        .trim_start_matches('/')
        .trim_start_matches('?')
        .trim_start_matches('!')
        .trim_end_matches('/')
        .split_whitespace()
        .next()?;
    name.rsplit(':').next().map(|s| s.to_string())
}

fn normalize_lines(input: &str) -> Vec<String> {
    input
        .lines()
        .map(normalize_space)
        .filter(|line| !line.is_empty())
        .collect()
}

fn normalize_space(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cap_lines(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .take(MAX_PREVIEW_LINES)
        .map(|line| {
            let mut s = line.chars().take(MAX_PREVIEW_CHARS).collect::<String>();
            if line.chars().count() > MAX_PREVIEW_CHARS {
                s.push('…');
            }
            s
        })
        .collect()
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn docx_xml_extracts_paragraphs() {
        let xml = r#"<w:document><w:body><w:p><w:r><w:t>Hola</w:t></w:r><w:r><w:t>mundo</w:t></w:r></w:p><w:p><w:r><w:t>Linea &amp; dos</w:t></w:r></w:p></w:body></w:document>"#;
        let lines = xml_text_stream(xml, &["t"], &["p"]);
        assert_eq!(lines, vec!["Hola mundo", "Linea & dos"]);
    }

    #[test]
    fn odf_xml_extracts_text_paragraphs() {
        let xml = r#"<office:document-content><text:h>Titulo</text:h><text:p>Uno <text:span>dos</text:span></text:p></office:document-content>"#;
        let lines = xml_elements_as_lines(xml, &["h", "p"]);
        assert_eq!(lines, vec!["Titulo", "Uno dos"]);
    }

    #[test]
    fn extracts_docx_from_minimal_zip() {
        let base = std::env::temp_dir().join(format!("kda_doc_suite_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let path = base.join("demo.docx");
        write_stored_zip(
            &path,
            "word/document.xml",
            br#"<w:document><w:body><w:p><w:r><w:t>Contrato</w:t></w:r></w:p><w:p><w:r><w:t>Monto &amp; fecha</w:t></w:r></w:p></w:body></w:document>"#,
        );

        let preview = extract_preview(&path).unwrap();
        assert_eq!(preview.kind, "DOCX");
        assert_eq!(preview.lines, vec!["Contrato", "Monto & fecha"]);
        let _ = std::fs::remove_dir_all(&base);
    }

    fn write_stored_zip(path: &PathBuf, name: &str, content: &[u8]) {
        let mut out = Vec::new();
        let name_bytes = name.as_bytes();
        let local_offset = out.len() as u32;
        push_u32(&mut out, 0x0403_4b50);
        push_u16(&mut out, 20);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u32(&mut out, 0);
        push_u32(&mut out, content.len() as u32);
        push_u32(&mut out, content.len() as u32);
        push_u16(&mut out, name_bytes.len() as u16);
        push_u16(&mut out, 0);
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(content);

        let cd_offset = out.len() as u32;
        push_u32(&mut out, 0x0201_4b50);
        push_u16(&mut out, 20);
        push_u16(&mut out, 20);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u32(&mut out, 0);
        push_u32(&mut out, content.len() as u32);
        push_u32(&mut out, content.len() as u32);
        push_u16(&mut out, name_bytes.len() as u16);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u32(&mut out, 0);
        push_u32(&mut out, local_offset);
        out.extend_from_slice(name_bytes);

        let cd_size = out.len() as u32 - cd_offset;
        push_u32(&mut out, 0x0605_4b50);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, 1);
        push_u16(&mut out, 1);
        push_u32(&mut out, cd_size);
        push_u32(&mut out, cd_offset);
        push_u16(&mut out, 0);
        std::fs::write(path, out).unwrap();
    }

    fn push_u16(out: &mut Vec<u8>, value: u16) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }
}
