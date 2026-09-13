//! Chunker de oraciones para TTS streaming (plan de voz §11).
//!
//! El LLM emite tokens; este chunker los acumula y recorta oraciones
//! completas lo antes posible para que piper las sintetice y suene audio
//! mientras el resto de la respuesta sigue llegando.
//!
//! Reglas de corte:
//! - Corte duro: `!` `?` `…` `\n` apenas aparecen.
//! - Corte blando: `.` `:` `;` solo si la oración ya tiene una longitud
//!   mínima y el siguiente carácter no es dígito (`3.14`, `3:30`) ni el
//!   mismo signo (`...`).
//! - Lo que quede al final se entrega con `flush()` aunque no cierre.

/// Longitud mínima (bytes) de la oración para permitir corte blando.
const MIN_SOFT_LEN: usize = 24;

#[derive(Debug, Default)]
pub struct SentenceChunker {
    buf: String,
}

impl SentenceChunker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Acumula texto (uno o varios tokens) y devuelve las oraciones
    /// completas detectadas, listas para sintetizar.
    pub fn push(&mut self, text: &str) -> Vec<String> {
        self.buf.push_str(text);
        let mut out = Vec::new();
        while let Some(end) = self.find_boundary() {
            let raw: String = self.buf.drain(..end).collect();
            let s = raw.trim();
            // Descartar cortes que solo contienen puntuación (p. ej. "…").
            if s.chars().any(|c| c.is_alphanumeric()) {
                out.push(s.to_string());
            }
        }
        out
    }

    /// Resto pendiente cuando el stream terminó. `None` si está vacío.
    pub fn flush(&mut self) -> Option<String> {
        let rest = self.buf.trim();
        if rest.is_empty() || !rest.chars().any(|c| c.is_alphanumeric()) {
            self.buf.clear();
            None
        } else {
            let s = rest.to_string();
            self.buf.clear();
            Some(s)
        }
    }

    /// Byte index EXCLUSIVO del fin de la primera oración completa
    /// (incluye el signo de corte). `None` si aún no hay corte.
    fn find_boundary(&self) -> Option<usize> {
        for (i, ch) in self.buf.char_indices() {
            match ch {
                '!' | '?' | '…' | '\n' => return Some(i + ch.len_utf8()),
                '.' | ':' | ';' => {
                    // La longitud es la de la oración acumulada hasta el
                    // corte (i), no la del buffer completo.
                    if i < MIN_SOFT_LEN {
                        continue;
                    }
                    let next = self.buf[i + ch.len_utf8()..].chars().next();
                    if let Some(n) = next {
                        // No cortar decimales ("3.14"), horas ("3:30")
                        // ni puntos suspensivos ASCII ("...").
                        if n.is_ascii_digit() || n == ch {
                            continue;
                        }
                    }
                    return Some(i + ch.len_utf8());
                }
                _ => {}
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corta_en_signos_duros_enseguida() {
        let mut c = SentenceChunker::new();
        // Emisión greedy: las dos oraciones duras salen en el mismo push.
        let out = c.push("¡Sí! ¿Qué necesitas?");
        assert_eq!(out, vec!["¡Sí!".to_string(), "¿Qué necesitas?".to_string()]);
        assert!(c.flush().is_none());
    }

    #[test]
    fn punto_requiere_longitud_minima() {
        let mut c = SentenceChunker::new();
        // "No." son 3 bytes: no corta aunque haya punto.
        assert!(c.push("No. ").is_empty());
        let out = c.push("Prefiero esperar un poco más.");
        assert_eq!(out, vec!["No. Prefiero esperar un poco más.".to_string()]);
        assert!(c.flush().is_none());
    }

    #[test]
    fn no_corta_decimales_ni_horas() {
        let mut c = SentenceChunker::new();
        let out = c.push("La respuesta es 3.14 según el cálculo. Nos vemos a las 3:30 en点.");
        // 3.14 y 3:30 no cortan; corta tras "cálculo." y al final ('.' con len>=24)
        assert_eq!(out.len(), 2);
        assert!(out[0].contains("3.14"));
        assert!(out[1].contains("3:30"));
        assert!(c.flush().is_none());
    }

    #[test]
    fn no_corta_puntos_suspensivos_ascii() {
        let mut c = SentenceChunker::new();
        let out = c.push("Déjame pensar... ya está listo el resultado final.");
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("Déjame pensar..."));
    }

    #[test]
    fn streaming_letra_a_letra() {
        let mut c = SentenceChunker::new();
        let texto = "Hola, ¿cómo estás? Bien. Perfecto, sigamos adelante con eso!";
        let mut emitted = Vec::new();
        for ch in texto.chars() {
            let mut buf = [0u8; 4];
            emitted.extend(c.push(ch.encode_utf8(&mut buf)));
        }
        emitted.extend(c.flush());
        assert_eq!(
            emitted,
            vec![
                "Hola, ¿cómo estás?".to_string(),
                "Bien. Perfecto, sigamos adelante con eso!".to_string()
            ]
        );
    }

    #[test]
    fn newline_corta_en_markdown() {
        let mut c = SentenceChunker::new();
        let out = c.push("Pasos:\nPrimero esto\nSegundo esto otro");
        assert_eq!(out, vec!["Pasos:".to_string(), "Primero esto".to_string()]);
        assert_eq!(c.flush(), Some("Segundo esto otro".to_string()));
    }

    #[test]
    fn flush_vacio_o_solo_puntuacion_no_emite() {
        let mut c = SentenceChunker::new();
        c.push("Listo!");
        assert!(c.flush().is_none());
        c.push("…");
        assert!(c.flush().is_none());
    }

    #[test]
    fn multibyte_no_parte_caracteres() {
        let mut c = SentenceChunker::new();
        let out = c.push("Está listo… vamos");
        assert_eq!(out, vec!["Está listo…".to_string()]);
        assert_eq!(c.flush(), Some("vamos".to_string()));
    }
}
